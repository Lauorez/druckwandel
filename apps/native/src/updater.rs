//! Lean, anonymous, cryptographically verified auto-updater backed by GitHub Releases.
//!
//! Security model:
//! - Anonymous HTTPS-only `GET` requests to static release assets (no telemetry, no IDs).
//! - Manifest bytes (`latest.json`) are strictly size-limited and verified against an
//!   Ed25519 signature (`latest.json.sig`) using a compiled-in public key BEFORE JSON parsing.
//! - Every URL in the manifest and every HTTP redirect hop is checked against a strict
//!   HTTPS allowlist for this repository's GitHub Releases and official GitHub CDN hosts.
//! - Installers are streamed into a dedicated temporary directory with a hard byte cap
//!   and verified against the signed SHA-256 digest and exact byte length before persisting
//!   or launching. Temporary files are removed automatically on any error.

use crate::file_io::encode_hex;
use ed25519_dalek::{Signature, VerifyingKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

pub const REPOSITORY_OWNER: &str = "Lauorez";
pub const REPOSITORY_NAME: &str = "druckwandel";
pub const RELEASE_URL_PREFIX: &str = "https://github.com/Lauorez/druckwandel/releases/";
pub const LATEST_MANIFEST_URL: &str =
    "https://github.com/Lauorez/druckwandel/releases/latest/download/latest.json";
pub const LATEST_SIGNATURE_URL: &str =
    "https://github.com/Lauorez/druckwandel/releases/latest/download/latest.json.sig";

pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
pub const MAX_SIGNATURE_BYTES: u64 = 512;
pub const MAX_INSTALLER_BYTES: u64 = 512 * 1024 * 1024;
pub const AUTO_CHECK_COOLDOWN_SECS: i64 = 24 * 60 * 60;

const MAX_REDIRECTS: usize = 5;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const MANIFEST_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_RECV_TIMEOUT: Duration = Duration::from_secs(60);
const STATIC_USER_AGENT: &str = "Druckwandel-Updater";

const ALLOWED_REDIRECT_CDN_HOSTS: &[&str] = &[
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "github-releases.githubusercontent.com",
];

/// Compiled-in Ed25519 public key (32 bytes encoded as 64 lowercase hex characters)
/// used to verify `latest.json.sig` over the exact raw bytes of `latest.json`.
///
/// Key rotation:
/// 1. Run `python scripts/build-update-manifest.py --keygen` locally.
/// 2. Store the private key ONLY in the GitHub Actions secret `UPDATE_SIGNING_PRIVATE_KEY`.
/// 3. Update this constant to the printed 64-character public key hex string.
///
/// If a placeholder or empty value is ever present, update verification refuses to proceed
/// instead of silently bypassing signature checks.
pub const UPDATE_SIGNING_PUBLIC_KEY_HEX: &str =
    "495b3b24b58ca370a96c4bc17679b6af7a1fe1d9e90704884836d137f4b3faf3";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct UpdateManifest {
    pub version: String,
    #[serde(default)]
    pub published_at: Option<String>,
    pub notes_url: String,
    pub platforms: BTreeMap<String, PlatformAsset>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlatformAsset {
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub published_at: Option<String>,
    pub notes_url: String,
    pub platform_key: String,
    pub asset: PlatformAsset,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateCheckOutcome {
    UpToDate { current_version: String },
    UpdateAvailable(UpdateInfo),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateError {
    PublicKeyNotConfigured,
    UnsupportedPlatform(String),
    InvalidUrl(String),
    ManifestNotFound,
    Network(String),
    SizeLimitExceeded { limit: u64 },
    InvalidSignature(String),
    InvalidManifest(String),
    ChecksumMismatch { expected: String, actual: String },
    SizeMismatch { expected: u64, actual: u64 },
    Io(String),
    LaunchFailed(String),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PublicKeyNotConfigured => write!(
                f,
                "Automatische Updates sind in diesem Build noch nicht aktiviert: Der öffentliche Ed25519-Signaturschlüssel ist noch nicht hinterlegt."
            ),
            Self::UnsupportedPlatform(platform) => {
                write!(
                    f,
                    "Für diese Plattform ({platform}) ist kein Update-Paket im Manifest hinterlegt."
                )
            }
            Self::InvalidUrl(reason) => write!(f, "Ungültige Update-URL: {reason}"),
            Self::ManifestNotFound => write!(
                f,
                "Für das aktuelle GitHub-Release ist noch kein Update-Manifest (latest.json) hinterlegt."
            ),
            Self::Network(reason) => write!(f, "Update-Server nicht erreichbar: {reason}"),
            Self::SizeLimitExceeded { limit } => {
                write!(
                    f,
                    "Die heruntergeladene Datei überschreitet das Sicherheitslimit von {limit} Bytes."
                )
            }
            Self::InvalidSignature(reason) => {
                write!(
                    f,
                    "Signaturprüfung des Update-Manifests fehlgeschlagen: {reason}"
                )
            }
            Self::InvalidManifest(reason) => {
                write!(f, "Ungültiges Update-Manifest: {reason}")
            }
            Self::ChecksumMismatch { expected, actual } => write!(
                f,
                "Prüfsummenfehler (SHA-256) beim Update-Download (erwartet {expected}, erhalten {actual}). Die temporäre Datei wurde verworfen."
            ),
            Self::SizeMismatch { expected, actual } => write!(
                f,
                "Dateigröße des Update-Downloads stimmt nicht mit dem Manifest überein (erwartet {expected} Bytes, erhalten {actual} Bytes)."
            ),
            Self::Io(reason) => write!(f, "Dateifehler beim Update: {reason}"),
            Self::LaunchFailed(reason) => {
                write!(
                    f,
                    "Update-Installer konnte nicht gestartet werden: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for UpdateError {}

pub fn current_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0)
}

pub fn should_auto_check(auto_check: bool, last_checked_unix: Option<i64>, now_unix: i64) -> bool {
    if !auto_check {
        return false;
    }
    match last_checked_unix {
        None => true,
        Some(last) => now_unix < last || now_unix.saturating_sub(last) >= AUTO_CHECK_COOLDOWN_SECS,
    }
}

pub fn current_platform_key() -> Option<&'static str> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        Some("windows-x86_64")
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        Some("macos-aarch64")
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        Some("macos-x86_64")
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
    )))]
    {
        None
    }
}

pub fn configured_public_key() -> Result<[u8; 32], UpdateError> {
    parse_public_key_hex(UPDATE_SIGNING_PUBLIC_KEY_HEX)
}

pub fn is_public_key_configured() -> bool {
    configured_public_key().is_ok()
}

pub fn parse_public_key_hex(raw: &str) -> Result<[u8; 32], UpdateError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with("REPLACE_WITH_") {
        return Err(UpdateError::PublicKeyNotConfigured);
    }
    let decoded = decode_hex_exact::<32>(trimmed).ok_or_else(|| {
        UpdateError::InvalidSignature(
            "Öffentlicher Schlüssel muss 64 Hex-Zeichen enthalten.".into(),
        )
    })?;
    if decoded.iter().all(|&b| b == 0) {
        return Err(UpdateError::PublicKeyNotConfigured);
    }
    let key = VerifyingKey::from_bytes(&decoded)
        .map_err(|e| UpdateError::InvalidSignature(format!("Ungültiger Ed25519-Schlüssel: {e}")))?;
    if key.is_weak() {
        return Err(UpdateError::InvalidSignature(
            "Schwacher Ed25519-Schlüssel wird abgelehnt.".into(),
        ));
    }
    Ok(decoded)
}

fn decode_hex_exact<const N: usize>(hex: &str) -> Option<[u8; N]> {
    if hex.len() != N * 2 || !hex.is_ascii() {
        return None;
    }
    let mut out = [0u8; N];
    for (slot, pair) in out.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
        let hi = hex_nibble(pair[0])?;
        let lo = hex_nibble(pair[1])?;
        *slot = (hi << 4) | lo;
    }
    Some(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn validate_sha256_hex(value: &str) -> Result<String, UpdateError> {
    let trimmed = value.trim();
    if trimmed.len() != 64 || !trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(UpdateError::InvalidManifest(
            "SHA-256-Prüfsumme muss genau 64 Hex-Zeichen enthalten.".into(),
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn parse_https_host_and_path(url: &str) -> Result<(String, &str), UpdateError> {
    if url.is_empty()
        || url
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace() || b == b'\\')
    {
        return Err(UpdateError::InvalidUrl(
            "URL enthält unzulässige Zeichen oder Leerzeichen.".into(),
        ));
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return Err(UpdateError::InvalidUrl(
            "Es sind ausschließlich HTTPS-URLs zulässig.".into(),
        ));
    };
    let Some(slash_idx) = rest.find('/') else {
        return Err(UpdateError::InvalidUrl(
            "URL enthält keinen gültigen Pfad.".into(),
        ));
    };
    let authority = &rest[..slash_idx];
    let path_and_query = &rest[slash_idx..];
    if authority.is_empty() || authority.contains('@') || authority.contains(':') {
        return Err(UpdateError::InvalidUrl(
            "Host-Angabe enthält unzulässige Benutzerdaten oder Ports.".into(),
        ));
    }
    let path_only = path_and_query
        .split(['?', '#'])
        .next()
        .unwrap_or(path_and_query);
    let path_lower = path_only.to_ascii_lowercase();
    if path_only
        .split('/')
        .any(|segment| segment == "." || segment == "..")
        || path_lower.contains("%2e")
        || path_lower.contains("%2f")
        || path_lower.contains("%5c")
    {
        return Err(UpdateError::InvalidUrl(
            "Pfad enthält unzulässige Pfadsegmente.".into(),
        ));
    }
    Ok((authority.to_ascii_lowercase(), path_and_query))
}

/// Strictly validates URLs that appear in the signed manifest or start a release fetch:
/// only `https://github.com/Lauorez/druckwandel/releases/...` without query or fragment.
pub fn validate_release_url(url: &str) -> Result<(), UpdateError> {
    let (host, path_and_query) = parse_https_host_and_path(url)?;
    if host != "github.com" {
        return Err(UpdateError::InvalidUrl(format!(
            "Nicht vertrauenswürdige Domain: {host}"
        )));
    }
    if path_and_query.contains('?') || path_and_query.contains('#') {
        return Err(UpdateError::InvalidUrl(
            "Release-URLs dürfen keine Query-Parameter oder Fragmente enthalten.".into(),
        ));
    }
    let expected_prefix =
        format!("/{REPOSITORY_OWNER}/{REPOSITORY_NAME}/releases/").to_ascii_lowercase();
    let lower_path = path_and_query.to_ascii_lowercase();
    let Some(tail) = lower_path.strip_prefix(&expected_prefix) else {
        return Err(UpdateError::InvalidUrl(format!(
            "URL zeigt nicht auf {RELEASE_URL_PREFIX}"
        )));
    };
    if tail.is_empty() {
        return Err(UpdateError::InvalidUrl(
            "Release-URL ist unvollständig.".into(),
        ));
    }
    Ok(())
}

/// Validates redirect hops during downloads: allows either repository release URLs on
/// `github.com` or GitHub's official release-asset CDN hosts over HTTPS.
pub fn validate_redirect_url(url: &str) -> Result<(), UpdateError> {
    if validate_release_url(url).is_ok() {
        return Ok(());
    }
    let (host, path_and_query) = parse_https_host_and_path(url)?;
    if path_and_query.contains('#') {
        return Err(UpdateError::InvalidUrl(
            "Redirect-URL darf kein Fragment enthalten.".into(),
        ));
    }
    if ALLOWED_REDIRECT_CDN_HOSTS.contains(&host.as_str()) {
        return Ok(());
    }
    Err(UpdateError::InvalidUrl(format!(
        "Redirect auf nicht vertrauenswürdige Domain abgelehnt: {host}"
    )))
}

fn resolve_redirect_location(current_url: &str, location: &str) -> Result<String, UpdateError> {
    let location = location.trim();
    if location.starts_with('/') && !location.starts_with("//") {
        let (host, _) = parse_https_host_and_path(current_url)?;
        let resolved = format!("https://{host}{location}");
        validate_redirect_url(&resolved)?;
        Ok(resolved)
    } else {
        validate_redirect_url(location)?;
        Ok(location.to_owned())
    }
}

pub fn decode_signature(raw_signature: &[u8]) -> Result<[u8; 64], UpdateError> {
    if raw_signature.len() as u64 > MAX_SIGNATURE_BYTES {
        return Err(UpdateError::SizeLimitExceeded {
            limit: MAX_SIGNATURE_BYTES,
        });
    }
    if raw_signature.len() == 64 {
        let mut sig = [0u8; 64];
        sig.copy_from_slice(raw_signature);
        return Ok(sig);
    }
    let text = std::str::from_utf8(raw_signature)
        .map_err(|_| UpdateError::InvalidSignature("Signaturformat ist ungültig.".into()))?
        .trim();
    decode_hex_exact::<64>(text).ok_or_else(|| {
        UpdateError::InvalidSignature(
            "Signatur muss entweder 64 Rohbytes oder 128 Hex-Zeichen enthalten.".into(),
        )
    })
}

pub fn verify_manifest_signature(
    raw_manifest: &[u8],
    raw_signature: &[u8],
    public_key: &[u8; 32],
) -> Result<(), UpdateError> {
    if raw_manifest.is_empty() {
        return Err(UpdateError::InvalidManifest("Manifest ist leer.".into()));
    }
    if raw_manifest.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(UpdateError::SizeLimitExceeded {
            limit: MAX_MANIFEST_BYTES,
        });
    }
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|e| UpdateError::InvalidSignature(format!("Ungültiger Public Key: {e}")))?;
    if key.is_weak() {
        return Err(UpdateError::InvalidSignature(
            "Schwacher Ed25519-Schlüssel wird abgelehnt.".into(),
        ));
    }
    let sig_bytes = decode_signature(raw_signature)?;
    let signature = Signature::from_bytes(&sig_bytes);
    key.verify_strict(raw_manifest, &signature)
        .map_err(|_| UpdateError::InvalidSignature("Ed25519-Signatur ist ungültig.".into()))
}

/// Verifies the Ed25519 signature over the exact raw bytes of `latest.json` BEFORE
/// parsing or trusting any JSON field, then validates SemVer, URLs, checksums, and sizes.
pub fn parse_verified_manifest(
    raw_manifest: &[u8],
    raw_signature: &[u8],
    public_key: &[u8; 32],
) -> Result<UpdateManifest, UpdateError> {
    verify_manifest_signature(raw_manifest, raw_signature, public_key)?;
    let mut manifest: UpdateManifest = serde_json::from_slice(raw_manifest)
        .map_err(|e| UpdateError::InvalidManifest(format!("JSON-Fehler: {e}")))?;
    Version::parse(manifest.version.trim()).map_err(|e| {
        UpdateError::InvalidManifest(format!(
            "Ungültige SemVer-Version '{}': {e}",
            manifest.version
        ))
    })?;
    manifest.version = manifest.version.trim().to_owned();
    validate_release_url(&manifest.notes_url)?;
    if manifest.platforms.is_empty() {
        return Err(UpdateError::InvalidManifest(
            "Manifest enthält keine Plattform-Einträge.".into(),
        ));
    }
    for (platform, asset) in &mut manifest.platforms {
        validate_release_url(&asset.url)?;
        asset.sha256 = validate_sha256_hex(&asset.sha256)?;
        if asset.size_bytes == 0 || asset.size_bytes > MAX_INSTALLER_BYTES {
            return Err(UpdateError::InvalidManifest(format!(
                "Ungültige Dateigröße ({} Bytes) für Plattform '{platform}'.",
                asset.size_bytes
            )));
        }
    }
    Ok(manifest)
}

pub fn is_newer_version(current: &str, candidate: &str) -> Result<bool, UpdateError> {
    let current = Version::parse(current.trim()).map_err(|e| {
        UpdateError::InvalidManifest(format!("Ungültige aktuelle Version '{current}': {e}"))
    })?;
    let candidate = Version::parse(candidate.trim()).map_err(|e| {
        UpdateError::InvalidManifest(format!("Ungültige Update-Version '{candidate}': {e}"))
    })?;
    Ok(candidate > current)
}

pub fn evaluate_manifest(
    manifest: &UpdateManifest,
    current_version: &str,
    platform_key: &str,
) -> Result<UpdateCheckOutcome, UpdateError> {
    if !is_newer_version(current_version, &manifest.version)? {
        return Ok(UpdateCheckOutcome::UpToDate {
            current_version: current_version.trim().to_owned(),
        });
    }
    let Some(asset) = manifest.platforms.get(platform_key).cloned() else {
        return Err(UpdateError::UnsupportedPlatform(platform_key.to_owned()));
    };
    Ok(UpdateCheckOutcome::UpdateAvailable(UpdateInfo {
        version: manifest.version.clone(),
        published_at: manifest.published_at.clone(),
        notes_url: manifest.notes_url.clone(),
        platform_key: platform_key.to_owned(),
        asset,
    }))
}

fn build_http_agent(global_timeout: Option<Duration>, recv_timeout: Duration) -> Agent {
    let tls = TlsConfig::builder()
        .provider(TlsProvider::Rustls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    let mut builder = Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent(STATIC_USER_AGENT)
        .tls_config(tls)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(recv_timeout))
        .timeout_recv_body(Some(recv_timeout));
    if let Some(timeout) = global_timeout {
        builder = builder.timeout_global(Some(timeout));
    }
    Agent::new_with_config(builder.build())
}

fn get_with_safe_redirects(
    agent: &Agent,
    initial_url: &str,
    map_not_found: bool,
) -> Result<(ureq::http::Response<ureq::Body>, String), UpdateError> {
    validate_release_url(initial_url)?;
    let mut current_url = initial_url.to_owned();
    let mut last_release_url = initial_url.to_owned();
    for _ in 0..=MAX_REDIRECTS {
        if validate_release_url(&current_url).is_ok() {
            last_release_url = current_url.clone();
        }
        let response = agent
            .get(&current_url)
            .call()
            .map_err(|e| UpdateError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        if matches!(status, 301 | 302 | 303 | 307 | 308) {
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    UpdateError::Network("Weiterleitung ohne Location-Header erhalten.".into())
                })?;
            current_url = resolve_redirect_location(&current_url, location)?;
            continue;
        }
        if status == 404 && map_not_found {
            return Err(UpdateError::ManifestNotFound);
        }
        if status != 200 {
            return Err(UpdateError::Network(format!(
                "Server antwortete mit HTTP-Status {status}."
            )));
        }
        return Ok((response, last_release_url));
    }
    Err(UpdateError::Network(
        "Zu viele HTTP-Weiterleitungen beim Laden des Updates.".into(),
    ))
}

fn derive_signature_url(resolved_manifest_url: &str) -> String {
    if validate_release_url(resolved_manifest_url).is_ok()
        && resolved_manifest_url.ends_with("/latest.json")
    {
        return format!("{resolved_manifest_url}.sig");
    }
    LATEST_SIGNATURE_URL.to_owned()
}

fn read_limited_bytes(mut reader: impl Read, limit: u64) -> Result<Vec<u8>, UpdateError> {
    let mut buffer = Vec::new();
    reader
        .by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut buffer)
        .map_err(|e| UpdateError::Network(e.to_string()))?;
    if buffer.len() as u64 > limit {
        return Err(UpdateError::SizeLimitExceeded { limit });
    }
    Ok(buffer)
}

/// Checks GitHub Releases for a newer signed update manifest for the current platform.
pub fn check_for_update() -> Result<UpdateCheckOutcome, UpdateError> {
    let public_key = configured_public_key()?;
    let platform_key = current_platform_key()
        .ok_or_else(|| UpdateError::UnsupportedPlatform(std::env::consts::OS.to_owned()))?;
    let agent = build_http_agent(Some(MANIFEST_TIMEOUT), MANIFEST_TIMEOUT);
    let (manifest_response, resolved_manifest_url) =
        get_with_safe_redirects(&agent, LATEST_MANIFEST_URL, true)?;
    let raw_manifest = read_limited_bytes(
        manifest_response.into_body().into_reader(),
        MAX_MANIFEST_BYTES,
    )?;
    let sig_url = derive_signature_url(&resolved_manifest_url);
    let (sig_response, _) = get_with_safe_redirects(&agent, &sig_url, true)?;
    let raw_signature =
        read_limited_bytes(sig_response.into_body().into_reader(), MAX_SIGNATURE_BYTES)?;
    let manifest = parse_verified_manifest(&raw_manifest, &raw_signature, &public_key)?;
    evaluate_manifest(&manifest, current_app_version(), platform_key)
}

fn sanitize_asset_filename(url: &str, platform_key: &str) -> Result<String, UpdateError> {
    validate_release_url(url)?;
    let name = url
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .ok_or_else(|| UpdateError::InvalidUrl("Kein Dateiname in der Download-URL.".into()))?;
    if name.contains("..")
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(UpdateError::InvalidUrl(
            "Dateiname in der Download-URL enthält unzulässige Zeichen.".into(),
        ));
    }
    let expected_ext = if platform_key.starts_with("windows-") {
        ".exe"
    } else if platform_key.starts_with("macos-") {
        ".dmg"
    } else {
        return Err(UpdateError::UnsupportedPlatform(platform_key.to_owned()));
    };
    if !name.to_ascii_lowercase().ends_with(expected_ext) {
        return Err(UpdateError::InvalidManifest(format!(
            "Dateiname '{name}' hat nicht die erwartete Endung '{expected_ext}'."
        )));
    }
    Ok(name.to_owned())
}

/// Streams `reader` into a temporary file inside `dest_dir`, verifies exact byte count and
/// SHA-256 checksum, and only persists to `dest_dir/<filename>` if both checks pass.
/// On any failure the temporary file is automatically removed.
pub fn stream_to_verified_file<R, F>(
    mut reader: R,
    expected_sha256: &str,
    expected_size: u64,
    dest_dir: &Path,
    filename: &str,
    mut on_progress: F,
) -> Result<PathBuf, UpdateError>
where
    R: Read,
    F: FnMut(u64, u64),
{
    let expected_sha256 = validate_sha256_hex(expected_sha256)?;
    if expected_size == 0 || expected_size > MAX_INSTALLER_BYTES {
        return Err(UpdateError::SizeLimitExceeded {
            limit: MAX_INSTALLER_BYTES,
        });
    }
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
    {
        return Err(UpdateError::InvalidUrl("Ungültiger Zieldateiname.".into()));
    }
    fs::create_dir_all(dest_dir).map_err(|e| UpdateError::Io(e.to_string()))?;
    let mut temp = tempfile::NamedTempFile::new_in(dest_dir).map_err(|e| {
        UpdateError::Io(format!("Temporäre Datei konnte nicht erstellt werden: {e}"))
    })?;
    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;
    let mut last_reported: u64 = 0;
    let mut chunk = [0u8; 64 * 1024];
    on_progress(0, expected_size);
    loop {
        let read = reader
            .read(&mut chunk)
            .map_err(|e| UpdateError::Network(format!("Download unterbrochen: {e}")))?;
        if read == 0 {
            break;
        }
        downloaded = downloaded.saturating_add(read as u64);
        if downloaded > MAX_INSTALLER_BYTES {
            return Err(UpdateError::SizeLimitExceeded {
                limit: MAX_INSTALLER_BYTES,
            });
        }
        if downloaded > expected_size {
            return Err(UpdateError::SizeMismatch {
                expected: expected_size,
                actual: downloaded,
            });
        }
        let slice = &chunk[..read];
        hasher.update(slice);
        temp.write_all(slice)
            .map_err(|e| UpdateError::Io(format!("Schreibfehler beim Download: {e}")))?;
        if downloaded.saturating_sub(last_reported) >= 128 * 1024 || downloaded == expected_size {
            last_reported = downloaded;
            on_progress(downloaded, expected_size);
        }
    }
    if downloaded != expected_size {
        return Err(UpdateError::SizeMismatch {
            expected: expected_size,
            actual: downloaded,
        });
    }
    let actual_sha256 = encode_hex(hasher.finalize().as_slice());
    if actual_sha256 != expected_sha256 {
        return Err(UpdateError::ChecksumMismatch {
            expected: expected_sha256,
            actual: actual_sha256,
        });
    }
    temp.as_file()
        .sync_all()
        .map_err(|e| UpdateError::Io(e.to_string()))?;
    let final_path = dest_dir.join(filename);
    temp.persist(&final_path)
        .map_err(|e| UpdateError::Io(format!("Datei konnte nicht gespeichert werden: {e}")))?;
    Ok(final_path)
}

pub fn default_download_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join("updates")
}

fn cleanup_previous_downloads(dir: &Path, keep_filename: Option<&str>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(keep) = keep_filename
            && path.file_name().and_then(|name| name.to_str()) == Some(keep)
        {
            continue;
        }
        let _ = fs::remove_file(path);
    }
}

fn verify_existing_file(path: &Path, expected_sha256: &str, expected_size: u64) -> bool {
    let Ok(expected_sha256) = validate_sha256_hex(expected_sha256) else {
        return false;
    };
    if expected_size == 0 || expected_size > MAX_INSTALLER_BYTES {
        return false;
    }
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let Ok(metadata) = file.metadata() else {
        return false;
    };
    if !metadata.is_file() || metadata.len() != expected_size {
        return false;
    }
    let mut hasher = Sha256::new();
    let mut total: u64 = 0;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let Ok(read) = file.read(&mut buf) else {
            return false;
        };
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > expected_size {
            return false;
        }
        hasher.update(&buf[..read]);
    }
    total == expected_size && encode_hex(hasher.finalize().as_slice()) == expected_sha256
}

/// Downloads and verifies the installer for `info` into `dest_dir`.
/// Reuses an already-downloaded installer in `dest_dir` only if both its exact byte size
/// and SHA-256 digest match the signed manifest.
pub fn download_verified_installer<F>(
    info: &UpdateInfo,
    dest_dir: &Path,
    mut on_progress: F,
) -> Result<PathBuf, UpdateError>
where
    F: FnMut(u64, u64),
{
    let filename = sanitize_asset_filename(&info.asset.url, &info.platform_key)?;
    fs::create_dir_all(dest_dir).map_err(|e| UpdateError::Io(e.to_string()))?;
    let existing_path = dest_dir.join(&filename);
    if verify_existing_file(&existing_path, &info.asset.sha256, info.asset.size_bytes) {
        cleanup_previous_downloads(dest_dir, Some(&filename));
        on_progress(info.asset.size_bytes, info.asset.size_bytes);
        return Ok(existing_path);
    }
    cleanup_previous_downloads(dest_dir, None);
    let agent = build_http_agent(None, DOWNLOAD_RECV_TIMEOUT);
    let (response, _) = get_with_safe_redirects(&agent, &info.asset.url, false)?;
    stream_to_verified_file(
        response.into_body().into_reader(),
        &info.asset.sha256,
        info.asset.size_bytes,
        dest_dir,
        &filename,
        on_progress,
    )
}

/// Opens the verified GitHub Release notes URL in the system's default browser.
pub fn open_release_notes(url: &str) -> Result<(), UpdateError> {
    validate_release_url(url)?;
    #[cfg(target_os = "windows")]
    {
        shell_execute_open(url, None)
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| UpdateError::LaunchFailed(e.to_string()))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| UpdateError::LaunchFailed(e.to_string()))
    }
}

/// Launches the already-verified installer artifact for the current platform:
/// - Windows: launches `Druckwandel-*-Setup.exe` with `/AUTOUPDATE /WAITPID=<current pid>`
///   via `ShellExecuteW` so UAC elevation works cleanly and user cancellation is detected.
/// - macOS: opens the verified `.dmg` via `open <dmg>`.
pub fn launch_installer(installer_path: &Path) -> Result<(), UpdateError> {
    let metadata = fs::metadata(installer_path)
        .map_err(|e| UpdateError::LaunchFailed(format!("Installer nicht gefunden: {e}")))?;
    if !metadata.is_file() {
        return Err(UpdateError::LaunchFailed(
            "Installer-Pfad ist keine reguläre Datei.".into(),
        ));
    }
    #[cfg(target_os = "windows")]
    {
        let ext = installer_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if !ext.eq_ignore_ascii_case("exe") {
            return Err(UpdateError::LaunchFailed(
                "Windows-Installer muss eine .exe-Datei sein.".into(),
            ));
        }
        let target = installer_path.to_str().ok_or_else(|| {
            UpdateError::LaunchFailed("Installer-Pfad enthält ungültiges UTF-8.".into())
        })?;
        let params = format!("/AUTOUPDATE /WAITPID={}", std::process::id());
        shell_execute_open(target, Some(&params))
    }
    #[cfg(target_os = "macos")]
    {
        let ext = installer_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if !ext.eq_ignore_ascii_case("dmg") {
            return Err(UpdateError::LaunchFailed(
                "macOS-Paket muss eine .dmg-Datei sein.".into(),
            ));
        }
        std::process::Command::new("open")
            .arg(installer_path)
            .spawn()
            .map(|_| ())
            .map_err(|e| UpdateError::LaunchFailed(e.to_string()))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = installer_path;
        Err(UpdateError::UnsupportedPlatform(
            std::env::consts::OS.to_owned(),
        ))
    }
}

#[cfg(target_os = "windows")]
fn shell_execute_open(target: &str, parameters: Option<&str>) -> Result<(), UpdateError> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            hwnd: *mut core::ffi::c_void,
            lp_operation: *const u16,
            lp_file: *const u16,
            lp_parameters: *const u16,
            lp_directory: *const u16,
            n_show_cmd: i32,
        ) -> isize;
    }

    fn to_wide(value: &str) -> Vec<u16> {
        OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    const SW_SHOWNORMAL: i32 = 1;
    let verb = to_wide("open");
    let file = to_wide(target);
    let params = parameters.map(to_wide);
    let params_ptr = params.as_ref().map_or(ptr::null(), |buf| buf.as_ptr());

    // SAFETY: All non-null pointers reference NUL-terminated UTF-16 buffers that stay
    // alive for the duration of the synchronous ShellExecuteW call.
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            params_ptr,
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result > 32 {
        Ok(())
    } else if result == 5 {
        Err(UpdateError::LaunchFailed(
            "Benutzerkontensteuerung (UAC) wurde abgebrochen oder verweigert.".into(),
        ))
    } else {
        Err(UpdateError::LaunchFailed(format!(
            "ShellExecuteW fehlgeschlagen (Code {result})."
        )))
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / MIB).replace('.', ",")
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn test_keypair() -> (SigningKey, [u8; 32]) {
        let seed = [0x42u8; 32];
        let signing = SigningKey::from_bytes(&seed);
        let public = signing.verifying_key().to_bytes();
        (signing, public)
    }

    fn sample_manifest_json(version: &str, sha256: &str, size_bytes: u64) -> Vec<u8> {
        let payload = serde_json::json!({
            "version": version,
            "published_at": "2026-11-25T12:00:00Z",
            "notes_url": format!("https://github.com/Lauorez/druckwandel/releases/tag/v{version}"),
            "platforms": {
                "windows-x86_64": {
                    "url": format!("https://github.com/Lauorez/druckwandel/releases/download/v{version}/Druckwandel-{version}-x64-Setup.exe"),
                    "sha256": sha256,
                    "size_bytes": size_bytes
                },
                "macos-aarch64": {
                    "url": format!("https://github.com/Lauorez/druckwandel/releases/download/v{version}/Druckwandel-{version}-arm64.dmg"),
                    "sha256": sha256,
                    "size_bytes": size_bytes
                },
                "macos-x86_64": {
                    "url": format!("https://github.com/Lauorez/druckwandel/releases/download/v{version}/Druckwandel-{version}-x86_64.dmg"),
                    "sha256": sha256,
                    "size_bytes": size_bytes
                }
            }
        });
        serde_json::to_vec_pretty(&payload).unwrap()
    }

    #[test]
    fn placeholder_public_key_fails_explicitly_without_bypass() {
        assert_eq!(
            parse_public_key_hex("REPLACE_WITH_64_HEX_CHAR_ED25519_PUBLIC_KEY"),
            Err(UpdateError::PublicKeyNotConfigured)
        );
        assert_eq!(
            parse_public_key_hex(""),
            Err(UpdateError::PublicKeyNotConfigured)
        );
        assert_eq!(
            parse_public_key_hex(&"0".repeat(64)),
            Err(UpdateError::PublicKeyNotConfigured)
        );
        let (_, pub_bytes) = test_keypair();
        assert_eq!(
            parse_public_key_hex(&encode_hex(&pub_bytes)).unwrap(),
            pub_bytes
        );
        assert!(configured_public_key().is_ok());
    }

    #[test]
    fn ed25519_signature_accepts_valid_and_rejects_tampered_manifest() {
        let (signing, public) = test_keypair();
        let sha = crate::file_io::sha256_hex(b"installer-binary");
        let raw_manifest = sample_manifest_json("0.5.0", &sha, 16);
        let sig = signing.sign(&raw_manifest);
        let sig_hex = format!("{}\n", encode_hex(&sig.to_bytes()));

        // Both hex-encoded and raw 64-byte signatures succeed.
        let parsed_hex =
            parse_verified_manifest(&raw_manifest, sig_hex.as_bytes(), &public).unwrap();
        assert_eq!(parsed_hex.version, "0.5.0");
        assert!(parse_verified_manifest(&raw_manifest, &sig.to_bytes(), &public).is_ok());

        // Single-byte tampering in manifest must fail BEFORE JSON parsing.
        let mut tampered_manifest = raw_manifest.clone();
        tampered_manifest[5] ^= 0x01;
        assert!(matches!(
            parse_verified_manifest(&tampered_manifest, sig_hex.as_bytes(), &public),
            Err(UpdateError::InvalidSignature(_))
        ));

        // Tampered signature must fail.
        let mut tampered_sig = sig.to_bytes();
        tampered_sig[0] ^= 0xFF;
        assert!(matches!(
            parse_verified_manifest(&raw_manifest, &tampered_sig, &public),
            Err(UpdateError::InvalidSignature(_))
        ));

        // Wrong public key must fail.
        let other_public = SigningKey::from_bytes(&[0x99u8; 32])
            .verifying_key()
            .to_bytes();
        assert!(matches!(
            parse_verified_manifest(&raw_manifest, sig_hex.as_bytes(), &other_public),
            Err(UpdateError::InvalidSignature(_))
        ));

        // Oversized manifest is rejected.
        let oversized = vec![b' '; (MAX_MANIFEST_BYTES + 1) as usize];
        assert_eq!(
            verify_manifest_signature(&oversized, &sig.to_bytes(), &public),
            Err(UpdateError::SizeLimitExceeded {
                limit: MAX_MANIFEST_BYTES
            })
        );
    }

    #[test]
    fn semver_comparison_handles_newer_equal_older_and_prerelease() {
        assert!(is_newer_version("0.4.1", "0.4.2").unwrap());
        assert!(is_newer_version("0.4.1", "0.5.0").unwrap());
        assert!(is_newer_version("0.4.1", "1.0.0").unwrap());
        assert!(!is_newer_version("0.4.1", "0.4.1").unwrap());
        assert!(!is_newer_version("0.4.1", "0.4.0").unwrap());
        assert!(!is_newer_version("0.5.0", "0.4.9").unwrap());

        // Prerelease rules per SemVer 2.0.0:
        assert!(is_newer_version("0.5.0-beta.1", "0.5.0").unwrap());
        assert!(is_newer_version("0.5.0-beta.1", "0.5.0-beta.2").unwrap());
        assert!(!is_newer_version("0.5.0", "0.5.0-rc.1").unwrap());
        assert!(is_newer_version("0.4.1", "0.5.0-rc.1").unwrap());

        assert!(is_newer_version("0.4.1", "not-a-semver").is_err());
    }

    #[test]
    fn evaluate_manifest_reports_update_or_up_to_date() {
        let (signing, public) = test_keypair();
        let sha = crate::file_io::sha256_hex(b"installer");
        let raw = sample_manifest_json("0.5.0", &sha, 9);
        let sig = signing.sign(&raw).to_bytes();
        let manifest = parse_verified_manifest(&raw, &sig, &public).unwrap();

        match evaluate_manifest(&manifest, "0.4.1", "windows-x86_64").unwrap() {
            UpdateCheckOutcome::UpdateAvailable(info) => {
                assert_eq!(info.version, "0.5.0");
                assert_eq!(info.platform_key, "windows-x86_64");
                assert!(info.asset.url.ends_with("-x64-Setup.exe"));
            }
            other => panic!("expected UpdateAvailable, got {other:?}"),
        }

        assert_eq!(
            evaluate_manifest(&manifest, "0.5.0", "windows-x86_64").unwrap(),
            UpdateCheckOutcome::UpToDate {
                current_version: "0.5.0".into()
            }
        );
        assert_eq!(
            evaluate_manifest(&manifest, "0.6.0", "macos-aarch64").unwrap(),
            UpdateCheckOutcome::UpToDate {
                current_version: "0.6.0".into()
            }
        );
        assert!(matches!(
            evaluate_manifest(&manifest, "0.4.1", "linux-x86_64"),
            Err(UpdateError::UnsupportedPlatform(_))
        ));
    }

    #[test]
    fn url_allowlist_rejects_http_foreign_hosts_and_tricks() {
        assert!(
            validate_release_url(
                "https://github.com/Lauorez/druckwandel/releases/download/v0.5.0/Druckwandel-0.5.0-x64-Setup.exe"
            )
            .is_ok()
        );
        assert!(
            validate_release_url("https://github.com/Lauorez/druckwandel/releases/tag/v0.5.0")
                .is_ok()
        );
        assert!(
            validate_release_url(
                "https://github.com/Lauorez/druckwandel/releases/latest/download/latest.json"
            )
            .is_ok()
        );

        // HTTP rejected
        assert!(
            validate_release_url(
                "http://github.com/Lauorez/druckwandel/releases/latest/download/latest.json"
            )
            .is_err()
        );
        // Foreign domains and wrong repositories rejected
        assert!(
            validate_release_url(
                "https://evil.example/Lauorez/druckwandel/releases/download/v0.5.0/setup.exe"
            )
            .is_err()
        );
        assert!(
            validate_release_url(
                "https://github.com/other/repo/releases/download/v0.5.0/setup.exe"
            )
            .is_err()
        );
        // Userinfo, custom port, path traversal, and query strings rejected
        assert!(
            validate_release_url(
                "https://github.com@evil.example/Lauorez/druckwandel/releases/tag/v0.5.0"
            )
            .is_err()
        );
        assert!(
            validate_release_url("https://github.com:444/Lauorez/druckwandel/releases/tag/v0.5.0")
                .is_err()
        );
        assert!(
            validate_release_url(
                "https://github.com/Lauorez/druckwandel/releases/../../other/repo/setup.exe"
            )
            .is_err()
        );

        // Case-insensitive GitHub repo path normalization on redirects is allowed
        assert!(
            validate_release_url(
                "https://github.com/lauorez/druckwandel/releases/download/v1.0.0/latest.json"
            )
            .is_ok()
        );
        // Dot segments and percent-encoded path traversal are rejected
        assert!(
            validate_release_url(
                "https://github.com/Lauorez/druckwandel/releases/./download/v0.5.0/setup.exe"
            )
            .is_err()
        );
        assert!(
            validate_release_url(
                "https://github.com/Lauorez/druckwandel/releases/%2e%2e/other/setup.exe"
            )
            .is_err()
        );
        assert_eq!(
            derive_signature_url(
                "https://github.com/Lauorez/druckwandel/releases/download/v1.0.0/latest.json"
            ),
            "https://github.com/Lauorez/druckwandel/releases/download/v1.0.0/latest.json.sig"
        );
        assert_eq!(
            derive_signature_url("https://evil.example/latest.json"),
            LATEST_SIGNATURE_URL
        );

        // Redirect allowlist allows official GitHub asset CDN over HTTPS only
        assert!(
            validate_redirect_url(
                "https://objects.githubusercontent.com/github-production-release-asset/123?sig=abc"
            )
            .is_ok()
        );
        assert!(
            validate_redirect_url(
                "https://release-assets.githubusercontent.com/github-production-release-asset/123?sig=abc"
            )
            .is_ok()
        );
        assert!(
            validate_redirect_url("http://objects.githubusercontent.com/file?sig=abc").is_err()
        );
        assert!(validate_redirect_url("https://attacker.example/setup.exe").is_err());
    }

    #[test]
    fn streaming_download_verifies_sha256_and_cleans_up_on_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let payload = b"Druckwandel installer test bytes 0123456789";
        let good_hash = crate::file_io::sha256_hex(payload);

        let mut progress_events = Vec::new();
        let saved = stream_to_verified_file(
            std::io::Cursor::new(payload),
            &good_hash,
            payload.len() as u64,
            dir.path(),
            "Druckwandel-0.5.0-x64-Setup.exe",
            |done, total| progress_events.push((done, total)),
        )
        .unwrap();
        assert_eq!(fs::read(&saved).unwrap(), payload);
        assert_eq!(progress_events.first(), Some(&(0, payload.len() as u64)));
        assert_eq!(
            progress_events.last(),
            Some(&(payload.len() as u64, payload.len() as u64))
        );

        // Mismatched SHA-256 must fail and leave no temporary file behind.
        let bad_dir = tempfile::tempdir().unwrap();
        let wrong_hash = "f".repeat(64);
        let err = stream_to_verified_file(
            std::io::Cursor::new(payload),
            &wrong_hash,
            payload.len() as u64,
            bad_dir.path(),
            "Druckwandel-0.5.0-x64-Setup.exe",
            |_, _| {},
        )
        .unwrap_err();
        assert!(matches!(err, UpdateError::ChecksumMismatch { .. }));
        assert_eq!(fs::read_dir(bad_dir.path()).unwrap().count(), 0);

        // Size mismatch must fail and leave no temporary file behind.
        let err = stream_to_verified_file(
            std::io::Cursor::new(payload),
            &good_hash,
            payload.len() as u64 + 10,
            bad_dir.path(),
            "Druckwandel-0.5.0-x64-Setup.exe",
            |_, _| {},
        )
        .unwrap_err();
        assert!(matches!(err, UpdateError::SizeMismatch { .. }));
        assert_eq!(fs::read_dir(bad_dir.path()).unwrap().count(), 0);

        // Verified existing file matches size + SHA-256; tampered file fails verification.
        assert!(verify_existing_file(
            &saved,
            &good_hash,
            payload.len() as u64
        ));
        assert!(!verify_existing_file(
            &saved,
            &wrong_hash,
            payload.len() as u64
        ));
        assert!(!verify_existing_file(
            &saved,
            &good_hash,
            payload.len() as u64 + 1
        ));
        let stale = dir.path().join("Druckwandel-0.4.0-x64-Setup.exe");
        fs::write(&stale, b"old").unwrap();
        cleanup_previous_downloads(dir.path(), Some("Druckwandel-0.5.0-x64-Setup.exe"));
        assert!(saved.exists());
        assert!(!stale.exists());
    }

    #[test]
    fn auto_check_cooldown_enforces_24_hour_window() {
        let now = 1_800_000_000;
        assert!(!should_auto_check(false, None, now));
        assert!(!should_auto_check(false, Some(0), now));
        assert!(should_auto_check(true, None, now));
        assert!(!should_auto_check(true, Some(now), now));
        assert!(!should_auto_check(
            true,
            Some(now - AUTO_CHECK_COOLDOWN_SECS + 1),
            now
        ));
        assert!(should_auto_check(
            true,
            Some(now - AUTO_CHECK_COOLDOWN_SECS),
            now
        ));
        assert!(should_auto_check(
            true,
            Some(now - AUTO_CHECK_COOLDOWN_SECS - 3600),
            now
        ));
        // If system clock moved backwards past last_checked_unix, allow re-check.
        assert!(should_auto_check(true, Some(now + 60), now));
    }

    #[test]
    fn verifies_rfc8032_test_vector_1_for_script_interoperability() {
        // RFC 8032 Section 7.1 TEST 1 (empty message)
        let public = parse_public_key_hex(
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
        )
        .unwrap();
        let sig_hex = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155\
                       5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";
        let key = VerifyingKey::from_bytes(&public).unwrap();
        let sig = Signature::from_bytes(&decode_signature(sig_hex.as_bytes()).unwrap());
        assert!(key.verify_strict(b"", &sig).is_ok());
    }
}
