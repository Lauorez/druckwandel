#!/usr/bin/env python3
"""Prepare the pinned private Windows Tesseract bundle, never install system-wide."""
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1] / "resources" / "ocr"
MANIFEST = json.loads((ROOT / "manifest.json").read_text(encoding="utf-8"))


def get(url):
    request = urllib.request.Request(url, headers={"User-Agent": "druckwandel-ocr-build"})
    for attempt in range(3):
        try:
            return urllib.request.urlopen(request, timeout=180)
        except urllib.error.URLError as error:
            if attempt == 2 or (isinstance(error, urllib.error.HTTPError) and error.code not in (403, 408, 429, 500, 502, 503, 504)):
                raise
            time.sleep(2 ** (attempt + 1))


def download(url, target, expected):
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=target.parent, suffix=".download", delete=False) as output:
            temporary = Path(output.name)
            with get(url) as source:
                shutil.copyfileobj(source, output)
        with temporary.open("rb") as source:
            actual = hashlib.file_digest(source, "sha256").hexdigest()
        if actual != expected:
            raise ValueError(f"Unexpected SHA-256 for {target.name}")
        temporary.replace(target)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    print(f"Verified {target.name}", flush=True)


def main():
    if platform.system() != "Windows":
        raise SystemExit("On macOS/Linux, install local Tesseract with deu+eng language data; this bundle is Windows x64 only.")
    compiler = shutil.which("7z") or str(Path("C:/Program Files/7-Zip/7z.exe"))
    if not Path(compiler).is_file():
        raise SystemExit("7-Zip is required to unpack the OCR runtime.")
    with tempfile.TemporaryDirectory(prefix="druckwandel-ocr-build-") as directory:
        directory = Path(directory)
        setup = directory / "tesseract.exe"
        download(MANIFEST["windows"]["url"], setup, MANIFEST["windows"]["sha256"])
        unpacked = directory / "unpacked"
        result = subprocess.run([compiler, "x", "-y", f"-o{unpacked}", str(setup)], capture_output=True)
        if result.returncode not in (0, 1) or not (unpacked / "tesseract.exe").is_file():
            raise RuntimeError("OCR runtime extraction failed")
        new_dlls = {source.name for source in unpacked.glob("*.dll")}
        for old_dll in ROOT.glob("*.dll"):
            if old_dll.name not in new_dlls:
                old_dll.unlink()
        for source in list(unpacked.glob("*.dll")) + [unpacked / "tesseract.exe"]:
            shutil.copy2(source, ROOT / source.name)
        if (ROOT / "doc").exists():
            shutil.rmtree(ROOT / "doc")
        shutil.copytree(unpacked / "doc", ROOT / "doc")
        if (ROOT / "tessdata" / "configs").exists():
            shutil.rmtree(ROOT / "tessdata" / "configs")
        shutil.copytree(unpacked / "tessdata" / "configs", ROOT / "tessdata" / "configs")
    for language, digest in MANIFEST["languages"].items():
        download(f"https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/{MANIFEST['dataTag']}/{language}.traineddata", ROOT / "tessdata" / f"{language}.traineddata", digest)
    subprocess.run([str(ROOT / "tesseract.exe"), "--list-langs", "--tessdata-dir", str(ROOT / "tessdata")], check=True)


if __name__ == "__main__":
    main()
