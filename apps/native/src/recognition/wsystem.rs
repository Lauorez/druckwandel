//! Optional W System Cloud invoice recognition (`POST /einvoice/extract`)
//! and quota check (`GET /billing/usage`).
//!
//! Verified API contract sources:
//! - https://api.wsystem.dev/openapi.json
//! - https://wsystem.dev/pdf-to-xrechnung
//! - https://wsystem.dev/docs
//! - https://api.wsystem.dev/einvoice/sample
//! - Live verification against `GET /billing/usage` and
//!   `POST /einvoice/extract?format=xrechnung` on 2026-11-26.
//!
//! Note on OpenAPI schema:
//! Both `POST /einvoice/extract` and `GET /billing/usage` declare
//! `responses.200.content["application/json"].schema = {}` in OpenAPI.
//! Live verification confirmed top-level `invoice`, 0-based `fields[]`,
//! `vat_breakdown[]`, `totals`, `review` (`low_confidence`, `model_uncertainties`,
//! `input_error`, `blocked`), `arithmetic_check` (per-line + BT-106/110/112/115
//! rows), `validation`, and UBL 2.1 `xml`. The mapper handles all of these and
//! keeps defensive fallbacks (`draft`/`extracted`, 1-based `fields[]`, XML).

use super::{
    CONTRACT_VERSION, FieldProposal, LineProposal, RecognitionMethod, RecognitionProvider,
    RecognitionRequest, RecognitionResult,
};
use crate::extraction::{TextLine, reconstruct_lines};
use crate::invoice::{AllowanceCharge, InvoiceLine, Tax};
use crate::secrets::{ApiKey, SecretStore};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

pub const WSYSTEM_API_BASE: &str = "https://api.wsystem.dev";
pub const WSYSTEM_FREE_KEY_URL: &str = "https://api.wsystem.dev/account/free-key?ref=druckwandel";
pub const WSYSTEM_BILLING_URL: &str = "https://api.wsystem.dev/billing/druckwandel";
pub const WSYSTEM_DPA_URL: &str = "https://wsystem.dev/legal/dpa";
pub const WSYSTEM_DOCS_URL: &str = "https://wsystem.dev/pdf-to-xrechnung";
pub const MAX_WSYSTEM_PDF_BYTES: u64 = 10 * 1024 * 1024;
pub const UNITS_PER_EXTRACTION: u64 = 80;

const MAX_USAGE_RESPONSE_BYTES: u64 = 256 * 1024;
const MAX_EXTRACT_RESPONSE_BYTES: u64 = 5 * 1024 * 1024;
const USAGE_TIMEOUT: Duration = Duration::from_secs(15);
const EXTRACT_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsageInfo {
    pub plan: Option<String>,
    pub quota: Option<u64>,
    pub used: Option<u64>,
    pub remaining: Option<u64>,
    pub period: Option<String>,
    pub extract_unit_weight: Option<u64>,
    pub request_id: Option<String>,
}

impl UsageInfo {
    pub fn summary(&self) -> String {
        let weight = self
            .extract_unit_weight
            .filter(|&w| w > 0)
            .unwrap_or(UNITS_PER_EXTRACTION);
        let mut parts = vec!["API-Schlüssel gültig".to_owned()];
        if let Some(plan) = &self.plan {
            parts.push(format!("Tarif: {plan}"));
        }
        if let Some(period) = &self.period {
            parts.push(format!("Zeitraum: {period}"));
        }
        match (self.remaining, self.quota) {
            (Some(remaining), Some(quota)) => {
                let est = remaining / weight;
                parts.push(format!(
                    "{remaining} von {quota} Einheiten verfügbar (ca. {est} Rechnungen à {weight} Einheiten)"
                ));
            }
            (Some(remaining), None) => {
                let est = remaining / weight;
                parts.push(format!(
                    "{remaining} Einheiten verfügbar (ca. {est} Rechnungen à {weight} Einheiten)"
                ));
            }
            (None, Some(quota)) => {
                if let Some(used) = self.used {
                    parts.push(format!("{used} von {quota} Einheiten verbraucht"));
                } else {
                    parts.push(format!("Kontingent: {quota} Einheiten"));
                }
            }
            (None, None) => {
                if let Some(used) = self.used {
                    parts.push(format!("{used} Einheiten verbraucht"));
                }
            }
        }
        if let Some(request_id) = &self.request_id {
            parts.push(format!("Request-ID: {request_id}"));
        }
        parts.join(" · ")
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ExtractPayload {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub review_required: Option<bool>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub fields: Vec<ExtractedField>,
    #[serde(default)]
    pub review: Option<ExtractReview>,
    #[serde(default)]
    pub arithmetic_check: Option<ArithmeticCheck>,
    #[serde(default)]
    pub validation: Option<ExtractValidation>,
    #[serde(default)]
    pub vat_breakdown: Vec<VatBreakdownEntry>,
    #[serde(default)]
    pub totals: Option<Value>,
    #[serde(default)]
    pub xml: Option<String>,
    #[serde(default)]
    pub disclaimer: Option<String>,
    #[serde(default)]
    pub invoice: Option<Value>,
    #[serde(default)]
    pub draft: Option<Value>,
    #[serde(default)]
    pub extracted: Option<Value>,
    #[serde(default)]
    pub request_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct VatBreakdownEntry {
    #[serde(default)]
    pub vat_category: Option<String>,
    #[serde(default)]
    pub exemption_reason_text: Option<String>,
    #[serde(default)]
    pub exemption_reason_code: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExtractedField {
    pub path: String,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub checks: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ExtractReview {
    #[serde(default)]
    pub low_confidence: Vec<String>,
    #[serde(default)]
    pub model_uncertainties: Vec<Value>,
    #[serde(default)]
    pub input_error: Option<String>,
    #[serde(default)]
    pub blocked: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ArithmeticCheck {
    #[serde(default)]
    pub all_match: Option<bool>,
    #[serde(default)]
    pub rows: Vec<ArithmeticRow>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ArithmeticRow {
    #[serde(default)]
    pub check: String,
    #[serde(default)]
    pub computed: Value,
    #[serde(default)]
    pub printed: Value,
    #[serde(default, rename = "match")]
    pub matched: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ExtractValidation {
    #[serde(default)]
    pub valid: Option<bool>,
    #[serde(default)]
    pub errors: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct ExtractResponse {
    pub payload: ExtractPayload,
    pub request_id: Option<String>,
    pub units_used: Option<u64>,
    pub units_remaining: Option<u64>,
    pub idempotent_replayed: bool,
}

#[derive(Clone)]
pub struct WSystemClient {
    base_url: String,
}

impl Default for WSystemClient {
    fn default() -> Self {
        Self::new()
    }
}

impl WSystemClient {
    pub fn new() -> Self {
        Self {
            base_url: WSYSTEM_API_BASE.to_owned(),
        }
    }

    /// Create a client targeting a custom base URL.
    /// Non-loopback endpoints must use HTTPS so API keys and invoices are never sent in plaintext.
    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self, String> {
        let base = base_url.into().trim().trim_end_matches('/').to_owned();
        if !is_allowed_base_url(&base) {
            return Err(
                "W-System-Endpunkt muss HTTPS verwenden (HTTP ist nur für lokale Test-Mocks zulässig)."
                    .into(),
            );
        }
        Ok(Self { base_url: base })
    }

    fn agent(timeout: Duration) -> ureq::Agent {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .max_redirects(0)
            .http_status_as_error(false)
            .build();
        ureq::Agent::new_with_config(config)
    }

    fn user_agent() -> String {
        format!("druckwandel-native/{}", env!("CARGO_PKG_VERSION"))
    }

    pub fn check_usage(&self, api_key: &ApiKey) -> Result<UsageInfo, String> {
        let client_request_id = format!("dw-{}", Uuid::new_v4());
        let url = format!("{}/billing/usage", self.base_url);
        let agent = Self::agent(USAGE_TIMEOUT);
        let response = agent
            .get(&url)
            .header("X-API-Key", api_key.expose())
            .header("Accept", "application/json")
            .header("X-Request-Id", &client_request_id)
            .header("User-Agent", &Self::user_agent())
            .call()
            .map_err(|error| map_transport_error(error, api_key, &client_request_id))?;

        let status = response.status().as_u16();
        let header_request_id = header_str(&response, "x-request-id");
        let header_used = header_u64(&response, "x-units-used");
        let header_remaining = header_u64(&response, "x-units-remaining");
        let body = read_bounded_body(response, MAX_USAGE_RESPONSE_BYTES, api_key)?;

        if !(200..300).contains(&status) {
            return Err(format_api_error(
                status,
                &body,
                header_request_id.as_deref().or(Some(&client_request_id)),
                api_key,
            ));
        }

        let json: Value = serde_json::from_slice(&body).map_err(|_| {
            format_with_request_id(
                "W System lieferte eine ungültige JSON-Antwort für /billing/usage.",
                header_request_id.as_deref().or(Some(&client_request_id)),
            )
        })?;

        let request_id = header_request_id
            .or_else(|| json_string(json.get("request_id")))
            .or(Some(client_request_id));
        let plan = json_string(json.get("plan")).or_else(|| json_string(json.get("tier")));
        let period = json_string(json.get("period"));
        let extract_unit_weight = json
            .get("unit_weights")
            .and_then(Value::as_object)
            .and_then(|weights| json_u64(weights.get("/einvoice/extract")))
            .filter(|&w| w > 0);
        let quota = json_u64(json.get("quota"))
            .or_else(|| json_u64(json.get("monthly_quota")))
            .or_else(|| json_u64(json.get("units_limit")));
        let used = json_u64(json.get("used"))
            .or_else(|| json_u64(json.get("units_used")))
            .or(header_used);
        let remaining = json_u64(json.get("remaining"))
            .or_else(|| json_u64(json.get("units_remaining")))
            .or(header_remaining)
            .or_else(|| match (quota, used) {
                (Some(q), Some(u)) => Some(q.saturating_sub(u)),
                _ => None,
            });

        Ok(UsageInfo {
            plan,
            quota,
            used,
            remaining,
            period,
            extract_unit_weight,
            request_id,
        })
    }

    pub fn extract_pdf(
        &self,
        api_key: &ApiKey,
        pdf_bytes: &[u8],
    ) -> Result<ExtractResponse, String> {
        if pdf_bytes.len() < 5 || &pdf_bytes[..5] != b"%PDF-" {
            return Err("Die Datei ist keine gültige PDF-Rechnung.".into());
        }
        if pdf_bytes.len() as u64 > MAX_WSYSTEM_PDF_BYTES {
            return Err(
                "Die PDF-Datei überschreitet das W-System-Limit von 10 MB. Bitte Schemaerkennung verwenden."
                    .into(),
            );
        }

        let client_request_id = format!("dw-{}", Uuid::new_v4());
        let idempotency_key = format!("dw-extract-{}", Uuid::new_v4());
        let url = format!("{}/einvoice/extract?format=xrechnung", self.base_url);
        let agent = Self::agent(EXTRACT_TIMEOUT);
        let response = agent
            .post(&url)
            .header("X-API-Key", api_key.expose())
            .header("Accept", "application/json")
            .header("X-Request-Id", &client_request_id)
            .header("Idempotency-Key", &idempotency_key)
            .header("User-Agent", &Self::user_agent())
            .content_type("application/pdf")
            .send(pdf_bytes)
            .map_err(|error| map_transport_error(error, api_key, &client_request_id))?;

        let status = response.status().as_u16();
        let header_request_id = header_str(&response, "x-request-id");
        let units_used = header_u64(&response, "x-units-used");
        let units_remaining = header_u64(&response, "x-units-remaining");
        let idempotent_replayed = header_str(&response, "idempotent-replayed")
            .is_some_and(|v| v.eq_ignore_ascii_case("true"));
        let body = read_bounded_body(response, MAX_EXTRACT_RESPONSE_BYTES, api_key)?;

        if !(200..300).contains(&status) {
            return Err(format_api_error(
                status,
                &body,
                header_request_id.as_deref().or(Some(&client_request_id)),
                api_key,
            ));
        }

        let payload: ExtractPayload = serde_json::from_slice(&body).map_err(|_| {
            format_with_request_id(
                "W System lieferte eine unerwartete Antwort für /einvoice/extract.",
                header_request_id.as_deref().or(Some(&client_request_id)),
            )
        })?;

        let request_id = header_request_id
            .or_else(|| payload.request_id.clone())
            .or(Some(client_request_id));

        Ok(ExtractResponse {
            payload,
            request_id,
            units_used,
            units_remaining,
            idempotent_replayed,
        })
    }
}

/// Optional `RecognitionProvider` backed by W System Cloud (`POST /einvoice/extract`).
pub struct WSystemRecognizer {
    client: WSystemClient,
    secrets: Arc<dyn SecretStore>,
    consent_accepted: bool,
    last_units_remaining: Arc<Mutex<Option<u64>>>,
}

impl WSystemRecognizer {
    pub fn new(
        client: WSystemClient,
        secrets: Arc<dyn SecretStore>,
        consent_accepted: bool,
    ) -> Self {
        Self {
            client,
            secrets,
            consent_accepted,
            last_units_remaining: Arc::new(Mutex::new(None)),
        }
    }

    pub fn last_units_remaining(&self) -> Option<u64> {
        self.last_units_remaining
            .lock()
            .ok()
            .and_then(|guard| *guard)
    }
}

impl RecognitionProvider for WSystemRecognizer {
    fn method(&self) -> RecognitionMethod {
        RecognitionMethod::WSystem
    }

    fn recognize(&self, request: &RecognitionRequest) -> Result<RecognitionResult, String> {
        if !self.consent_accepted {
            return Err(
                "Die Übermittlung an W System ist noch nicht freigegeben. Bitte unter Einstellungen → Erkennung die Übermittlung bestätigen."
                    .into(),
            );
        }
        let api_key = self.secrets.load_wsystem_key()?.ok_or_else(|| {
            "Kein W-System-API-Schlüssel gespeichert. Bitte unter Einstellungen → Erkennung einen API-Schlüssel hinterlegen."
                .to_owned()
        })?;
        let pdf_bytes =
            crate::file_io::read_verified_pdf(&request.source_pdf, &request.document_sha256)?;
        let response = self.client.extract_pdf(&api_key, &pdf_bytes)?;
        if let Ok(mut guard) = self.last_units_remaining.lock() {
            *guard = response.units_remaining;
        }
        Ok(map_extract_response(request, &response))
    }
}

/// Map a W System `/einvoice/extract` response to Druckwandel's `RecognitionResult`.
pub fn map_extract_response(
    request: &RecognitionRequest,
    response: &ExtractResponse,
) -> RecognitionResult {
    let text_lines = reconstruct_lines(&request.pages);
    let mut result = RecognitionResult {
        schema_version: CONTRACT_VERSION,
        document_sha256: request.document_sha256.clone(),
        method: RecognitionMethod::WSystem,
        provider_version: "wsystem-extract-1".into(),
        fields: BTreeMap::new(),
        lines: Vec::new(),
        warnings: Vec::new(),
        request_id: response.request_id.clone(),
        allowances: Vec::new(),
    };

    let payload = &response.payload;
    let structured = payload
        .invoice
        .as_ref()
        .or(payload.draft.as_ref())
        .or(payload.extracted.as_ref())
        .and_then(Value::as_object);

    let mut exemption_reasons = HashMap::<String, String>::new();
    let mut exemption_reason_codes = HashMap::<String, String>::new();
    for entry in &payload.vat_breakdown {
        if let Some(cat) = entry
            .vat_category
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_uppercase)
        {
            if let Some(reason) = entry
                .exemption_reason_text
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                exemption_reasons.insert(cat.clone(), reason.to_owned());
            }
            if let Some(code) = entry
                .exemption_reason_code
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                exemption_reason_codes.insert(cat, code.to_owned());
            }
        }
    }
    let mut partial_lines = BTreeMap::<usize, PartialLine>::new();

    // 1. Read structured `/einvoice/create`-shaped invoice object if present.
    if let Some(inv) = structured {
        if let Some(map) = inv.get("vat_exemption_reason").and_then(Value::as_object) {
            for (cat, reason) in map {
                if let Some(reason_text) = json_string(Some(reason)) {
                    exemption_reasons.insert(cat.trim().to_uppercase(), reason_text);
                }
            }
        }
        ingest_structured_invoice(
            inv,
            &text_lines,
            &mut result,
            &mut partial_lines,
            &exemption_reasons,
        );
    }

    // 2. Overlay verified `fields[]` entries (carrying per-field confidence and checks).
    let has_structured_lines = !partial_lines.is_empty();
    let fields_are_one_based = detect_one_based_lines(&payload.fields, partial_lines.len());
    for field in &payload.fields {
        let confidence = sanitize_confidence(field.confidence.unwrap_or(0.85));
        let path = normalize_field_path(&field.path);
        if let Some((raw_idx, attr)) = parse_line_path(&path) {
            let idx = if has_structured_lines && fields_are_one_based {
                raw_idx.saturating_sub(1)
            } else {
                raw_idx
            };
            let entry = partial_lines.entry(idx).or_default();
            entry.apply_field(attr, &field.value, confidence, &field.checks);
            continue;
        }
        if let Some(cat) = path.strip_prefix("vat_exemption_reason.") {
            if let Some(reason) = json_string(Some(&field.value)) {
                exemption_reasons.insert(cat.trim().to_uppercase(), reason);
            }
            continue;
        }
        if let Some(target_key) = map_scalar_path(&path)
            && let Some(normalized) = normalize_field_value(target_key, &field.value)
        {
            let proposal = build_field_proposal(
                target_key,
                normalized,
                confidence,
                &field.checks,
                &text_lines,
            );
            result.fields.insert(target_key.to_owned(), proposal);
        }
    }

    // 3. Build normalized `LineProposal`s from collected line entries.
    let mut missing_tax_warning = false;
    for (position, (_, partial)) in partial_lines.into_iter().enumerate() {
        if let Some((line_proposal, missing_tax)) = partial.into_line_proposal(
            position + 1,
            &exemption_reasons,
            &exemption_reason_codes,
            &text_lines,
        ) {
            if missing_tax {
                missing_tax_warning = true;
            }
            result.lines.push(line_proposal);
        }
    }
    if missing_tax_warning {
        result.warnings.push(
            "Mindestens eine von W System erkannte Position enthält keinen eindeutigen Steuersatz und muss manuell ergänzt werden."
                .into(),
        );
    }

    // 4. Connect W System arithmetic_check printed totals BEFORE XML/computed fallbacks so
    // Druckwandel's export total check compares against the actual printed PDF totals.
    if let Some(arith) = &payload.arithmetic_check {
        for row in &arith.rows {
            if let Some(printed) = value_to_money(&row.printed)
                && let Some(total_key) = arithmetic_row_total_key(&row.check)
            {
                result
                    .fields
                    .entry(total_key.to_owned())
                    .or_insert_with(|| {
                        let conf = if row.matched == Some(false) {
                            0.55
                        } else {
                            0.90
                        };
                        let checks = if row.matched == Some(false) {
                            vec!["totals_mismatch".to_owned()]
                        } else {
                            vec!["found_in_pdf_text".to_owned()]
                        };
                        build_field_proposal(total_key, printed, conf, &checks, &text_lines)
                    });
            }
            if row.matched == Some(false) {
                let computed = value_to_display(&row.computed).unwrap_or_else(|| "?".into());
                let printed = value_to_display(&row.printed).unwrap_or_else(|| "?".into());
                result.warnings.push(format!(
                    "W System Rechenprüfung ({}): berechnet {computed}, laut PDF gedruckt {printed}. Bitte Positionen und Beträge am Original prüfen.",
                    row.check
                ));
            }
        }
        if arith.all_match == Some(false) && arith.rows.iter().all(|r| r.matched != Some(false)) {
            result.warnings.push(
                "W System meldet eine Abweichung zwischen den berechneten Positionssummen und den gedruckten Beträgen."
                    .into(),
            );
        }
        if let Some(note) = arith
            .note
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            result
                .warnings
                .push(format!("W System Rechenhinweis: {note}"));
        }
    }

    // 5. Fallback to top-level `totals` object if printed totals were not populated.
    if let Some(totals) = payload.totals.as_ref().and_then(Value::as_object) {
        ingest_totals_if_missing(totals, &text_lines, &mut result);
    }

    // 6. Optional UBL 2.1 XML fallback if structured fields or lines are still missing.
    if let Some(xml) = payload.xml.as_deref().filter(|s| !s.trim().is_empty()) {
        ingest_ubl_xml_fallback(xml, &text_lines, &mut result);
    }

    // 7. Surface status, review notices, validation findings, disclaimer, and metadata.
    if payload.status.as_deref() == Some("needs_review_amounts") {
        result.warnings.push(
            "W System Status ‚needs_review_amounts‘: Beträge weichen von den gedruckten Summen ab. Alle Werte bitte sorgfältig prüfen."
                .into(),
        );
    }
    if let Some(review) = &payload.review {
        if let Some(blocked) = review
            .blocked
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            result
                .warnings
                .push(format!("W System Prüfhinweis: {}", blocked.trim()));
        }
        if let Some(input_error) = review
            .input_error
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            result
                .warnings
                .push(format!("W System Eingabehinweis: {}", input_error.trim()));
        }
        if !review.low_confidence.is_empty() {
            let labels = review
                .low_confidence
                .iter()
                .map(|path| format_wsystem_path_label(path))
                .collect::<Vec<_>>()
                .join(", ");
            result.warnings.push(format!(
                "W System markiert folgende Felder mit niedriger Konfidenz zur Prüfung: {labels}."
            ));
        }
        if !review.model_uncertainties.is_empty() {
            let notes = review
                .model_uncertainties
                .iter()
                .filter_map(format_uncertainty)
                .collect::<Vec<_>>()
                .join("; ");
            if !notes.is_empty() {
                result.warnings.push(format!(
                    "W System meldet Unsicherheiten bei der Extraktion: {notes}"
                ));
            }
        }
    }
    if let Some(validation) = &payload.validation
        && validation.valid == Some(false)
        && !validation.errors.is_empty()
    {
        let preview = validation
            .errors
            .iter()
            .take(3)
            .filter_map(format_validation_error)
            .collect::<Vec<_>>()
            .join("; ");
        if !preview.is_empty() {
            result.warnings.push(format!(
                "W System Vorprüfung meldet fehlende oder zu prüfende Angaben (finale Prüfung erfolgt lokal beim Export): {preview}"
            ));
        }
    }
    if response.idempotent_replayed {
        result.warnings.push(
            "Antwort wurde von W System aus dem Idempotenz-Cache wiedergegeben (keine erneute Abrechnung)."
                .into(),
        );
    }
    if let Some(remaining) = response.units_remaining {
        result.warnings.push(format!(
            "W System Restkontingent laut Antwort-Header: {remaining} Einheiten."
        ));
    }
    if let Some(disclaimer) = payload
        .disclaimer
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        result.warnings.push(disclaimer.trim().to_owned());
    } else {
        result.warnings.push(
            "KI-gestützter Entwurf von W System: Alle Felder, Beträge und Positionen vor der Ausgabe am Original-PDF prüfen."
                .into(),
        );
    }
    if result.fields.is_empty() && result.lines.is_empty() {
        result.warnings.push(
            "W System hat keine verwertbaren Rechnungsfelder zurückgegeben. Bitte PDF-Text prüfen oder Daten manuell erfassen."
                .into(),
        );
    }

    result
}

#[derive(Default)]
struct PartialLine {
    name: Option<String>,
    description: Option<String>,
    quantity: Option<String>,
    unit_code: Option<String>,
    net_unit_price: Option<String>,
    net_amount: Option<String>,
    tax_category: Option<String>,
    tax_rate: Option<String>,
    exemption_reason: Option<String>,
    exemption_reason_code: Option<String>,
    allowances: Vec<AllowanceCharge>,
    min_confidence: Option<f32>,
    checks: Vec<String>,
}

impl PartialLine {
    fn note_confidence(&mut self, confidence: f32, checks: &[String]) {
        self.min_confidence = Some(match self.min_confidence {
            Some(existing) => existing.min(confidence),
            None => confidence,
        });
        for check in checks {
            if !self.checks.contains(check) {
                self.checks.push(check.clone());
            }
        }
    }

    fn apply_field(&mut self, attr: &str, value: &Value, confidence: f32, checks: &[String]) {
        self.note_confidence(confidence, checks);
        match attr {
            "name" | "item_name" => {
                if let Some(v) = json_string(Some(value)) {
                    self.name = Some(v);
                }
            }
            "description" | "item_description" => {
                if let Some(v) = json_string(Some(value)) {
                    self.description = Some(v);
                }
            }
            "quantity" => {
                if let Some(v) = value_to_decimal(value) {
                    self.quantity = Some(v);
                }
            }
            "unit" | "unit_code" => {
                if let Some(v) = json_string(Some(value)) {
                    self.unit_code = Some(v.to_uppercase());
                }
            }
            "unit_price" | "unit_price_net" | "net_unit_price" | "price" => {
                if let Some(v) = value_to_decimal(value) {
                    self.net_unit_price = Some(v);
                }
            }
            "line_total" | "line_net" | "net_amount" | "amount" => {
                if let Some(v) = value_to_money(value) {
                    self.net_amount = Some(v);
                }
            }
            "vat_category" | "tax_category" | "tax_category_code" => {
                if let Some(v) = json_string(Some(value)) {
                    let upper = v.to_uppercase();
                    if ["S", "Z", "E", "AE", "K"].contains(&upper.as_str()) {
                        self.tax_category = Some(upper);
                    }
                }
            }
            "vat_rate" | "tax_rate" => {
                if let Some(v) = value_to_decimal(value) {
                    self.tax_rate = Some(v);
                }
            }
            "vat_exemption_reason" | "exemption_reason" => {
                if let Some(v) = json_string(Some(value)) {
                    self.exemption_reason = Some(v);
                }
            }
            "vat_exemption_reason_code" | "exemption_reason_code" => {
                if let Some(v) = json_string(Some(value)) {
                    self.exemption_reason_code = Some(v);
                }
            }
            _ => {}
        }
    }

    fn into_line_proposal(
        self,
        id: usize,
        exemption_reasons: &HashMap<String, String>,
        exemption_reason_codes: &HashMap<String, String>,
        text_lines: &[TextLine],
    ) -> Option<(LineProposal, bool)> {
        let (name, description) = match (self.name, self.description) {
            (Some(name), desc) => (name, desc),
            (None, Some(desc)) => (desc, None),
            (None, None) => (String::new(), None),
        };
        if name.is_empty()
            && self.quantity.is_none()
            && self.net_unit_price.is_none()
            && self.net_amount.is_none()
        {
            return None;
        }

        let quantity = self.quantity.unwrap_or_else(|| "1".into());
        let net_unit_price = self
            .net_unit_price
            .or_else(|| self.net_amount.clone())
            .unwrap_or_else(|| "0.00".into());
        let unit_code = self.unit_code.unwrap_or_else(|| "C62".into());

        let (tax_category, tax_rate, missing_tax) = match (self.tax_category, self.tax_rate) {
            (Some(cat), Some(rate)) => (cat, rate, false),
            (Some(cat), None) if cat != "S" => (cat, "0".into(), false),
            (Some(cat), None) => (cat, String::new(), true),
            (None, Some(rate)) => {
                let parsed = Decimal::from_str(&rate).ok();
                if parsed.is_some_and(|d| d > Decimal::ZERO) {
                    ("S".into(), rate, false)
                } else if parsed.is_some_and(|d| d == Decimal::ZERO) {
                    let cat = if exemption_reasons.contains_key("E") {
                        "E"
                    } else if exemption_reasons.contains_key("AE") {
                        "AE"
                    } else if exemption_reasons.contains_key("K") {
                        "K"
                    } else {
                        "Z"
                    };
                    (cat.into(), "0".into(), false)
                } else {
                    ("S".into(), String::new(), true)
                }
            }
            (None, None) => ("S".into(), String::new(), true),
        };

        let exemption_reason = self
            .exemption_reason
            .or_else(|| exemption_reasons.get(&tax_category).cloned());
        let exemption_reason_code = self
            .exemption_reason_code
            .or_else(|| exemption_reason_codes.get(&tax_category).cloned());
        let tax = Tax {
            category_code: tax_category,
            rate: tax_rate,
            exemption_reason,
            exemption_reason_code,
        };

        let mut allowances = self.allowances;
        for item in &mut allowances {
            item.tax = tax.clone();
        }

        let net_amount = self.net_amount.unwrap_or_else(|| {
            compute_line_net(&quantity, &net_unit_price, &allowances)
                .unwrap_or_else(|| "0.00".into())
        });

        let mut confidence = sanitize_confidence(self.min_confidence.unwrap_or(0.85));
        if missing_tax {
            confidence = confidence.min(0.60);
        }

        let matched = (!name.is_empty())
            .then(|| find_matching_line(text_lines, &name))
            .flatten();
        let (source_token_ids, source_text) = match matched {
            Some(line) => (
                line.tokens.iter().map(|t| t.id.clone()).collect(),
                annotate_source(&line.text, &self.checks),
            ),
            None => (
                Vec::new(),
                annotate_source(&format!("W System Position {id}: {name}"), &self.checks),
            ),
        };

        Some((
            LineProposal {
                line: InvoiceLine {
                    id: id.to_string(),
                    name,
                    description,
                    quantity,
                    unit_code,
                    net_unit_price,
                    tax,
                    allowances,
                },
                net_amount,
                confidence,
                source_token_ids,
                source_text,
            },
            missing_tax,
        ))
    }
}

fn ingest_structured_invoice(
    inv: &serde_json::Map<String, Value>,
    text_lines: &[TextLine],
    result: &mut RecognitionResult,
    partial_lines: &mut BTreeMap<usize, PartialLine>,
    exemption_reasons: &HashMap<String, String>,
) {
    let default_conf = 0.85_f32;
    let empty_checks: &[String] = &[];
    let mut insert_scalar = |target_key: &str, value: Option<&Value>| {
        if let Some(val) = value
            && let Some(normalized) = normalize_field_value(target_key, val)
        {
            result.fields.insert(
                target_key.to_owned(),
                build_field_proposal(
                    target_key,
                    normalized,
                    default_conf,
                    empty_checks,
                    text_lines,
                ),
            );
        }
    };

    insert_scalar("invoiceNumber", inv.get("invoice_number"));
    insert_scalar(
        "invoiceType",
        inv.get("type_code").or_else(|| inv.get("invoice_type")),
    );
    insert_scalar("issueDate", inv.get("issue_date"));
    insert_scalar("dueDate", inv.get("due_date"));
    insert_scalar(
        "serviceDate",
        inv.get("delivery_date")
            .or_else(|| inv.get("service_date"))
            .or_else(|| inv.get("tax_point_date")),
    );
    insert_scalar("currency", inv.get("currency"));
    insert_scalar("buyerReference", inv.get("buyer_reference"));
    insert_scalar("precedingInvoiceNumber", inv.get("preceding_invoice"));
    insert_scalar("precedingInvoiceDate", inv.get("preceding_invoice_date"));
    insert_scalar("prepaidAmount", inv.get("prepaid_amount"));

    if let Some(seller) = inv.get("seller").and_then(Value::as_object) {
        insert_scalar("sellerName", seller.get("name"));
        insert_scalar(
            "sellerAddressLine1",
            seller.get("street").or_else(|| seller.get("address_line1")),
        );
        insert_scalar(
            "sellerAddressLine2",
            seller
                .get("street2")
                .or_else(|| seller.get("address_line2")),
        );
        insert_scalar("sellerPostalCode", seller.get("postal_code"));
        insert_scalar("sellerCity", seller.get("city"));
        insert_scalar(
            "sellerCountryCode",
            seller.get("country").or_else(|| seller.get("country_code")),
        );
        insert_scalar("sellerVatId", seller.get("vat_id"));
        insert_scalar("sellerTaxNumber", seller.get("tax_number"));
        insert_scalar("sellerContact", seller.get("contact_name"));
        insert_scalar("sellerPhone", seller.get("phone"));
        insert_scalar("sellerEmail", seller.get("email"));
        insert_scalar("sellerEndpoint", seller.get("endpoint_id"));
    }

    if let Some(buyer) = inv.get("buyer").and_then(Value::as_object) {
        insert_scalar("buyerName", buyer.get("name"));
        insert_scalar(
            "buyerAddressLine1",
            buyer.get("street").or_else(|| buyer.get("address_line1")),
        );
        insert_scalar(
            "buyerAddressLine2",
            buyer.get("street2").or_else(|| buyer.get("address_line2")),
        );
        insert_scalar("buyerPostalCode", buyer.get("postal_code"));
        insert_scalar("buyerCity", buyer.get("city"));
        insert_scalar(
            "buyerCountryCode",
            buyer.get("country").or_else(|| buyer.get("country_code")),
        );
        insert_scalar("buyerVatId", buyer.get("vat_id"));
        insert_scalar("buyerEmail", buyer.get("email"));
        insert_scalar("buyerEndpoint", buyer.get("endpoint_id"));
    }

    if let Some(payment) = inv.get("payment").and_then(Value::as_object) {
        insert_scalar("iban", payment.get("iban"));
        insert_scalar("bic", payment.get("bic"));
        insert_scalar("paymentTerms", payment.get("terms"));
        insert_scalar("paymentMeans", payment.get("means_code"));
        insert_scalar("paymentAccountName", payment.get("account_name"));
        insert_scalar(
            "paymentReference",
            payment
                .get("payment_reference")
                .or_else(|| payment.get("remittance_information")),
        );
    }

    if let Some(totals) = inv.get("totals").and_then(Value::as_object) {
        insert_scalar(
            "lineNet",
            totals
                .get("line_net")
                .or_else(|| totals.get("line_total"))
                .or_else(|| totals.get("net_total")),
        );
        insert_scalar(
            "taxTotal",
            totals.get("tax_total").or_else(|| totals.get("vat_total")),
        );
        insert_scalar(
            "taxInclusive",
            totals
                .get("tax_inclusive")
                .or_else(|| totals.get("total_with_vat"))
                .or_else(|| totals.get("gross_total")),
        );
        insert_scalar(
            "payable",
            totals.get("payable").or_else(|| totals.get("amount_due")),
        );
    }

    if let Some(lines) = inv.get("lines").and_then(Value::as_array) {
        for (idx, line_val) in lines.iter().enumerate() {
            let Some(obj) = line_val.as_object() else {
                continue;
            };
            let entry = partial_lines.entry(idx).or_default();
            for (k, v) in obj {
                if k == "allowances" || k == "charges" {
                    let is_charge = k == "charges";
                    if let Some(items) = v.as_array() {
                        for item in items {
                            if let Some(ac) =
                                parse_allowance_charge(item, is_charge, exemption_reasons)
                            {
                                entry.allowances.push(ac);
                            }
                        }
                    }
                } else {
                    entry.apply_field(k, v, default_conf, empty_checks);
                }
            }
        }
    }

    for (key, is_charge) in [("allowances", false), ("charges", true)] {
        if let Some(items) = inv.get(key).and_then(Value::as_array) {
            for item in items {
                if let Some(ac) = parse_allowance_charge(item, is_charge, exemption_reasons) {
                    result.allowances.push(ac);
                }
            }
        }
    }
}

fn parse_allowance_charge(
    value: &Value,
    charge: bool,
    exemption_reasons: &HashMap<String, String>,
) -> Option<AllowanceCharge> {
    let obj = value.as_object()?;
    let amount = obj.get("amount").and_then(value_to_money)?;
    let reason = json_string(obj.get("reason"));
    let reason_code = json_string(obj.get("reason_code"));
    let percent = obj.get("percent").and_then(value_to_decimal);
    let base_amount = obj.get("base_amount").and_then(value_to_money);
    let rate = obj
        .get("vat_rate")
        .or_else(|| obj.get("tax_rate"))
        .and_then(value_to_decimal)
        .unwrap_or_else(|| "0".into());
    let category_code = json_string(obj.get("vat_category").or_else(|| obj.get("tax_category")))
        .map(|s| s.to_uppercase())
        .filter(|s| ["S", "Z", "E", "AE", "K"].contains(&s.as_str()))
        .unwrap_or_else(|| {
            if Decimal::from_str(&rate).is_ok_and(|d| d > Decimal::ZERO) {
                "S".into()
            } else {
                "Z".into()
            }
        });
    let exemption_reason = json_string(
        obj.get("vat_exemption_reason")
            .or_else(|| obj.get("exemption_reason")),
    )
    .or_else(|| exemption_reasons.get(&category_code).cloned());
    Some(AllowanceCharge {
        charge,
        amount,
        reason,
        reason_code,
        percent,
        base_amount,
        tax: Tax {
            category_code,
            rate,
            exemption_reason,
            exemption_reason_code: json_string(obj.get("vat_exemption_reason_code")),
        },
    })
}

fn ingest_ubl_xml_fallback(xml: &str, text_lines: &[TextLine], result: &mut RecognitionResult) {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return;
    };
    let root = doc.root_element();
    let checks = vec!["ubl_xml_fallback".to_owned()];

    let mut insert_if_missing = |key: &str, raw: Option<String>| {
        if result.fields.contains_key(key) {
            return;
        }
        let Some(raw_val) = raw else { return };
        if let Some(normalized) = normalize_field_value(key, &Value::String(raw_val)) {
            result.fields.insert(
                key.to_owned(),
                build_field_proposal(key, normalized, 0.85, &checks, text_lines),
            );
        }
    };

    insert_if_missing("invoiceNumber", direct_child_text(root, "ID"));
    insert_if_missing("issueDate", direct_child_text(root, "IssueDate"));
    insert_if_missing("dueDate", direct_child_text(root, "DueDate"));
    insert_if_missing(
        "invoiceType",
        direct_child_text(root, "InvoiceTypeCode")
            .or_else(|| direct_child_text(root, "CreditNoteTypeCode")),
    );
    insert_if_missing("currency", direct_child_text(root, "DocumentCurrencyCode"));
    insert_if_missing("buyerReference", direct_child_text(root, "BuyerReference"));
    if let Some(delivery) = direct_child(root, "Delivery") {
        insert_if_missing(
            "serviceDate",
            direct_child_text(delivery, "ActualDeliveryDate"),
        );
    }
    if let Some(billing_ref) = direct_child(root, "BillingReference")
        .and_then(|n| direct_child(n, "InvoiceDocumentReference"))
    {
        insert_if_missing(
            "precedingInvoiceNumber",
            direct_child_text(billing_ref, "ID"),
        );
        insert_if_missing(
            "precedingInvoiceDate",
            direct_child_text(billing_ref, "IssueDate"),
        );
    }

    for (tag, prefix) in [
        ("AccountingSupplierParty", "seller"),
        ("AccountingCustomerParty", "buyer"),
    ] {
        if let Some(party) = direct_child(root, tag).and_then(|n| direct_child(n, "Party")) {
            let name = direct_child(party, "PartyLegalEntity")
                .and_then(|n| direct_child_text(n, "RegistrationName"))
                .or_else(|| {
                    direct_child(party, "PartyName").and_then(|n| direct_child_text(n, "Name"))
                });
            insert_if_missing(&format!("{prefix}Name"), name);
            insert_if_missing(
                &format!("{prefix}Endpoint"),
                direct_child_text(party, "EndpointID"),
            );
            if let Some(addr) = direct_child(party, "PostalAddress") {
                insert_if_missing(
                    &format!("{prefix}AddressLine1"),
                    direct_child_text(addr, "StreetName"),
                );
                insert_if_missing(
                    &format!("{prefix}AddressLine2"),
                    direct_child_text(addr, "AdditionalStreetName"),
                );
                insert_if_missing(
                    &format!("{prefix}PostalCode"),
                    direct_child_text(addr, "PostalZone"),
                );
                insert_if_missing(
                    &format!("{prefix}City"),
                    direct_child_text(addr, "CityName"),
                );
                insert_if_missing(
                    &format!("{prefix}CountryCode"),
                    direct_child(addr, "Country")
                        .and_then(|c| direct_child_text(c, "IdentificationCode")),
                );
            }
            for tax_scheme in party
                .children()
                .filter(|n| n.is_element() && n.tag_name().name() == "PartyTaxScheme")
            {
                let company_id = direct_child_text(tax_scheme, "CompanyID");
                let scheme_id =
                    direct_child(tax_scheme, "TaxScheme").and_then(|s| direct_child_text(s, "ID"));
                if scheme_id.as_deref() == Some("VAT") {
                    insert_if_missing(&format!("{prefix}VatId"), company_id);
                } else if prefix == "seller" {
                    insert_if_missing("sellerTaxNumber", company_id);
                }
            }
            if prefix == "seller"
                && let Some(contact) = direct_child(party, "Contact")
            {
                insert_if_missing("sellerContact", direct_child_text(contact, "Name"));
                insert_if_missing("sellerPhone", direct_child_text(contact, "Telephone"));
                insert_if_missing("sellerEmail", direct_child_text(contact, "ElectronicMail"));
            }
        }
    }

    if let Some(pm) = direct_child(root, "PaymentMeans") {
        insert_if_missing("paymentMeans", direct_child_text(pm, "PaymentMeansCode"));
        insert_if_missing("paymentReference", direct_child_text(pm, "PaymentID"));
        if let Some(acc) = direct_child(pm, "PayeeFinancialAccount") {
            insert_if_missing("iban", direct_child_text(acc, "ID"));
            insert_if_missing("paymentAccountName", direct_child_text(acc, "Name"));
            insert_if_missing(
                "bic",
                direct_child(acc, "FinancialInstitutionBranch")
                    .and_then(|b| direct_child_text(b, "ID")),
            );
        }
    }
    if let Some(terms) = direct_child(root, "PaymentTerms") {
        insert_if_missing("paymentTerms", direct_child_text(terms, "Note"));
    }
    if let Some(lmt) = direct_child(root, "LegalMonetaryTotal") {
        insert_if_missing("lineNet", direct_child_text(lmt, "LineExtensionAmount"));
        insert_if_missing("taxInclusive", direct_child_text(lmt, "TaxInclusiveAmount"));
        insert_if_missing("prepaidAmount", direct_child_text(lmt, "PrepaidAmount"));
        insert_if_missing("payable", direct_child_text(lmt, "PayableAmount"));
    }
    if let Some(tax_total) = direct_child(root, "TaxTotal") {
        insert_if_missing("taxTotal", direct_child_text(tax_total, "TaxAmount"));
    }

    if result.lines.is_empty() {
        let mut idx = 1usize;
        for node in root.children().filter(|n| {
            n.is_element() && matches!(n.tag_name().name(), "InvoiceLine" | "CreditNoteLine")
        }) {
            let mut partial = PartialLine::default();
            partial.note_confidence(0.85, &checks);
            let qty_node = direct_child(node, "InvoicedQuantity")
                .or_else(|| direct_child(node, "CreditedQuantity"));
            if let Some(q) = qty_node {
                if let Some(text) = q.text().map(str::trim).filter(|s| !s.is_empty()) {
                    partial.quantity = crate::recognition::schema::decimal(text);
                }
                if let Some(unit) = q.attribute("unitCode").filter(|s| !s.trim().is_empty()) {
                    partial.unit_code = Some(unit.trim().to_uppercase());
                }
            }
            if let Some(ext) = direct_child_text(node, "LineExtensionAmount") {
                partial.net_amount = value_to_money(&Value::String(ext));
            }
            if let Some(item) = direct_child(node, "Item") {
                partial.name = direct_child_text(item, "Name");
                partial.description = direct_child_text(item, "Description");
                if let Some(tax_cat) = direct_child(item, "ClassifiedTaxCategory") {
                    partial.tax_category =
                        direct_child_text(tax_cat, "ID").map(|s| s.to_uppercase());
                    partial.tax_rate = direct_child_text(tax_cat, "Percent")
                        .and_then(|s| crate::recognition::schema::decimal(&s));
                }
            }
            if let Some(price) = direct_child(node, "Price")
                && let Some(amt) = direct_child_text(price, "PriceAmount")
            {
                partial.net_unit_price = crate::recognition::schema::decimal(&amt);
            }
            if let Some((line_proposal, _)) =
                partial.into_line_proposal(idx, &HashMap::new(), &HashMap::new(), text_lines)
            {
                result.lines.push(line_proposal);
                idx += 1;
            }
        }
    }
}

fn direct_child<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    local_name: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == local_name)
}

fn direct_child_text(node: roxmltree::Node<'_, '_>, local_name: &str) -> Option<String> {
    direct_child(node, local_name)
        .and_then(|child| child.text())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn detect_one_based_lines(fields: &[ExtractedField], structured_len: usize) -> bool {
    if structured_len == 0 {
        return false;
    }
    let mut min_idx: Option<usize> = None;
    let mut max_idx: Option<usize> = None;
    for field in fields {
        let path = normalize_field_path(&field.path);
        if let Some((idx, _)) = parse_line_path(&path) {
            min_idx = Some(min_idx.map_or(idx, |m| m.min(idx)));
            max_idx = Some(max_idx.map_or(idx, |m| m.max(idx)));
        }
    }
    matches!((min_idx, max_idx), (Some(min), Some(max)) if min >= 1 && max == structured_len)
}

fn normalize_field_path(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches("invoice.");
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        match ch {
            '[' => out.push('.'),
            ']' => {}
            _ => out.push(ch),
        }
    }
    out.replace("..", ".")
}

fn parse_line_path(path: &str) -> Option<(usize, &str)> {
    let rest = path.strip_prefix("lines.")?;
    let (idx_str, attr) = rest.split_once('.')?;
    let idx = idx_str.parse::<usize>().ok()?;
    Some((idx, attr))
}

fn map_scalar_path(path: &str) -> Option<&'static str> {
    match path {
        "invoice_number" => Some("invoiceNumber"),
        "type_code" | "invoice_type" => Some("invoiceType"),
        "issue_date" => Some("issueDate"),
        "due_date" => Some("dueDate"),
        "delivery_date" | "service_date" | "tax_point_date" => Some("serviceDate"),
        "currency" => Some("currency"),
        "buyer_reference" => Some("buyerReference"),
        "preceding_invoice" => Some("precedingInvoiceNumber"),
        "preceding_invoice_date" => Some("precedingInvoiceDate"),
        "prepaid_amount" => Some("prepaidAmount"),
        "seller.name" => Some("sellerName"),
        "seller.street" | "seller.address_line1" | "seller.address.line1" => {
            Some("sellerAddressLine1")
        }
        "seller.street2" | "seller.address_line2" | "seller.address.line2" => {
            Some("sellerAddressLine2")
        }
        "seller.postal_code" | "seller.address.postal_code" => Some("sellerPostalCode"),
        "seller.city" | "seller.address.city" => Some("sellerCity"),
        "seller.country" | "seller.country_code" | "seller.address.country_code" => {
            Some("sellerCountryCode")
        }
        "seller.vat_id" => Some("sellerVatId"),
        "seller.tax_number" | "seller.tax_registration_id" => Some("sellerTaxNumber"),
        "seller.contact_name" | "seller.contact.name" => Some("sellerContact"),
        "seller.phone" | "seller.contact.phone" => Some("sellerPhone"),
        "seller.email" | "seller.contact.email" => Some("sellerEmail"),
        "seller.endpoint_id" => Some("sellerEndpoint"),
        "buyer.name" => Some("buyerName"),
        "buyer.street" | "buyer.address_line1" | "buyer.address.line1" => Some("buyerAddressLine1"),
        "buyer.street2" | "buyer.address_line2" | "buyer.address.line2" => {
            Some("buyerAddressLine2")
        }
        "buyer.postal_code" | "buyer.address.postal_code" => Some("buyerPostalCode"),
        "buyer.city" | "buyer.address.city" => Some("buyerCity"),
        "buyer.country" | "buyer.country_code" | "buyer.address.country_code" => {
            Some("buyerCountryCode")
        }
        "buyer.vat_id" => Some("buyerVatId"),
        "buyer.email" | "buyer.contact.email" => Some("buyerEmail"),
        "buyer.endpoint_id" => Some("buyerEndpoint"),
        "payment.iban" | "iban" => Some("iban"),
        "payment.bic" | "bic" => Some("bic"),
        "payment.terms" | "payment_terms" => Some("paymentTerms"),
        "payment.means_code" => Some("paymentMeans"),
        "payment.account_name" => Some("paymentAccountName"),
        "payment.payment_reference" | "payment.remittance_information" => Some("paymentReference"),
        "totals.line_net" | "totals.net_total" | "line_net" | "net_total" => Some("lineNet"),
        "totals.tax_total" | "totals.vat_total" | "tax_total" | "vat_total" => Some("taxTotal"),
        "totals.tax_inclusive" | "totals.gross_total" | "tax_inclusive" | "gross_total" => {
            Some("taxInclusive")
        }
        "totals.payable" | "totals.amount_due" | "payable" | "amount_due" => Some("payable"),
        _ => None,
    }
}

fn normalize_field_value(target_key: &str, value: &Value) -> Option<String> {
    match target_key {
        "issueDate" | "dueDate" | "serviceDate" | "precedingInvoiceDate" => {
            let text = value_to_display(value)?;
            crate::recognition::schema::date(&text)
        }
        "currency" => {
            let text = value_to_display(value)?.to_uppercase();
            (text.len() == 3 && text.bytes().all(|b| b.is_ascii_uppercase())).then_some(text)
        }
        "sellerCountryCode" | "buyerCountryCode" => {
            let text = value_to_display(value)?.to_uppercase();
            (text.len() == 2 && text.bytes().all(|b| b.is_ascii_uppercase())).then_some(text)
        }
        "invoiceType" => {
            let text = value_to_display(value)?;
            ["380", "381", "384", "326"]
                .contains(&text.as_str())
                .then_some(text)
        }
        "iban" | "sellerVatId" | "buyerVatId" => {
            let compact: String = value_to_display(value)?
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .to_uppercase();
            (!compact.is_empty()).then_some(compact)
        }
        "lineNet" | "taxTotal" | "taxInclusive" | "payable" | "prepaidAmount" => {
            value_to_money(value)
        }
        _ => value_to_display(value),
    }
}

fn build_field_proposal(
    target_key: &str,
    value: String,
    confidence: f32,
    checks: &[String],
    text_lines: &[TextLine],
) -> FieldProposal {
    let matched = find_matching_line(text_lines, &value);
    let (source_token_ids, source_text) = match matched {
        Some(line) => (
            line.tokens.iter().map(|t| t.id.clone()).collect(),
            annotate_source(&line.text, checks),
        ),
        None => (
            Vec::new(),
            annotate_source(
                &format!("W System ({})", super::field_label(target_key)),
                checks,
            ),
        ),
    };
    FieldProposal {
        value,
        confidence: sanitize_confidence(confidence),
        source_token_ids,
        source_text,
    }
}

fn find_matching_line<'a>(lines: &'a [TextLine], needle: &str) -> Option<&'a TextLine> {
    let trimmed = needle.trim();
    if trimmed.len() < 2 {
        return None;
    }
    let lowered = trimmed.to_lowercase();
    if let Some(found) = lines
        .iter()
        .find(|line| line.text.to_lowercase().contains(&lowered))
    {
        return Some(found);
    }
    // Try German decimal comma variant for numeric values (e.g. "1556.00" -> "1556,00")
    if trimmed.contains('.') {
        let comma = trimmed.replace('.', ",");
        if let Some(found) = lines.iter().find(|line| line.text.contains(&comma)) {
            return Some(found);
        }
    }
    None
}

fn annotate_source(base: &str, checks: &[String]) -> String {
    if checks.is_empty() {
        return base.to_owned();
    }
    let labels = checks
        .iter()
        .map(|check| match check.as_str() {
            "found_in_pdf_text" => "im PDF-Text bestätigt",
            "totals_match_printed" => "mit gedruckten Summen abgeglichen",
            "totals_mismatch" => "Summenabweichung",
            "vat_id_format_ok" => "USt-IdNr.-Format gültig",
            "iban_checksum_ok" => "IBAN-Prüfsumme gültig",
            "country_guessed" => "Land geschätzt",
            "ubl_xml_fallback" => "aus UBL-XML",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{base} · W System: {labels}")
}

fn ingest_totals_if_missing(
    totals: &serde_json::Map<String, Value>,
    text_lines: &[TextLine],
    result: &mut RecognitionResult,
) {
    let empty_checks: &[String] = &[];
    for (target_key, raw) in [
        (
            "lineNet",
            totals
                .get("line_net")
                .or_else(|| totals.get("line_total"))
                .or_else(|| totals.get("net_total")),
        ),
        (
            "taxTotal",
            totals.get("tax_total").or_else(|| totals.get("vat_total")),
        ),
        (
            "taxInclusive",
            totals
                .get("tax_inclusive")
                .or_else(|| totals.get("total_with_vat"))
                .or_else(|| totals.get("gross_total")),
        ),
        (
            "payable",
            totals.get("payable").or_else(|| totals.get("amount_due")),
        ),
    ] {
        if result.fields.contains_key(target_key) {
            continue;
        }
        if let Some(val) = raw
            && let Some(normalized) = normalize_field_value(target_key, val)
        {
            result.fields.insert(
                target_key.to_owned(),
                build_field_proposal(target_key, normalized, 0.85, empty_checks, text_lines),
            );
        }
    }
}

fn arithmetic_row_total_key(check_name: &str) -> Option<&'static str> {
    let lower = check_name.trim().to_lowercase();
    if lower.starts_with("lines.") || lower.starts_with("line ") {
        return None;
    }
    if lower.contains("bt-106") || lower.contains("bt-109") || lower.contains("sum of line net") {
        Some("lineNet")
    } else if lower.contains("bt-110") || lower.contains("vat total") || lower.contains("tax total")
    {
        Some("taxTotal")
    } else if lower.contains("bt-112")
        || lower.contains("total with vat")
        || lower.contains("gross")
        || lower.contains("tax inclusive")
    {
        Some("taxInclusive")
    } else if lower.contains("bt-115") || lower.contains("payable") || lower.contains("amount due")
    {
        Some("payable")
    } else {
        None
    }
}

fn format_uncertainty(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return (!s.trim().is_empty()).then(|| s.trim().to_owned());
    }
    let obj = value.as_object()?;
    let field = json_string(obj.get("field").or_else(|| obj.get("path")));
    let reason = json_string(
        obj.get("reason")
            .or_else(|| obj.get("message"))
            .or_else(|| obj.get("note")),
    );
    match (field, reason) {
        (Some(f), Some(r)) => Some(format!("{}: {r}", format_wsystem_path_label(&f))),
        (None, Some(r)) => Some(r),
        (Some(f), None) => Some(format_wsystem_path_label(&f)),
        (None, None) => None,
    }
}

fn format_wsystem_path_label(path: &str) -> String {
    let normalized = normalize_field_path(path);
    if let Some(key) = map_scalar_path(&normalized) {
        return super::field_label(key).to_owned();
    }
    if let Some((idx, attr)) = parse_line_path(&normalized) {
        let attr_label = match attr {
            "unit_price" | "unit_price_net" | "net_unit_price" => "Nettopreis",
            "quantity" => "Menge",
            "description" | "name" => "Bezeichnung",
            "vat_rate" | "tax_rate" => "USt-Satz",
            "unit" | "unit_code" => "Einheit",
            other => other,
        };
        return format!("Position {idx} ({attr_label})");
    }
    path.to_owned()
}

fn format_validation_error(err: &Value) -> Option<String> {
    if let Some(s) = err.as_str() {
        return (!s.trim().is_empty()).then(|| s.trim().to_owned());
    }
    let obj = err.as_object()?;
    let rule = json_string(obj.get("rule_id").or_else(|| obj.get("id")));
    let msg = json_string(obj.get("message").or_else(|| obj.get("detail")));
    match (rule, msg) {
        (Some(r), Some(m)) => Some(format!("{r}: {m}")),
        (None, Some(m)) => Some(m),
        (Some(r), None) => Some(r),
        (None, None) => None,
    }
}

fn compute_line_net(
    quantity: &str,
    unit_price: &str,
    allowances: &[AllowanceCharge],
) -> Option<String> {
    let mut amount = Decimal::from_str(quantity)
        .ok()?
        .checked_mul(Decimal::from_str(unit_price).ok()?)?;
    for item in allowances {
        let adj = Decimal::from_str(&item.amount).ok()?;
        amount = if item.charge {
            amount.checked_add(adj)?
        } else {
            amount.checked_sub(adj)?
        };
    }
    let mut rounded = amount.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
    rounded.rescale(2);
    Some(rounded.to_string())
}

fn sanitize_confidence(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

fn json_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn json_u64(value: Option<&Value>) -> Option<u64> {
    let val = value?;
    if let Some(n) = val.as_u64() {
        return Some(n);
    }
    if let Some(n) = val.as_i64()
        && n >= 0
    {
        return Some(n as u64);
    }
    val.as_str().and_then(|s| s.trim().parse::<u64>().ok())
}

fn value_to_display(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => {
            let t = s.trim();
            (!t.is_empty()).then(|| t.to_owned())
        }
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn value_to_decimal(value: &Value) -> Option<String> {
    match value {
        Value::Number(n) => Decimal::from_str(&n.to_string())
            .ok()
            .map(|d| d.normalize().to_string()),
        Value::String(s) => crate::recognition::schema::decimal(s),
        _ => None,
    }
}

fn value_to_money(value: &Value) -> Option<String> {
    let dec = value_to_decimal(value)?;
    let mut rounded = Decimal::from_str(&dec)
        .ok()?
        .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
    rounded.rescale(2);
    Some(rounded.to_string())
}

fn is_allowed_base_url(url: &str) -> bool {
    if url.starts_with("https://") && url.len() > "https://".len() {
        return true;
    }
    if let Some(rest) = url.strip_prefix("http://") {
        let host = rest.split('/').next().unwrap_or("");
        return host == "localhost"
            || host.starts_with("localhost:")
            || host == "127.0.0.1"
            || host.starts_with("127.0.0.1:")
            || host == "[::1]"
            || host.starts_with("[::1]:");
    }
    false
}

fn header_str<T>(response: &ureq::http::Response<T>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn header_u64<T>(response: &ureq::http::Response<T>, name: &str) -> Option<u64> {
    header_str(response, name).and_then(|s| s.parse::<u64>().ok())
}

fn read_bounded_body(
    response: ureq::http::Response<ureq::Body>,
    limit: u64,
    api_key: &ApiKey,
) -> Result<Vec<u8>, String> {
    response
        .into_body()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|error| {
            sanitize_secret(
                &format!("W-System-Antwort konnte nicht gelesen werden: {error}"),
                api_key,
            )
        })
}

fn map_transport_error(error: ureq::Error, api_key: &ApiKey, request_id: &str) -> String {
    let base = match error {
        ureq::Error::Timeout(_) => {
            "Zeitüberschreitung bei der Verbindung zu W System. Bitte erneut versuchen."
        }
        ureq::Error::RedirectFailed | ureq::Error::TooManyRedirects => {
            "W System hat eine unerwartete Weiterleitung gesendet. Anfrage zum Schutz des API-Schlüssels abgebrochen."
        }
        _ => "Verbindung zu W System fehlgeschlagen. Bitte Internetverbindung und Endpunkt prüfen.",
    };
    sanitize_secret(&format_with_request_id(base, Some(request_id)), api_key)
}

fn format_api_error(
    status: u16,
    body: &[u8],
    fallback_request_id: Option<&str>,
    api_key: &ApiKey,
) -> String {
    let parsed = serde_json::from_slice::<Value>(body).ok();
    let body_request_id = parsed
        .as_ref()
        .and_then(|v| json_string(v.get("request_id")));
    let request_id = body_request_id.as_deref().or(fallback_request_id);
    let code = parsed.as_ref().and_then(extract_error_code);
    let message = parsed.as_ref().and_then(extract_error_message);

    let explanation = match (status, code.as_deref()) {
        (401 | 403, _) => {
            "W System hat den API-Schlüssel abgelehnt (HTTP 401/403). Bitte Schlüssel unter Einstellungen → Erkennung prüfen oder neu anfordern."
                .to_owned()
        }
        (402, Some("paid_plan_required")) => {
            "Das kostenlose W-System-Testkontingent (3 Extraktionen/Monat) ist aufgebraucht (HTTP 402). Pay-as-you-go (0,15 €/Rechnung, kein Abo) lässt sich unter Einstellungen → Erkennung aktivieren; es wurden keine Kosten berechnet."
                .to_owned()
        }
        (402, _) => {
            "W System meldet ein fehlendes Guthaben bzw. einen erforderlichen Tarif (HTTP 402). Es wurden keine Kosten berechnet."
                .to_owned()
        }
        (409, Some("idempotency_key_in_flight")) => {
            "Eine identische Anfrage an W System läuft noch (HTTP 409). Bitte kurz warten und erneut versuchen."
                .to_owned()
        }
        (422, Some("no_text_layer")) => {
            "Diese PDF enthält keine auslesbare Textebene (Scan). W System unterstützt derzeit keine OCR; bitte Schemaerkennung mit lokaler OCR verwenden."
                .to_owned()
        }
        (422, Some("already_einvoice")) => {
            "Diese PDF enthält bereits eine eingebettete E-Rechnung (ZUGFeRD/Factur-X). W System lehnt sie für /einvoice/extract ab."
                .to_owned()
        }
        (429, Some("monthly_quota_exhausted")) => {
            "Das monatliche W-System-Kontingent ist erschöpft (HTTP 429). Es wurden keine Einheiten abgezogen."
                .to_owned()
        }
        (429, _) => {
            "W System meldet ein erreichtes Kontingent oder Rate-Limit (HTTP 429). Es wurden keine Einheiten abgezogen."
                .to_owned()
        }
        (501, Some("extract_not_enabled")) => {
            "Der W-System-Endpunkt /einvoice/extract ist serverseitig derzeit deaktiviert (HTTP 501)."
                .to_owned()
        }
        (status, _) if status >= 500 => {
            format!(
                "W System ist vorübergehend nicht erreichbar (HTTP {status}). Fehlgeschlagene Aufrufe werden laut Anbieter nicht berechnet."
            )
        }
        (status, Some(code)) => match message {
            Some(msg) => format!("W System Fehler (HTTP {status}, {code}): {msg}"),
            None => format!("W System Fehler (HTTP {status}, {code})."),
        },
        (status, None) => match message {
            Some(msg) => format!("W System Fehler (HTTP {status}): {msg}"),
            None => format!("W System Fehler (HTTP {status})."),
        },
    };

    sanitize_secret(&format_with_request_id(&explanation, request_id), api_key)
}

fn extract_error_code(value: &Value) -> Option<String> {
    if let Some(code) = value
        .get("detail")
        .and_then(Value::as_object)
        .and_then(|obj| json_string(obj.get("code").or_else(|| obj.get("error"))))
    {
        return Some(code);
    }
    json_string(
        value
            .get("error")
            .or_else(|| value.get("code"))
            .or_else(|| value.get("error_code")),
    )
}

fn extract_error_message(value: &Value) -> Option<String> {
    json_string(value.get("message"))
        .or_else(|| json_string(value.get("detail")))
        .or_else(|| {
            value
                .get("detail")
                .and_then(Value::as_object)
                .and_then(|obj| json_string(obj.get("message")))
        })
}

fn format_with_request_id(message: &str, request_id: Option<&str>) -> String {
    match request_id.filter(|id| !id.trim().is_empty()) {
        Some(id) => format!("{message} [Request-ID: {}]", id.trim()),
        None => message.to_owned(),
    }
}

fn sanitize_secret(text: &str, api_key: &ApiKey) -> String {
    let secret = api_key.expose();
    if secret.is_empty() {
        text.to_owned()
    } else {
        text.replace(secret, "[REDACTED]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_policy_enforces_https_except_loopback() {
        assert!(WSystemClient::with_base_url("https://api.wsystem.dev").is_ok());
        assert!(WSystemClient::with_base_url("http://127.0.0.1:8080").is_ok());
        assert!(WSystemClient::with_base_url("http://localhost:9000").is_ok());
        assert!(WSystemClient::with_base_url("http://api.wsystem.dev").is_err());
        assert!(WSystemClient::with_base_url("http://127.0.0.1.evil.example").is_err());
        assert!(WSystemClient::with_base_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn sanitize_never_leaks_api_key() {
        let key = ApiKey::parse("wsk_live_secret_value").unwrap();
        let cleaned = sanitize_secret("header X-API-Key: wsk_live_secret_value failed", &key);
        assert!(!cleaned.contains("wsk_live_secret_value"));
        assert!(cleaned.contains("[REDACTED]"));
    }

    #[test]
    fn usage_summary_formats_quota_and_estimated_invoices() {
        let usage = UsageInfo {
            plan: Some("free".into()),
            quota: Some(500),
            used: Some(231),
            remaining: Some(269),
            period: Some("2026-11".into()),
            extract_unit_weight: Some(80),
            request_id: Some("req_123".into()),
        };
        let summary = usage.summary();
        assert!(summary.contains("Tarif: free"));
        assert!(summary.contains("Zeitraum: 2026-11"));
        assert!(summary.contains("269 von 500 Einheiten"));
        assert!(summary.contains("ca. 3 Rechnungen"));
        assert!(summary.contains("req_123"));
    }
}
