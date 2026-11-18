#!/usr/bin/env python3
"""Prepare the pinned private Windows Tesseract bundle, never install system-wide."""
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1] / "resources" / "ocr"
MANIFEST = json.loads((ROOT / "manifest.json").read_text(encoding="utf-8"))


def download(url, target, expected):
    request = urllib.request.Request(url, headers={"User-Agent": "druckwandel-ocr-build"})
    with urllib.request.urlopen(request, timeout=180) as source, target.open("wb") as output:
        shutil.copyfileobj(source, output)
    with target.open("rb") as source:
        actual = hashlib.file_digest(source, "sha256").hexdigest()
    if actual != expected:
        target.unlink()
        raise ValueError(f"Unexpected SHA-256 for {target.name}")
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
        for source in list(unpacked.glob("*.dll")) + [unpacked / "tesseract.exe"]:
            shutil.copy2(source, ROOT / source.name)
        shutil.copytree(unpacked / "doc", ROOT / "doc", dirs_exist_ok=True)
        shutil.copytree(unpacked / "tessdata" / "configs", ROOT / "tessdata" / "configs", dirs_exist_ok=True)
    for language, digest in MANIFEST["languages"].items():
        download(f"https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/{MANIFEST['dataTag']}/{language}.traineddata", ROOT / "tessdata" / f"{language}.traineddata", digest)
    subprocess.run([str(ROOT / "tesseract.exe"), "--list-langs", "--tessdata-dir", str(ROOT / "tessdata")], check=True)


if __name__ == "__main__":
    main()
