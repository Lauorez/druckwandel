#!/usr/bin/env python3
"""Fetch pinned offline validators and a platform-specific Temurin 21 JRE (Python 3.12+)."""
import hashlib
import json
import platform
import shutil
import tarfile
import tempfile
import urllib.request
import urllib.error
import time
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "resources" / "validators"
MANIFEST = json.loads((ROOT / "manifest.json").read_text())


def get(url):
    request = urllib.request.Request(url, headers={"User-Agent": "druckwandel-validator-build"})
    for attempt in range(3):
        try:
            return urllib.request.urlopen(request, timeout=180)
        except urllib.error.URLError as error:
            if attempt == 2 or (isinstance(error, urllib.error.HTTPError) and error.code not in (403, 408, 429, 500, 502, 503, 504)):
                raise
            time.sleep(2 ** (attempt + 1))


def download(url, target, checksums, key, expected=None):
    target.parent.mkdir(parents=True, exist_ok=True)
    with get(url) as response, target.open("wb") as output:
        shutil.copyfileobj(response, output)
    with target.open("rb") as artifact:
        digest = hashlib.file_digest(artifact, "sha256").hexdigest()
    if expected is not None and digest != expected:
        raise ValueError(f"SHA-256 mismatch for {key}")
    checksums[key] = digest
    print(f"{digest}  {key}", flush=True)


def unpack(archive, destination):
    destination.mkdir(parents=True, exist_ok=True)
    if zipfile.is_zipfile(archive):
        with zipfile.ZipFile(archive) as source:
            for item in source.infolist():
                if not (destination / item.filename).resolve().is_relative_to(destination.resolve()):
                    raise ValueError("Unsafe archive path")
            source.extractall(destination)
    else:
        with tarfile.open(archive) as source:
            source.extractall(destination, filter="data")


def replace_tree(source, destination):
    if destination.exists():
        shutil.rmtree(destination)
    shutil.copytree(source, destination, symlinks=True)


def remove_obsolete_jars(current):
    # Each validator directory contains one standalone JAR. Packaging copies
    # these directories, so previous versions must not remain beside it.
    for candidate in current.parent.glob("*.jar"):
        if candidate != current:
            candidate.unlink()
            print(f"Removed obsolete validator: {candidate.name}", flush=True)


def main():
    checksums = {}
    for tool, url in [
        ("kosit", f"https://github.com/itplr-kosit/validator/releases/download/v{MANIFEST['kosit']['engineVersion']}/{Path(MANIFEST['kosit']['jar']).name}"),
        ("mustang", f"https://repo.maven.apache.org/maven2/org/mustangproject/Mustang-CLI/{MANIFEST['mustang']['engineVersion']}/{Path(MANIFEST['mustang']['jar']).name}"),
        ("verapdf", f"https://repo.maven.apache.org/maven2/org/verapdf/apps/greenfield-apps/{MANIFEST['verapdf']['engineVersion']}/{Path(MANIFEST['verapdf']['jar']).name}"),
    ]:
        relative = MANIFEST[tool]["jar"]
        download(url, ROOT / relative, checksums, relative, MANIFEST[tool]["sha256"])
        remove_obsolete_jars(ROOT / relative)
    with tempfile.TemporaryDirectory(prefix="druckwandel-validators-") as temporary:
        temporary = Path(temporary)
        tag = MANIFEST["kosit"]["configurationTag"]
        name = MANIFEST["kosit"]["configurationArchive"]
        archive = temporary / "configuration.zip"
        download(f"https://github.com/itplr-kosit/validator-configuration-xrechnung/releases/download/{tag}/{name}", archive, checksums, "kosit/configuration.zip", MANIFEST["kosit"]["configurationSha256"])
        unpack(archive, temporary / "configuration")
        scenarios = next((temporary / "configuration").rglob("scenarios.xml"))
        replace_tree(scenarios.parent, ROOT / "kosit" / "xrechnung")
        system = {"Darwin": "mac", "Windows": "windows", "Linux": "linux"}[platform.system()]
        arch = "aarch64" if platform.machine().lower() in ("arm64", "aarch64") else "x64"
        archive = temporary / ("jre.zip" if system == "windows" else "jre.tar.gz")
        download(f"https://api.adoptium.net/v3/binary/latest/21/ga/{system}/{arch}/jre/hotspot/normal/eclipse?project=jdk", archive, checksums, f"jre/{system}-{arch}")
        unpack(archive, temporary / "runtime")
        java = next((temporary / "runtime").rglob("java.exe" if system == "windows" else "java"))
        replace_tree(java.parent.parent, ROOT / "jre")
    (ROOT / "checksums.json").write_text(json.dumps(checksums, indent=2) + "\n")
    print("Offline-Prüfpaket vollständig.")


if __name__ == "__main__":
    main()
