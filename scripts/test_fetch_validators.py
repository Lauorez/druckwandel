"""Offline regression tests for preserving an installed validator during download."""
import hashlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("fetch_validators", Path(__file__).with_name("fetch-validators.py"))
fetch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fetch)


class DownloadTests(unittest.TestCase):
    def test_bad_checksum_preserves_installed_validator(self):
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "validator.jar"
            target.write_bytes(b"working")
            checksums = {}
            with patch.object(fetch, "get", return_value=io.BytesIO(b"wrong")):
                with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                    fetch.download("unused", target, checksums, "validator", "expected")
            self.assertEqual(target.read_bytes(), b"working")
            self.assertEqual(list(Path(root).iterdir()), [target])
            self.assertEqual(checksums, {})

    def test_interrupted_download_preserves_installed_validator(self):
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "validator.jar"
            target.write_bytes(b"working")
            with patch.object(fetch, "get", return_value=io.BytesIO(b"partial")):
                with patch.object(fetch.shutil, "copyfileobj", side_effect=OSError("interrupted")):
                    with self.assertRaises(OSError):
                        fetch.download("unused", target, {}, "validator")
            self.assertEqual(target.read_bytes(), b"working")
            self.assertEqual(list(Path(root).iterdir()), [target])

    def test_verified_download_replaces_installed_validator(self):
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "validator.jar"
            target.write_bytes(b"old")
            checksums = {}
            digest = hashlib.sha256(b"verified").hexdigest()
            with patch.object(fetch, "get", return_value=io.BytesIO(b"verified")):
                fetch.download("unused", target, checksums, "validator", digest)
            self.assertEqual(target.read_bytes(), b"verified")
            self.assertEqual(checksums, {"validator": digest})

    def test_java_downloads_are_pinned_with_sha256(self):
        for system, arch in [
            ("windows", "x64"),
            ("mac", "aarch64"),
            ("mac", "x64"),
            ("linux", "x64"),
            ("linux", "aarch64"),
        ]:
            target, url, sha256 = fetch.java_download(system, arch)
            self.assertEqual(target, f"{system}-{arch}")
            self.assertTrue(url.startswith("https://github.com/adoptium/temurin21-binaries/releases/download/"))
            self.assertEqual(len(sha256), 64)
            int(sha256, 16)
        with self.assertRaises(ValueError):
            fetch.java_download("windows", "riscv64")


if __name__ == "__main__":
    unittest.main()
