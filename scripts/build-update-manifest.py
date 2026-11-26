#!/usr/bin/env python3
"""Build and Ed25519-sign the Druckwandel GitHub Releases update manifest (`latest.json`).

Uses Python standard library only (pure-Python RFC 8032 Ed25519) so CI and local
key generation require no third-party pip packages.

Examples:
    # Generate a new Ed25519 keypair once locally:
    python scripts/build-update-manifest.py --keygen

    # Build and sign latest.json + latest.json.sig from built release artifacts:
    UPDATE_SIGNING_PRIVATE_KEY=<64-hex-chars> \\
      python scripts/build-update-manifest.py --version 0.5.0 --artifacts-dir artifacts --output-dir artifacts/release
"""

from __future__ import annotations

import argparse
import base64
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import sys
from typing import Dict, Optional, Tuple

DEFAULT_REPO = "Lauorez/druckwandel"
DEFAULT_PRIVATE_KEY_ENV = "UPDATE_SIGNING_PRIVATE_KEY"

# --- Pure-Python RFC 8032 Ed25519 (Section 5.1) ---
_P = (1 << 255) - 19
_L = (1 << 252) + 27742317777372353535851937790883648493
_D = (-121665 * pow(121666, _P - 2, _P)) % _P
_I = pow(2, (_P - 1) // 4, _P)


def _recover_x(y: int, sign: int) -> Optional[int]:
    if y >= _P:
        return None
    x2 = ((y * y - 1) * pow((_D * y * y + 1) % _P, _P - 2, _P)) % _P
    if x2 == 0:
        return None if sign else 0
    x = pow(x2, (_P + 3) // 8, _P)
    if (x * x - x2) % _P != 0:
        x = (x * _I) % _P
    if (x * x - x2) % _P != 0:
        return None
    if (x & 1) != sign:
        x = _P - x
    return x


_BY = (4 * pow(5, _P - 2, _P)) % _P
_BX = _recover_x(_BY, 0)
assert _BX is not None
_BASE = (_BX, _BY, 1, (_BX * _BY) % _P)
_IDENTITY = (0, 1, 1, 0)


def _point_add(
    p1: Tuple[int, int, int, int], p2: Tuple[int, int, int, int]
) -> Tuple[int, int, int, int]:
    x1, y1, z1, t1 = p1
    x2, y2, z2, t2 = p2
    a = ((y1 - x1) * (y2 - x2)) % _P
    b = ((y1 + x1) * (y2 + x2)) % _P
    c = (2 * _D * t1 * t2) % _P
    d = (2 * z1 * z2) % _P
    e = (b - a) % _P
    f = (d - c) % _P
    g = (d + c) % _P
    h = (b + a) % _P
    return (e * f) % _P, (g * h) % _P, (f * g) % _P, (e * h) % _P


def _scalar_mult(scalar: int, point: Tuple[int, int, int, int]) -> Tuple[int, int, int, int]:
    result = _IDENTITY
    addend = point
    k = scalar
    while k > 0:
        if k & 1:
            result = _point_add(result, addend)
        addend = _point_add(addend, addend)
        k >>= 1
    return result


def _encode_point(point: Tuple[int, int, int, int]) -> bytes:
    x, y, z, _ = point
    z_inv = pow(z, _P - 2, _P)
    x_aff = (x * z_inv) % _P
    y_aff = (y * z_inv) % _P
    encoded = bytearray(y_aff.to_bytes(32, "little"))
    encoded[31] |= (x_aff & 1) << 7
    return bytes(encoded)


def _decode_point(data: bytes) -> Optional[Tuple[int, int, int, int]]:
    if len(data) != 32:
        return None
    y = int.from_bytes(data, "little") & ((1 << 255) - 1)
    sign = data[31] >> 7
    x = _recover_x(y, sign)
    if x is None:
        return None
    return (x, y, 1, (x * y) % _P)


def _point_equal(p1: Tuple[int, int, int, int], p2: Tuple[int, int, int, int]) -> bool:
    x1, y1, z1, _ = p1
    x2, y2, z2, _ = p2
    return (x1 * z2 - x2 * z1) % _P == 0 and (y1 * z2 - y2 * z1) % _P == 0


def _clamp_scalar(seed: bytes) -> Tuple[int, bytes]:
    if len(seed) != 32:
        raise ValueError("Ed25519 private key seed must be exactly 32 bytes.")
    digest = hashlib.sha512(seed).digest()
    a_bytes = bytearray(digest[:32])
    a_bytes[0] &= 248
    a_bytes[31] &= 127
    a_bytes[31] |= 64
    return int.from_bytes(a_bytes, "little"), digest[32:]


def ed25519_public_key(seed: bytes) -> bytes:
    a, _ = _clamp_scalar(seed)
    return _encode_point(_scalar_mult(a, _BASE))


def ed25519_sign(seed: bytes, message: bytes) -> bytes:
    a, prefix = _clamp_scalar(seed)
    pubkey = _encode_point(_scalar_mult(a, _BASE))
    r = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little") % _L
    r_enc = _encode_point(_scalar_mult(r, _BASE))
    k = int.from_bytes(hashlib.sha512(r_enc + pubkey + message).digest(), "little") % _L
    s = (r + k * a) % _L
    return r_enc + s.to_bytes(32, "little")


def ed25519_verify(public_key: bytes, message: bytes, signature: bytes) -> bool:
    if len(public_key) != 32 or len(signature) != 64:
        return False
    a_point = _decode_point(public_key)
    r_point = _decode_point(signature[:32])
    if a_point is None or r_point is None:
        return False
    s = int.from_bytes(signature[32:], "little")
    if s >= _L:
        return False
    k = int.from_bytes(hashlib.sha512(signature[:32] + public_key + message).digest(), "little") % _L
    lhs = _scalar_mult(s, _BASE)
    rhs = _point_add(r_point, _scalar_mult(k, a_point))
    return _point_equal(lhs, rhs)


def parse_private_key_seed(raw: str) -> bytes:
    cleaned = raw.strip()
    if not cleaned:
        raise ValueError("Private key is empty.")
    if len(cleaned) == 64 and re.fullmatch(r"[0-9a-fA-F]{64}", cleaned):
        return bytes.fromhex(cleaned)
    try:
        decoded = base64.b64decode(cleaned, validate=True)
    except Exception as exc:
        raise ValueError(
            "Private key must be 64 hex characters or base64-encoded 32 bytes."
        ) from exc
    if len(decoded) != 32:
        raise ValueError(
            f"Decoded private key must be 32 bytes, got {len(decoded)} bytes."
        )
    return decoded


def generate_keypair() -> Tuple[bytes, bytes]:
    seed = secrets.token_bytes(32)
    return seed, ed25519_public_key(seed)


def sha256_and_size(path: Path) -> Tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(65536)
            if not chunk:
                break
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size


def read_cargo_version(cargo_toml: Path) -> str:
    content = cargo_toml.read_text(encoding="utf-8")
    match = re.search(r'^version\s*=\s*"([^"]+)"', content, flags=re.MULTILINE)
    if not match:
        raise ValueError(f"Could not read package version from {cargo_toml}")
    return match.group(1)


def read_public_key_from_rust_source(updater_rs: Path) -> bytes:
    content = updater_rs.read_text(encoding="utf-8")
    match = re.search(
        r'UPDATE_SIGNING_PUBLIC_KEY_HEX\s*:\s*&str\s*=\s*"([0-9a-fA-F]{64})"',
        content,
    )
    if not match:
        raise ValueError(
            f"Could not find a 64-hex UPDATE_SIGNING_PUBLIC_KEY_HEX constant in {updater_rs}"
        )
    return bytes.fromhex(match.group(1))


def parse_public_key_hex(raw: str) -> bytes:
    cleaned = raw.strip()
    if len(cleaned) != 64 or not re.fullmatch(r"[0-9a-fA-F]{64}", cleaned):
        raise ValueError("Expected public key must be exactly 64 hex characters.")
    return bytes.fromhex(cleaned)


def discover_platform_artifacts(artifacts_dir: Path) -> Dict[str, Path]:
    found: Dict[str, Path] = {}

    windows_dir = artifacts_dir / "windows"
    if windows_dir.is_dir():
        win_candidates = sorted(
            p
            for p in windows_dir.glob("*.exe")
            if p.is_file() and "-AppOnly-" not in p.name
        )
        if len(win_candidates) > 1:
            raise ValueError(
                f"Multiple Windows installers found in {windows_dir}: {[p.name for p in win_candidates]}"
            )
        if win_candidates:
            found["windows-x86_64"] = win_candidates[0]

    macos_dir = artifacts_dir / "macos"
    if macos_dir.is_dir():
        for dmg in sorted(p for p in macos_dir.glob("*.dmg") if p.is_file()):
            lower = dmg.name.lower()
            if "arm64" in lower or "aarch64" in lower:
                key = "macos-aarch64"
            elif "x86_64" in lower or "x64" in lower:
                key = "macos-x86_64"
            else:
                raise ValueError(
                    f"Cannot determine macOS architecture from filename: {dmg.name}"
                )
            if key in found:
                raise ValueError(
                    f"Multiple macOS installers found for {key}: {found[key].name}, {dmg.name}"
                )
            found[key] = dmg

    return found


def build_manifest(
    *,
    version: str,
    tag: str,
    repo: str,
    platform_files: Dict[str, Path],
    published_at: Optional[str] = None,
    notes_url: Optional[str] = None,
) -> dict:
    if not platform_files:
        raise ValueError("No release installer artifacts found.")
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", version):
        raise ValueError(f"Invalid SemVer version: {version!r}")

    release_notes_url = notes_url or f"https://github.com/{repo}/releases/tag/{tag}"
    timestamp = published_at or datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    platforms: Dict[str, dict] = {}
    for platform_key in sorted(platform_files):
        artifact_path = platform_files[platform_key]
        if not artifact_path.is_file():
            raise ValueError(f"Artifact does not exist: {artifact_path}")
        digest, size_bytes = sha256_and_size(artifact_path)
        if size_bytes <= 0:
            raise ValueError(f"Artifact is empty: {artifact_path}")
        platforms[platform_key] = {
            "url": f"https://github.com/{repo}/releases/download/{tag}/{artifact_path.name}",
            "sha256": digest,
            "size_bytes": size_bytes,
        }

    return {
        "version": version,
        "published_at": timestamp,
        "notes_url": release_notes_url,
        "platforms": platforms,
    }


def serialize_manifest(manifest: dict) -> bytes:
    return (json.dumps(manifest, indent=2, ensure_ascii=False) + "\n").encode("utf-8")


def write_signed_manifest(
    manifest: dict,
    output_dir: Path,
    private_key_seed: Optional[bytes],
    expected_public_key: Optional[bytes] = None,
) -> Tuple[Path, Optional[Path]]:
    output_dir.mkdir(parents=True, exist_ok=True)
    raw_manifest = serialize_manifest(manifest)

    if private_key_seed is None:
        manifest_path = output_dir / "latest.json"
        manifest_path.write_bytes(raw_manifest)
        return manifest_path, None

    pubkey = ed25519_public_key(private_key_seed)
    if expected_public_key is not None and pubkey != expected_public_key:
        raise ValueError(
            "Private key does not match the expected public key configured in the app "
            f"(derived {pubkey.hex()}, expected {expected_public_key.hex()})."
        )
    signature = ed25519_sign(private_key_seed, raw_manifest)
    if not ed25519_verify(pubkey, raw_manifest, signature):
        raise RuntimeError("Self-verification of generated Ed25519 signature failed.")

    manifest_path = output_dir / "latest.json"
    manifest_path.write_bytes(raw_manifest)
    sig_path = output_dir / "latest.json.sig"
    sig_path.write_text(signature.hex() + "\n", encoding="utf-8")
    return manifest_path, sig_path


def run_keygen() -> int:
    seed, pubkey = generate_keypair()
    print("Ed25519 Update-Signaturschlüsselpaar erzeugt:\n")
    print(f"PRIVATE KEY (GEHEIM – nur als GitHub Secret {DEFAULT_PRIVATE_KEY_ENV} speichern!):")
    print(f"  {seed.hex()}\n")
    print("PUBLIC KEY (in apps/native/src/updater.rs als UPDATE_SIGNING_PUBLIC_KEY_HEX eintragen):")
    print(f"  {pubkey.hex()}\n")
    print("WICHTIG: Den Private Key NIEMALS in Git committen!")
    return 0


def parse_args(argv: Optional[list[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build and Ed25519-sign latest.json for Druckwandel GitHub Releases."
    )
    parser.add_argument(
        "--keygen",
        action="store_true",
        help="Generate a new Ed25519 keypair and print setup instructions.",
    )
    parser.add_argument("--version", help="Release SemVer version (e.g. 0.5.0).")
    parser.add_argument("--tag", help="GitHub release tag (default: v<version>).")
    parser.add_argument(
        "--repo",
        default=DEFAULT_REPO,
        help=f"GitHub owner/repo (default: {DEFAULT_REPO}).",
    )
    parser.add_argument(
        "--artifacts-dir",
        type=Path,
        default=Path("artifacts"),
        help="Directory containing windows/*.exe and macos/*.dmg artifacts.",
    )
    parser.add_argument("--windows-x86-64", type=Path, help="Explicit Windows x64 installer path.")
    parser.add_argument("--macos-aarch64", type=Path, help="Explicit macOS arm64 DMG path.")
    parser.add_argument("--macos-x86-64", type=Path, help="Explicit macOS x86_64 DMG path.")
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("artifacts/release"),
        help="Directory where latest.json and latest.json.sig will be written.",
    )
    parser.add_argument("--published-at", help="Optional ISO-8601 UTC timestamp.")
    parser.add_argument("--notes-url", help="Optional override for GitHub release notes URL.")
    parser.add_argument(
        "--private-key-env",
        default=DEFAULT_PRIVATE_KEY_ENV,
        help=f"Environment variable containing the Ed25519 private key (default: {DEFAULT_PRIVATE_KEY_ENV}).",
    )
    parser.add_argument(
        "--expected-public-key",
        help="Optional 64-hex Ed25519 public key that the private key must derive.",
    )
    parser.add_argument(
        "--expected-public-key-file",
        type=Path,
        help="Optional Rust source file (e.g. apps/native/src/updater.rs) containing UPDATE_SIGNING_PUBLIC_KEY_HEX to verify key consistency before signing.",
    )
    parser.add_argument(
        "--allow-unsigned",
        action="store_true",
        help="Write latest.json without signing (dry-run / local inspection only).",
    )
    return parser.parse_args(argv)


def main(argv: Optional[list[str]] = None) -> int:
    args = parse_args(argv)
    if args.keygen:
        return run_keygen()

    version = args.version
    if not version:
        if args.tag and args.tag.startswith("v"):
            version = args.tag[1:]
        else:
            cargo_toml = Path(__file__).resolve().parent.parent / "apps/native/Cargo.toml"
            version = read_cargo_version(cargo_toml)
    tag = args.tag or f"v{version}"

    platform_files = discover_platform_artifacts(args.artifacts_dir)
    if args.windows_x86_64:
        platform_files["windows-x86_64"] = args.windows_x86_64
    if args.macos_aarch64:
        platform_files["macos-aarch64"] = args.macos_aarch64
    if args.macos_x86_64:
        platform_files["macos-x86_64"] = args.macos_x86_64

    manifest = build_manifest(
        version=version,
        tag=tag,
        repo=args.repo,
        platform_files=platform_files,
        published_at=args.published_at,
        notes_url=args.notes_url,
    )

    expected_pubkey: Optional[bytes] = None
    if args.expected_public_key:
        expected_pubkey = parse_public_key_hex(args.expected_public_key)
    elif args.expected_public_key_file:
        expected_pubkey = read_public_key_from_rust_source(args.expected_public_key_file)

    seed: Optional[bytes] = None
    if not args.allow_unsigned:
        raw_key = os.environ.get(args.private_key_env, "")
        if not raw_key.strip():
            raise SystemExit(
                f"ERROR: Environment variable {args.private_key_env} is not set. "
                "Generate a keypair with --keygen and configure the secret before signing."
            )
        seed = parse_private_key_seed(raw_key)

    manifest_path, sig_path = write_signed_manifest(
        manifest, args.output_dir, seed, expected_public_key=expected_pubkey
    )
    print(f"Wrote {manifest_path}")
    if sig_path:
        print(f"Wrote {sig_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
