use crate::invoice::{
    AllowanceCharge, CalculatedInvoice, InvoiceInput, Party, Tax, calculate, preceding, validate,
};

pub const CUSTOMIZATION_ID: &str =
    "urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0";
pub const PROFILE_ID: &str = "urn:fdc:peppol.eu:2017:poacc:billing:01:1.0";

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn element(name: &str, value: Option<&str>, attributes: &str) -> String {
    value.map_or_else(String::new, |value| {
        format!("<{name}{attributes}>{}</{name}>", escape(value))
    })
}

fn e(name: &str, value: &str) -> String {
    element(name, Some(value), "")
}
fn optional(name: &str, value: Option<&str>) -> String {
    element(name, value, "")
}
fn amount(name: &str, value: &str, currency: &str) -> String {
    element(
        name,
        Some(value),
        &format!(" currencyID=\"{}\"", escape(currency)),
    )
}

fn party_xml(wrapper: &str, party: &Party) -> String {
    let mut inner = String::new();
    if let Some(endpoint) = &party.electronic_address {
        inner.push_str(&element(
            "cbc:EndpointID",
            Some(&endpoint.value),
            &format!(" schemeID=\"{}\"", escape(&endpoint.scheme_id)),
        ));
    }
    inner.push_str("<cac:PartyName>");
    inner.push_str(&e("cbc:Name", &party.name));
    inner.push_str("</cac:PartyName><cac:PostalAddress>");
    inner.push_str(&e("cbc:StreetName", &party.address.line1));
    inner.push_str(&optional(
        "cbc:AdditionalStreetName",
        party.address.line2.as_deref(),
    ));
    inner.push_str(&e("cbc:CityName", &party.address.city));
    inner.push_str(&e("cbc:PostalZone", &party.address.postal_code));
    inner.push_str("<cac:Country>");
    inner.push_str(&e("cbc:IdentificationCode", &party.address.country_code));
    inner.push_str("</cac:Country></cac:PostalAddress>");
    if let Some(vat_id) = &party.vat_id {
        inner.push_str("<cac:PartyTaxScheme>");
        inner.push_str(&e("cbc:CompanyID", vat_id));
        inner.push_str("<cac:TaxScheme><cbc:ID>VAT</cbc:ID></cac:TaxScheme></cac:PartyTaxScheme>");
    }
    inner.push_str("<cac:PartyLegalEntity>");
    inner.push_str(&e("cbc:RegistrationName", &party.name));
    inner.push_str("</cac:PartyLegalEntity>");
    if let Some(contact) = &party.contact {
        inner.push_str("<cac:Contact>");
        inner.push_str(&optional("cbc:Name", contact.name.as_deref()));
        inner.push_str(&optional("cbc:Telephone", contact.phone.as_deref()));
        inner.push_str(&optional("cbc:ElectronicMail", contact.email.as_deref()));
        inner.push_str("</cac:Contact>");
    }
    format!("<cac:{wrapper}><cac:Party>{inner}</cac:Party></cac:{wrapper}>")
}

fn tax_category(tax: &Tax, exemption: bool) -> String {
    let mut xml = e("cbc:ID", &tax.category_code);
    xml.push_str(&e("cbc:Percent", &tax.rate));
    if exemption {
        xml.push_str(&optional(
            "cbc:TaxExemptionReasonCode",
            tax.exemption_reason_code.as_deref(),
        ));
        xml.push_str(&optional(
            "cbc:TaxExemptionReason",
            tax.exemption_reason.as_deref(),
        ));
    }
    xml.push_str("<cac:TaxScheme><cbc:ID>VAT</cbc:ID></cac:TaxScheme>");
    xml
}

fn allowance_xml(item: &AllowanceCharge, currency: &str, with_tax: bool) -> String {
    let mut xml = String::from("<cac:AllowanceCharge>");
    xml.push_str(&e(
        "cbc:ChargeIndicator",
        if item.charge { "true" } else { "false" },
    ));
    xml.push_str(&optional(
        "cbc:AllowanceChargeReasonCode",
        item.reason_code.as_deref(),
    ));
    xml.push_str(&optional(
        "cbc:AllowanceChargeReason",
        item.reason.as_deref(),
    ));
    xml.push_str(&optional(
        "cbc:MultiplierFactorNumeric",
        item.percent.as_deref(),
    ));
    xml.push_str(&amount("cbc:Amount", &item.amount, currency));
    if let Some(base) = &item.base_amount {
        xml.push_str(&amount("cbc:BaseAmount", base, currency));
    }
    if with_tax {
        xml.push_str("<cac:TaxCategory>");
        xml.push_str(&tax_category(&item.tax, true));
        xml.push_str("</cac:TaxCategory>");
    }
    xml.push_str("</cac:AllowanceCharge>");
    xml
}

pub fn generate_ubl(input: InvoiceInput) -> Result<String, String> {
    let errors = validate(&input);
    if !errors.is_empty() {
        return Err(errors.join(" "));
    }
    let invoice = calculate(input)?;
    if invoice.totals.payable.starts_with('-') {
        return Err("Der Zahlbetrag darf nicht negativ sein.".into());
    }
    Ok(render(&invoice))
}

fn render(invoice: &CalculatedInvoice) -> String {
    let data = &invoice.input;
    let currency = &data.currency;
    let credit_note = data.invoice_type == "381";
    let root = if credit_note { "CreditNote" } else { "Invoice" };
    let xmlns = if credit_note {
        "urn:oasis:names:specification:ubl:schema:xsd:CreditNote-2"
    } else {
        "urn:oasis:names:specification:ubl:schema:xsd:Invoice-2"
    };
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><{root} xmlns=\"{xmlns}\" xmlns:cac=\"urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2\" xmlns:cbc=\"urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2\">"
    );
    xml.push_str(&e("cbc:CustomizationID", CUSTOMIZATION_ID));
    xml.push_str(&e("cbc:ProfileID", PROFILE_ID));
    xml.push_str(&e("cbc:ID", &data.invoice_number));
    xml.push_str(&e("cbc:IssueDate", &data.issue_date));
    if !credit_note {
        xml.push_str(&optional("cbc:DueDate", data.due_date.as_deref()));
    }
    xml.push_str(&e(
        if credit_note {
            "cbc:CreditNoteTypeCode"
        } else {
            "cbc:InvoiceTypeCode"
        },
        &data.invoice_type,
    ));
    for note in &data.notes {
        xml.push_str(&e("cbc:Note", note));
    }
    xml.push_str(&e("cbc:DocumentCurrencyCode", currency));
    xml.push_str(&e("cbc:BuyerReference", &data.buyer_reference));
    for item in preceding(data) {
        xml.push_str("<cac:BillingReference><cac:InvoiceDocumentReference>");
        xml.push_str(&e("cbc:ID", &item.invoice_number));
        xml.push_str(&optional("cbc:IssueDate", item.issue_date.as_deref()));
        xml.push_str("</cac:InvoiceDocumentReference></cac:BillingReference>");
    }
    xml.push_str(&party_xml("AccountingSupplierParty", &data.seller));
    xml.push_str(&party_xml("AccountingCustomerParty", &data.buyer));
    if data.service_date.is_some() || data.delivery_address.is_some() {
        xml.push_str("<cac:Delivery>");
        xml.push_str(&optional(
            "cbc:ActualDeliveryDate",
            data.service_date.as_deref(),
        ));
        if let Some(address) = &data.delivery_address {
            xml.push_str("<cac:DeliveryLocation><cac:Address>");
            xml.push_str(&e("cbc:StreetName", &address.line1));
            xml.push_str(&e("cbc:CityName", &address.city));
            xml.push_str(&e("cbc:PostalZone", &address.postal_code));
            xml.push_str("<cac:Country>");
            xml.push_str(&e("cbc:IdentificationCode", &address.country_code));
            xml.push_str("</cac:Country></cac:Address></cac:DeliveryLocation>");
        }
        xml.push_str("</cac:Delivery>");
    }
    if let Some(iban) = &data.payment.iban {
        xml.push_str("<cac:PaymentMeans>");
        xml.push_str(&e("cbc:PaymentMeansCode", &data.payment.means_code));
        if credit_note {
            xml.push_str(&optional("cbc:PaymentDueDate", data.due_date.as_deref()));
        }
        xml.push_str(&optional(
            "cbc:PaymentID",
            data.payment.payment_reference.as_deref(),
        ));
        xml.push_str("<cac:PayeeFinancialAccount>");
        xml.push_str(&e("cbc:ID", iban));
        xml.push_str(&optional("cbc:Name", data.payment.account_name.as_deref()));
        if let Some(bic) = &data.payment.bic {
            xml.push_str("<cac:FinancialInstitutionBranch>");
            xml.push_str(&e("cbc:ID", bic));
            xml.push_str("</cac:FinancialInstitutionBranch>");
        }
        xml.push_str("</cac:PayeeFinancialAccount></cac:PaymentMeans>");
    }
    if let Some(terms) = &data.payment.terms {
        xml.push_str("<cac:PaymentTerms>");
        xml.push_str(&e("cbc:Note", terms));
        xml.push_str("</cac:PaymentTerms>");
    }
    for item in &data.allowances {
        xml.push_str(&allowance_xml(item, currency, true));
    }
    xml.push_str("<cac:TaxTotal>");
    xml.push_str(&amount(
        "cbc:TaxAmount",
        &invoice.totals.tax_total,
        currency,
    ));
    for tax in &invoice.taxes {
        xml.push_str("<cac:TaxSubtotal>");
        xml.push_str(&amount("cbc:TaxableAmount", &tax.taxable_amount, currency));
        xml.push_str(&amount("cbc:TaxAmount", &tax.tax_amount, currency));
        xml.push_str("<cac:TaxCategory>");
        xml.push_str(&tax_category(&tax.tax, true));
        xml.push_str("</cac:TaxCategory></cac:TaxSubtotal>");
    }
    xml.push_str("</cac:TaxTotal><cac:LegalMonetaryTotal>");
    xml.push_str(&amount(
        "cbc:LineExtensionAmount",
        &invoice.totals.line_net,
        currency,
    ));
    xml.push_str(&amount(
        "cbc:TaxExclusiveAmount",
        &invoice.totals.tax_exclusive,
        currency,
    ));
    xml.push_str(&amount(
        "cbc:TaxInclusiveAmount",
        &invoice.totals.tax_inclusive,
        currency,
    ));
    if invoice.allowance_total != "0.00" {
        xml.push_str(&amount(
            "cbc:AllowanceTotalAmount",
            &invoice.allowance_total,
            currency,
        ));
    }
    if invoice.charge_total != "0.00" {
        xml.push_str(&amount(
            "cbc:ChargeTotalAmount",
            &invoice.charge_total,
            currency,
        ));
    }
    if invoice.prepaid != "0.00" {
        xml.push_str(&amount("cbc:PrepaidAmount", &invoice.prepaid, currency));
    }
    xml.push_str(&amount(
        "cbc:PayableAmount",
        &invoice.totals.payable,
        currency,
    ));
    xml.push_str("</cac:LegalMonetaryTotal>");
    for line in &invoice.lines {
        let line_tag = if credit_note {
            "CreditNoteLine"
        } else {
            "InvoiceLine"
        };
        xml.push_str(&format!("<cac:{line_tag}>"));
        xml.push_str(&e("cbc:ID", &line.line.id));
        xml.push_str(&element(
            if credit_note {
                "cbc:CreditedQuantity"
            } else {
                "cbc:InvoicedQuantity"
            },
            Some(&line.line.quantity),
            &format!(" unitCode=\"{}\"", escape(&line.line.unit_code)),
        ));
        xml.push_str(&amount(
            "cbc:LineExtensionAmount",
            &line.net_amount,
            currency,
        ));
        for item in &line.line.allowances {
            xml.push_str(&allowance_xml(item, currency, false));
        }
        xml.push_str("<cac:Item>");
        xml.push_str(&optional(
            "cbc:Description",
            line.line.description.as_deref(),
        ));
        xml.push_str(&e("cbc:Name", &line.line.name));
        xml.push_str("<cac:ClassifiedTaxCategory>");
        xml.push_str(&tax_category(&line.line.tax, false));
        xml.push_str("</cac:ClassifiedTaxCategory></cac:Item><cac:Price>");
        xml.push_str(&amount(
            "cbc:PriceAmount",
            &line.line.net_unit_price,
            currency,
        ));
        xml.push_str(&element(
            "cbc:BaseQuantity",
            Some("1"),
            &format!(" unitCode=\"{}\"", escape(&line.line.unit_code)),
        ));
        xml.push_str(&format!("</cac:Price></cac:{line_tag}>"));
    }
    xml.push_str(&format!("</{root}>"));
    xml
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_text_and_calculates_amounts() {
        let input: InvoiceInput = serde_json::from_str(r#"{
            "invoiceNumber":"A&B","invoiceType":"380","issueDate":"2026-10-16","currency":"EUR","buyerReference":"BR-1",
            "seller":{"name":"S","address":{"line1":"A 1","city":"Berlin","postalCode":"10115","countryCode":"DE"}},
            "buyer":{"name":"B","address":{"line1":"B 1","city":"Bonn","postalCode":"53111","countryCode":"DE"}},
            "lines":[{"id":"1","name":"Büro & Papier","quantity":"1","unitCode":"C62","netUnitPrice":"10.00","tax":{"categoryCode":"S","rate":"19"}}],
            "payment":{"meansCode":"58"}
        }"#).unwrap();
        let xml = generate_ubl(input).unwrap();
        assert!(xml.contains("<cbc:ID>A&amp;B</cbc:ID>"));
        assert!(xml.contains("Büro &amp; Papier"));
        assert!(xml.contains("<cbc:PayableAmount currencyID=\"EUR\">11.90</cbc:PayableAmount>"));
    }
}
