"""Offline unit tests for scripts/build-update-manifest.py (RFC 8032 Ed25519 + manifest builder)."""

import base64
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "build_update_manifest", Path(__file__).with_name("build-update-manifest.py")
)
manifest_mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(manifest_mod)


class Ed25519Rfc8032Tests(unittest.TestCase):
    def test_rfc8032_section_7_1_vectors(self):
        # Official RFC 8032 Section 7.1 TEST 1, TEST 2, and TEST 3
        vectors = [
            (
                "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
                "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
                b"",
                "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155"
                "5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
            ),
            (
                "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
                "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
                bytes.fromhex("72"),
                "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da"
                "085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
            ),
            (
                "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
                "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
                bytes.fromhex("af82"),
                "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac"
                "18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
            ),
        ]
        for seed_hex, pub_hex, msg, sig_hex in vectors:
            seed = bytes.fromhex(seed_hex)
            expected_pub = bytes.fromhex(pub_hex)
            expected_sig = bytes.fromhex(sig_hex)
            self.assertEqual(manifest_mod.ed25519_public_key(seed), expected_pub)
            actual_sig = manifest_mod.ed25519_sign(seed, msg)
            self.assertEqual(actual_sig, expected_sig)
            self.assertTrue(manifest_mod.ed25519_verify(expected_pub, msg, actual_sig))
            self.assertFalse(
                manifest_mod.ed25519_verify(expected_pub, msg + b"x", actual_sig)
            )
            tampered = bytearray(actual_sig)
            tampered[0] ^= 0x01
            self.assertFalse(
                manifest_mod.ed25519_verify(expected_pub, msg, bytes(tampered))
            )

    def test_private_key_parsing_supports_hex_and_base64(self):
        seed = bytes(range(32))
        self.assertEqual(manifest_mod.parse_private_key_seed(seed.hex()), seed)
        self.assertEqual(
            manifest_mod.parse_private_key_seed(base64.b64encode(seed).decode("ascii")),
            seed,
        )
        with self.assertRaises(ValueError):
            manifest_mod.parse_private_key_seed("not-valid-key")


class ManifestBuilderTests(unittest.TestCase):
    def test_discovers_artifacts_builds_and_signs_manifest(self):
        with tempfile.TemporaryDirectory() as root:
            artifacts = Path(root) / "artifacts"
            win_dir = artifacts / "windows"
            mac_dir = artifacts / "macos"
            out_dir = Path(root) / "out"
            win_dir.mkdir(parents=True)
            mac_dir.mkdir(parents=True)

            win_bytes = b"MZ-druckwandel-windows-setup"
            mac_arm_bytes = b"DMG-druckwandel-macos-arm64"
            mac_x64_bytes = b"DMG-druckwandel-macos-x86_64"
            (win_dir / "Druckwandel-0.5.0-x64-Setup.exe").write_bytes(win_bytes)
            # AppOnly test setups must be ignored automatically.
            (win_dir / "Druckwandel-0.5.0-x64-AppOnly-Setup.exe").write_bytes(b"ignore")
            (mac_dir / "Druckwandel-0.5.0-arm64.dmg").write_bytes(mac_arm_bytes)
            (mac_dir / "Druckwandel-0.5.0-x86_64.dmg").write_bytes(mac_x64_bytes)

            seed, pubkey = manifest_mod.generate_keypair()
            with patch.dict(
                os.environ, {"UPDATE_SIGNING_PRIVATE_KEY": seed.hex()}, clear=False
            ):
                rc = manifest_mod.main(
                    [
                        "--version",
                        "0.5.0",
                        "--artifacts-dir",
                        str(artifacts),
                        "--output-dir",
                        str(out_dir),
                        "--published-at",
                        "2026-11-25T15:00:00Z",
                    ]
                )
            self.assertEqual(rc, 0)

            manifest_bytes = (out_dir / "latest.json").read_bytes()
            sig_hex = (out_dir / "latest.json.sig").read_text(encoding="utf-8").strip()
            self.assertEqual(len(sig_hex), 128)
            self.assertTrue(
                manifest_mod.ed25519_verify(
                    pubkey, manifest_bytes, bytes.fromhex(sig_hex)
                )
            )

            data = json.loads(manifest_bytes.decode("utf-8"))
            self.assertEqual(data["version"], "0.5.0")
            self.assertEqual(data["published_at"], "2026-11-25T15:00:00Z")
            self.assertEqual(
                data["notes_url"],
                "https://github.com/Lauorez/druckwandel/releases/tag/v0.5.0",
            )
            self.assertEqual(
                set(data["platforms"].keys()),
                {"windows-x86_64", "macos-aarch64", "macos-x86_64"},
            )
            self.assertEqual(
                data["platforms"]["windows-x86_64"],
                {
                    "url": "https://github.com/Lauorez/druckwandel/releases/download/v0.5.0/Druckwandel-0.5.0-x64-Setup.exe",
                    "sha256": hashlib.sha256(win_bytes).hexdigest(),
                    "size_bytes": len(win_bytes),
                },
            )
            self.assertEqual(
                data["platforms"]["macos-aarch64"]["sha256"],
                hashlib.sha256(mac_arm_bytes).hexdigest(),
            )
            self.assertEqual(
                data["platforms"]["macos-x86_64"]["sha256"],
                hashlib.sha256(mac_x64_bytes).hexdigest(),
            )

    def test_requires_private_key_unless_allow_unsigned(self):
        with tempfile.TemporaryDirectory() as root:
            artifacts = Path(root) / "artifacts"
            (artifacts / "windows").mkdir(parents=True)
            (artifacts / "windows" / "Druckwandel-0.5.0-x64-Setup.exe").write_bytes(
                b"setup"
            )
            env = {k: v for k, v in os.environ.items() if k != "UPDATE_SIGNING_PRIVATE_KEY"}
            with patch.dict(os.environ, env, clear=True):
                with self.assertRaises(SystemExit):
                    manifest_mod.main(
                        [
                            "--version",
                            "0.5.0",
                            "--artifacts-dir",
                            str(artifacts),
                            "--output-dir",
                            str(Path(root) / "out"),
                        ]
                    )

    def test_keygen_prints_valid_hex_keypair(self):
        buf = io.StringIO()
        with patch("sys.stdout", buf):
            rc = manifest_mod.main(["--keygen"])
        self.assertEqual(rc, 0)
        output = buf.getvalue()
        hex_lines = [
            line.strip()
            for line in output.splitlines()
            if len(line.strip()) == 64
            and all(ch in "0123456789abcdef" for ch in line.strip())
        ]
        self.assertEqual(len(hex_lines), 2)
        seed = bytes.fromhex(hex_lines[0])
        pubkey = bytes.fromhex(hex_lines[1])
        self.assertEqual(manifest_mod.ed25519_public_key(seed), pubkey)

    def test_expected_public_key_file_detects_key_mismatch(self):
        with tempfile.TemporaryDirectory() as root:
            artifacts = Path(root) / "artifacts"
            (artifacts / "windows").mkdir(parents=True)
            (artifacts / "windows" / "Druckwandel-1.0.0-x64-Setup.exe").write_bytes(
                b"setup-bytes"
            )
            seed, pubkey = manifest_mod.generate_keypair()
            other_seed, _ = manifest_mod.generate_keypair()
            rust_file = Path(root) / "updater.rs"
            rust_file.write_text(
                f'pub const UPDATE_SIGNING_PUBLIC_KEY_HEX: &str = "{pubkey.hex()}";\n',
                encoding="utf-8",
            )
            out_ok = Path(root) / "out_ok"
            with patch.dict(
                os.environ, {"UPDATE_SIGNING_PRIVATE_KEY": seed.hex()}, clear=False
            ):
                rc = manifest_mod.main(
                    [
                        "--version",
                        "1.0.0",
                        "--artifacts-dir",
                        str(artifacts),
                        "--output-dir",
                        str(out_ok),
                        "--expected-public-key-file",
                        str(rust_file),
                    ]
                )
            self.assertEqual(rc, 0)
            self.assertTrue((out_ok / "latest.json.sig").is_file())

            out_bad = Path(root) / "out_bad"
            with patch.dict(
                os.environ, {"UPDATE_SIGNING_PRIVATE_KEY": other_seed.hex()}, clear=False
            ):
                with self.assertRaises(ValueError):
                    manifest_mod.main(
                        [
                            "--version",
                            "1.0.0",
                            "--artifacts-dir",
                            str(artifacts),
                            "--output-dir",
                            str(out_bad),
                            "--expected-public-key-file",
                            str(rust_file),
                        ]
                    )
            self.assertFalse((out_bad / "latest.json").exists())

    def test_real_updater_source_contains_parseable_public_key(self):
        updater_rs = Path(__file__).resolve().parent.parent / "apps/native/src/updater.rs"
        pubkey = manifest_mod.read_public_key_from_rust_source(updater_rs)
        self.assertEqual(len(pubkey), 32)
        self.assertNotEqual(pubkey, b"\x00" * 32)


if __name__ == "__main__":
    unittest.main()
