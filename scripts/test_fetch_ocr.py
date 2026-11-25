"""Offline regression tests for atomic, verified OCR bundle downloads."""
import hashlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("fetch_ocr", Path(__file__).with_name("fetch-ocr.py"))
fetch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fetch)


class OcrDownloadTests(unittest.TestCase):
    def test_bad_checksum_preserves_existing_language_file(self):
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "deu.traineddata"
            target.write_bytes(b"working")
            with patch.object(fetch, "get", return_value=io.BytesIO(b"corrupt")):
                with self.assertRaisesRegex(ValueError, "Unexpected SHA-256"):
                    fetch.download("unused", target, "expected")
            self.assertEqual(target.read_bytes(), b"working")
            self.assertEqual(list(Path(root).iterdir()), [target])

    def test_interrupted_download_preserves_existing_language_file(self):
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "deu.traineddata"
            target.write_bytes(b"working")
            with patch.object(fetch, "get", return_value=io.BytesIO(b"partial")):
                with patch.object(fetch.shutil, "copyfileobj", side_effect=OSError("interrupted")):
                    with self.assertRaises(OSError):
                        fetch.download("unused", target, "expected")
            self.assertEqual(target.read_bytes(), b"working")
            self.assertEqual(list(Path(root).iterdir()), [target])

    def test_verified_download_replaces_existing_language_file(self):
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "deu.traineddata"
            target.write_bytes(b"old")
            digest = hashlib.sha256(b"verified").hexdigest()
            with patch.object(fetch, "get", return_value=io.BytesIO(b"verified")):
                fetch.download("unused", target, digest)
            self.assertEqual(target.read_bytes(), b"verified")


if __name__ == "__main__":
    unittest.main()
