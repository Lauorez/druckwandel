//! Local recognition boundary. No model runtime, weights, LoRA, API client or AI
//! transport is included. A future local adapter implements RecognitionProvider.
mod schema;
use crate::extraction::{DocumentPage, reconstruct_lines};
use crate::invoice::{Contact, ElectronicAddress, InvoiceInput, InvoiceLine, Tax};
pub use schema::SchemaRecognizer;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const CONTRACT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecognitionMethod {
    Schema,
    LocalAi,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionRequest {
    pub schema_version: u32,
    pub document_sha256: String,
    pub source_pdf: PathBuf,
    pub pages: Vec<DocumentPage>,
}

impl RecognitionRequest {
    pub fn from_pdf(source_pdf: PathBuf, expected_sha256: &str) -> Result<Self, String> {
        use sha2::{Digest, Sha256};
        if std::fs::metadata(&source_pdf)
            .map_err(|e| e.to_string())?
            .len()
            > 100 * 1024 * 1024
        {
            return Err("PDF überschreitet 100 MB.".into());
        }
        let bytes = std::fs::read(&source_pdf).map_err(|e| e.to_string())?;
        if bytes.len() > 100 * 1024 * 1024 {
            return Err("PDF überschreitet 100 MB.".into());
        }
        let document_sha256 = format!("{:x}", Sha256::digest(&bytes));
        if document_sha256 != expected_sha256 {
            return Err("Die Original-PDF wurde außerhalb der Anwendung verändert.".into());
        }
        let pages = crate::extraction::read_document_bytes(&bytes)?;
        Ok(Self {
            schema_version: CONTRACT_VERSION,
            document_sha256,
            source_pdf,
            pages,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldProposal {
    pub value: String,
    /// Rule confidence, not a calibrated probability.
    pub confidence: f32,
    pub source_token_ids: Vec<String>,
    pub source_text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineProposal {
    pub line: InvoiceLine,
    pub net_amount: String,
    pub confidence: f32,
    pub source_token_ids: Vec<String>,
    pub source_text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionResult {
    pub schema_version: u32,
    pub document_sha256: String,
    pub method: RecognitionMethod,
    pub provider_version: String,
    pub fields: BTreeMap<String, FieldProposal>,
    pub lines: Vec<LineProposal>,
    pub warnings: Vec<String>,
}

pub trait RecognitionProvider: Send + Sync {
    fn method(&self) -> RecognitionMethod;
    fn recognize(&self, request: &RecognitionRequest) -> Result<RecognitionResult, String>;
}

pub struct RecognitionRegistry {
    providers: Vec<Box<dyn RecognitionProvider>>,
}
impl Default for RecognitionRegistry {
    fn default() -> Self {
        Self {
            providers: vec![Box::new(SchemaRecognizer)],
        }
    }
}

/// One application composition point, initialized once. The future optional
/// local adapter is registered here; imports and re-recognition use this registry.
pub fn application_registry() -> &'static RecognitionRegistry {
    static REGISTRY: std::sync::OnceLock<RecognitionRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(RecognitionRegistry::default)
}
impl RecognitionRegistry {
    /// Application composition point for a future adapter from the separate AI repo.
    pub fn register(&mut self, provider: Box<dyn RecognitionProvider>) -> Result<(), String> {
        if self.available(provider.method()) {
            return Err("Erkennungsmethode bereits registriert.".into());
        }
        self.providers.push(provider);
        Ok(())
    }
    pub fn available(&self, method: RecognitionMethod) -> bool {
        self.providers
            .iter()
            .any(|provider| provider.method() == method)
    }
    pub fn recognize(
        &self,
        method: RecognitionMethod,
        request: &RecognitionRequest,
    ) -> Result<RecognitionResult, String> {
        if request.schema_version != CONTRACT_VERSION {
            return Err("Unbekannte Version des Erkennungsvertrags.".into());
        }
        let provider=self.providers.iter().find(|provider| provider.method()==method).ok_or("Lokale KI-Erkennung ist noch nicht installiert. Das separate KI-Projekt kann später als lokaler Adapter ergänzt werden.")?;
        let result = provider.recognize(request)?;
        if result.schema_version != CONTRACT_VERSION
            || result.method != method
            || result.document_sha256 != request.document_sha256
        {
            return Err("Erkennungsergebnis gehört nicht zu diesem Dokument oder Vertrag.".into());
        }
        let tokens: std::collections::HashSet<_> = request
            .pages
            .iter()
            .flat_map(|page| &page.tokens)
            .map(|token| token.id.as_str())
            .collect();
        let valid_proposal = |confidence: f32, ids: &[String]| {
            confidence.is_finite()
                && (0.0..=1.0).contains(&confidence)
                && ids.iter().all(|id| tokens.contains(id.as_str()))
        };
        if result
            .fields
            .values()
            .any(|field| !valid_proposal(field.confidence, &field.source_token_ids))
            || result
                .lines
                .iter()
                .any(|line| !valid_proposal(line.confidence, &line.source_token_ids))
        {
            return Err("Erkennung enthält ungültige Konfidenzen oder Fundstellen.".into());
        }
        Ok(result)
    }
}

fn field_label(name: &str) -> &str {
    match name {
        "invoiceNumber" => "Rechnungsnummer",
        "issueDate" => "Rechnungsdatum",
        "dueDate" => "Fälligkeit",
        "serviceDate" => "Leistungsdatum",
        "buyerReference" => "Bestellnummer / Leitweg-ID",
        "currency" => "Währung",
        "sellerName" => "Absender",
        "sellerAddressLine1" => "Absender: Straße",
        "sellerPostalCode" => "Absender: PLZ",
        "sellerCity" => "Absender: Ort",
        "sellerCountryCode" => "Absender: Land",
        "sellerVatId" => "Absender: USt-ID",
        "sellerContact" => "Ansprechpartner",
        "sellerPhone" => "Telefon",
        "sellerEmail" => "E-Mail",
        "buyerName" => "Empfänger",
        "buyerAddressLine1" => "Empfänger: Straße",
        "buyerPostalCode" => "Empfänger: PLZ",
        "buyerCity" => "Empfänger: Ort",
        "buyerCountryCode" => "Empfänger: Land",
        "buyerVatId" => "Empfänger: USt-ID",
        "iban" => "IBAN",
        "bic" => "BIC",
        "paymentTerms" => "Zahlungsbedingungen",
        "lineNet" => "Netto",
        "taxTotal" => "Umsatzsteuer",
        "taxInclusive" => "Brutto",
        "payable" => "Zahlbetrag",
        _ => name,
    }
}

impl RecognitionResult {
    pub fn review_text(&self) -> String {
        let mut text = String::from("Erkennungsvorschläge – bitte mit dem Beleg vergleichen.\n");
        for (name, field) in &self.fields {
            let name = field_label(name);
            text += &format!(
                "\n{name}: {} ({:.0} %)\nQuelle: {}\n",
                field.value,
                field.confidence * 100.0,
                field.source_text
            );
        }
        for (index, line) in self.lines.iter().enumerate() {
            text += &format!(
                "\nPosition {}: {} ({:.0} %)\nQuelle: {}\n",
                index + 1,
                line.line.name,
                line.confidence * 100.0,
                line.source_text
            );
        }
        for warning in &self.warnings {
            text += &format!("\nHinweis: {warning}\n");
        }
        text
    }
    /// Proposals become an editable draft, never an independently validated output.
    pub fn invoice_draft(&self) -> InvoiceInput {
        let mut invoice = InvoiceInput::blank();
        let value = |name: &str| {
            self.fields
                .get(name)
                .map(|field| field.value.clone())
                .unwrap_or_default()
        };
        let opt = |name: &str| self.fields.get(name).map(|field| field.value.clone());
        invoice.invoice_number = value("invoiceNumber");
        invoice.issue_date = value("issueDate");
        invoice.due_date = opt("dueDate");
        invoice.service_date = opt("serviceDate");
        invoice.buyer_reference = value("buyerReference");
        if let Some(currency) = opt("currency") {
            invoice.currency = currency;
        }
        for (prefix, party) in [
            ("seller", &mut invoice.seller),
            ("buyer", &mut invoice.buyer),
        ] {
            party.name = value(&format!("{prefix}Name"));
            party.address.line1 = value(&format!("{prefix}AddressLine1"));
            party.address.postal_code = value(&format!("{prefix}PostalCode"));
            party.address.city = value(&format!("{prefix}City"));
            party.address.country_code = value(&format!("{prefix}CountryCode"));
            party.vat_id = opt(&format!("{prefix}VatId"));
        }
        invoice.seller.contact = Some(Contact {
            name: opt("sellerContact"),
            phone: opt("sellerPhone"),
            email: opt("sellerEmail"),
        });
        if let Some(email) = opt("sellerEmail") {
            invoice.seller.electronic_address = Some(ElectronicAddress {
                value: email,
                scheme_id: "EM".into(),
            });
        }
        invoice.payment.iban = opt("iban");
        invoice.payment.bic = opt("bic");
        invoice.payment.terms = opt("paymentTerms");
        if !self.lines.is_empty() {
            invoice.lines = self.lines.iter().map(|item| item.line.clone()).collect();
        }
        invoice
    }
}

pub(super) fn proposal(
    value: String,
    line: &crate::extraction::TextLine,
    confidence: f32,
) -> FieldProposal {
    FieldProposal {
        value,
        confidence,
        source_token_ids: line.tokens.iter().map(|token| token.id.clone()).collect(),
        source_text: line.text.clone(),
    }
}

pub fn fill_missing(invoice: &mut InvoiceInput, result: &RecognitionResult) {
    let proposal = result.invoice_draft();
    fn text(target: &mut String, value: String) {
        if target.trim().is_empty() {
            *target = value;
        }
    }
    fn optional(target: &mut Option<String>, value: Option<String>) {
        if target
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            *target = value;
        }
    }
    text(&mut invoice.invoice_number, proposal.invoice_number);
    text(&mut invoice.issue_date, proposal.issue_date);
    optional(&mut invoice.due_date, proposal.due_date);
    optional(&mut invoice.service_date, proposal.service_date);
    text(&mut invoice.buyer_reference, proposal.buyer_reference);
    for (target, source) in [
        (&mut invoice.seller, proposal.seller),
        (&mut invoice.buyer, proposal.buyer),
    ] {
        text(&mut target.name, source.name);
        text(&mut target.address.line1, source.address.line1);
        text(&mut target.address.postal_code, source.address.postal_code);
        text(&mut target.address.city, source.address.city);
        text(
            &mut target.address.country_code,
            source.address.country_code,
        );
        optional(&mut target.vat_id, source.vat_id);
        if target.electronic_address.is_none() {
            target.electronic_address = source.electronic_address;
        }
        if let Some(source) = source.contact {
            let target = target.contact.get_or_insert_with(Contact::default);
            optional(&mut target.name, source.name);
            optional(&mut target.email, source.email);
            optional(&mut target.phone, source.phone);
        }
    }
    optional(&mut invoice.payment.iban, proposal.payment.iban);
    optional(&mut invoice.payment.bic, proposal.payment.bic);
    optional(&mut invoice.payment.terms, proposal.payment.terms);
    if invoice.lines.iter().all(|line| line.name.trim().is_empty()) && !result.lines.is_empty() {
        invoice.lines = proposal.lines;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ai_is_explicitly_unavailable_without_a_registered_local_adapter() {
        let registry = RecognitionRegistry::default();
        let request = RecognitionRequest {
            schema_version: 1,
            document_sha256: "test".into(),
            source_pdf: "local.pdf".into(),
            pages: vec![],
        };
        assert!(registry.available(RecognitionMethod::Schema));
        assert!(!registry.available(RecognitionMethod::LocalAi));
        assert!(
            registry
                .recognize(RecognitionMethod::LocalAi, &request)
                .unwrap_err()
                .contains("noch nicht installiert")
        );
    }
}
