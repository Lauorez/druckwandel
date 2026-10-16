use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub line1: String,
    #[serde(default)]
    pub line2: Option<String>,
    pub city: String,
    pub postal_code: String,
    pub country_code: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElectronicAddress {
    pub value: String,
    pub scheme_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Party {
    pub name: String,
    pub address: Address,
    #[serde(default)]
    pub vat_id: Option<String>,
    #[serde(default)]
    pub tax_registration_id: Option<String>,
    #[serde(default)]
    pub electronic_address: Option<ElectronicAddress>,
    #[serde(default)]
    pub contact: Option<Contact>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tax {
    pub category_code: String,
    pub rate: String,
    #[serde(default)]
    pub exemption_reason: Option<String>,
    #[serde(default)]
    pub exemption_reason_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowanceCharge {
    pub charge: bool,
    pub amount: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub reason_code: Option<String>,
    #[serde(default)]
    pub percent: Option<String>,
    #[serde(default)]
    pub base_amount: Option<String>,
    pub tax: Tax,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceLine {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub quantity: String,
    pub unit_code: String,
    pub net_unit_price: String,
    pub tax: Tax,
    #[serde(default)]
    pub allowances: Vec<AllowanceCharge>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentInfo {
    pub means_code: String,
    #[serde(default)]
    pub iban: Option<String>,
    #[serde(default)]
    pub bic: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(default)]
    pub payment_reference: Option<String>,
    #[serde(default)]
    pub terms: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedingInvoice {
    pub invoice_number: String,
    #[serde(default)]
    pub issue_date: Option<String>,
    #[serde(default)]
    pub paid_amount: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceInput {
    pub invoice_number: String,
    pub invoice_type: String,
    pub issue_date: String,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub service_date: Option<String>,
    #[serde(default)]
    pub delivery_address: Option<Address>,
    pub currency: String,
    pub buyer_reference: String,
    pub seller: Party,
    pub buyer: Party,
    pub lines: Vec<InvoiceLine>,
    pub payment: PaymentInfo,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub preceding_invoice: Option<PrecedingInvoice>,
    #[serde(default)]
    pub preceding_invoices: Vec<PrecedingInvoice>,
    #[serde(default)]
    pub prepaid_amount: Option<String>,
    #[serde(default)]
    pub final_invoice: bool,
    #[serde(default)]
    pub prepayment_invoice: bool,
    #[serde(default)]
    pub allowances: Vec<AllowanceCharge>,
}

impl InvoiceInput {
    pub fn blank() -> Self {
        Self {
            invoice_number: String::new(),
            invoice_type: "380".into(),
            issue_date: String::new(),
            due_date: None,
            service_date: None,
            delivery_address: None,
            currency: "EUR".into(),
            buyer_reference: String::new(),
            seller: Party::default(),
            buyer: Party::default(),
            lines: vec![InvoiceLine {
                id: "1".into(),
                name: String::new(),
                description: None,
                quantity: "1".into(),
                unit_code: "C62".into(),
                net_unit_price: "0.00".into(),
                tax: Tax {
                    category_code: "S".into(),
                    rate: "19".into(),
                    exemption_reason: None,
                    exemption_reason_code: None,
                },
                allowances: Vec::new(),
            }],
            payment: PaymentInfo {
                means_code: "58".into(),
                ..PaymentInfo::default()
            },
            notes: Vec::new(),
            preceding_invoice: None,
            preceding_invoices: Vec::new(),
            prepaid_amount: None,
            final_invoice: false,
            prepayment_invoice: false,
            allowances: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalculatedLine {
    pub line: InvoiceLine,
    pub net_amount: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaxBreakdown {
    pub tax: Tax,
    pub taxable_amount: String,
    pub tax_amount: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InvoiceTotals {
    pub line_net: String,
    pub tax_exclusive: String,
    pub tax_total: String,
    pub tax_inclusive: String,
    pub payable: String,
}

#[derive(Clone, Debug)]
pub struct CalculatedInvoice {
    pub input: InvoiceInput,
    pub lines: Vec<CalculatedLine>,
    pub taxes: Vec<TaxBreakdown>,
    pub totals: InvoiceTotals,
    pub allowance_total: String,
    pub charge_total: String,
    pub prepaid: String,
}

fn decimal(value: &str) -> Result<Decimal, String> {
    Decimal::from_str(value).map_err(|_| format!("Ungültige Dezimalzahl: {value}"))
}

fn fixed(value: Decimal, places: u32) -> String {
    let mut rounded = value.round_dp_with_strategy(places, RoundingStrategy::MidpointAwayFromZero);
    rounded.rescale(places);
    rounded.to_string()
}

fn money(value: Decimal) -> String {
    fixed(value, 2)
}

pub fn calculate(input: InvoiceInput) -> Result<CalculatedInvoice, String> {
    let mut lines = Vec::with_capacity(input.lines.len());
    let mut tax_groups: Vec<(Tax, Decimal)> = Vec::new();
    for line in &input.lines {
        let mut amount = decimal(&line.quantity)? * decimal(&line.net_unit_price)?;
        for item in &line.allowances {
            let adjustment = decimal(&item.amount)?;
            amount = if item.charge {
                amount + adjustment
            } else {
                amount - adjustment
            };
        }
        let net_amount = money(amount);
        add_taxable(&mut tax_groups, &line.tax, decimal(&net_amount)?)?;
        lines.push(CalculatedLine {
            line: line.clone(),
            net_amount,
        });
    }
    for item in &input.allowances {
        let amount = decimal(&item.amount)?;
        add_taxable(
            &mut tax_groups,
            &item.tax,
            if item.charge { amount } else { -amount },
        )?;
    }
    let mut taxes = Vec::with_capacity(tax_groups.len());
    for (tax, taxable) in tax_groups {
        let rate = decimal(&tax.rate)?;
        taxes.push(TaxBreakdown {
            tax,
            taxable_amount: money(taxable),
            tax_amount: money(taxable * rate / Decimal::from(100)),
        });
    }
    let line_net = lines.iter().try_fold(Decimal::ZERO, |sum, line| {
        Ok::<_, String>(sum + decimal(&line.net_amount)?)
    })?;
    let (allowance_total, charge_total) = adjustment_totals(&input.allowances)?;
    let tax_exclusive = line_net - allowance_total + charge_total;
    let tax_total = taxes.iter().try_fold(Decimal::ZERO, |sum, tax| {
        Ok::<_, String>(sum + decimal(&tax.tax_amount)?)
    })?;
    let gross = tax_exclusive + tax_total;
    let prepaid = if let Some(amount) = input
        .prepaid_amount
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        decimal(amount)?
    } else {
        preceding(&input)
            .iter()
            .try_fold(Decimal::ZERO, |sum, item| {
                Ok::<_, String>(
                    sum + item
                        .paid_amount
                        .as_deref()
                        .map(decimal)
                        .transpose()?
                        .unwrap_or(Decimal::ZERO),
                )
            })?
    };
    Ok(CalculatedInvoice {
        input,
        lines,
        taxes,
        totals: InvoiceTotals {
            line_net: money(line_net),
            tax_exclusive: money(tax_exclusive),
            tax_total: money(tax_total),
            tax_inclusive: money(gross),
            payable: money(gross - prepaid),
        },
        allowance_total: money(allowance_total),
        charge_total: money(charge_total),
        prepaid: money(prepaid),
    })
}

fn add_taxable(groups: &mut Vec<(Tax, Decimal)>, tax: &Tax, amount: Decimal) -> Result<(), String> {
    let key = fixed(decimal(&tax.rate)?, 2);
    if let Some((_, taxable)) = groups.iter_mut().find(|(existing, _)| {
        existing.category_code == tax.category_code
            && fixed(decimal(&existing.rate).expect("validated tax group"), 2) == key
    }) {
        *taxable += amount;
    } else {
        groups.push((tax.clone(), amount));
    }
    Ok(())
}

pub fn adjustment_totals(items: &[AllowanceCharge]) -> Result<(Decimal, Decimal), String> {
    items.iter().try_fold(
        (Decimal::ZERO, Decimal::ZERO),
        |(allowance, charge), item| {
            let amount = decimal(&item.amount)?;
            Ok(if item.charge {
                (allowance, charge + amount)
            } else {
                (allowance + amount, charge)
            })
        },
    )
}

pub fn preceding(input: &InvoiceInput) -> Vec<&PrecedingInvoice> {
    if !input.preceding_invoices.is_empty() {
        input
            .preceding_invoices
            .iter()
            .filter(|item| !item.invoice_number.trim().is_empty())
            .collect()
    } else {
        input
            .preceding_invoice
            .as_ref()
            .filter(|item| !item.invoice_number.trim().is_empty())
            .into_iter()
            .collect()
    }
}

pub fn validate(input: &InvoiceInput) -> Vec<String> {
    let mut errors = Vec::new();
    let required = [
        (&input.invoice_number, "Rechnungsnummer"),
        (&input.seller.name, "Verkäufer"),
        (&input.buyer.name, "Käufer"),
        (&input.buyer_reference, "Bestellnummer oder Leitweg-ID"),
        (&input.seller.address.line1, "Straße des Verkäufers"),
        (&input.seller.address.city, "Ort des Verkäufers"),
        (&input.seller.address.postal_code, "PLZ des Verkäufers"),
        (&input.buyer.address.line1, "Straße des Käufers"),
        (&input.buyer.address.city, "Ort des Käufers"),
        (&input.buyer.address.postal_code, "PLZ des Käufers"),
    ];
    for (value, name) in required {
        if value.trim().is_empty() {
            errors.push(format!("{name} fehlt."));
        }
    }
    if !["380", "381", "384", "326"].contains(&input.invoice_type.as_str()) {
        errors.push("Rechnungstyp ist ungültig.".into());
    }
    if !valid_date(&input.issue_date) {
        errors.push("Rechnungsdatum ist ungültig.".into());
    }
    for (value, name) in [
        (input.due_date.as_deref(), "Fälligkeitsdatum"),
        (input.service_date.as_deref(), "Leistungsdatum"),
    ] {
        if let Some(value) = value.filter(|v| !v.is_empty()) {
            if !valid_date(value) {
                errors.push(format!("{name} ist ungültig."));
            }
        }
    }
    if input.currency.len() != 3 || !input.currency.bytes().all(|c| c.is_ascii_uppercase()) {
        errors.push("Währung muss aus drei Großbuchstaben bestehen.".into());
    }
    for (party, name) in [(&input.seller, "Verkäufer"), (&input.buyer, "Käufer")] {
        if party.address.country_code.len() != 2
            || !party
                .address
                .country_code
                .bytes()
                .all(|c| c.is_ascii_uppercase())
        {
            errors.push(format!("Ländercode für {name} ist ungültig."));
        }
    }
    if input.lines.is_empty() {
        errors.push("Mindestens eine Position ist erforderlich.".into());
    }
    for (index, line) in input.lines.iter().enumerate() {
        if line.id.trim().is_empty() || line.name.trim().is_empty() {
            errors.push(format!("Position {} ist unvollständig.", index + 1));
        }
        if decimal(&line.quantity).map_or(true, |value| value <= Decimal::ZERO) {
            errors.push(format!("Menge der Position {} ist ungültig.", index + 1));
        }
        if decimal(&line.net_unit_price).map_or(true, |value| value < Decimal::ZERO) {
            errors.push(format!("Preis der Position {} ist ungültig.", index + 1));
        }
        validate_tax(&line.tax, &mut errors);
        for item in &line.allowances {
            validate_adjustment(item, &mut errors);
        }
    }
    for item in &input.allowances {
        validate_adjustment(item, &mut errors);
    }
    if input.final_invoice && input.prepayment_invoice {
        errors.push("Anzahlung und Schlussrechnung schließen sich aus.".into());
    }
    if (input.final_invoice || input.prepayment_invoice) && input.invoice_type != "380" {
        errors.push("Anzahlung und Schlussrechnung müssen Rechnungstyp 380 verwenden.".into());
    }
    if ["381", "384"].contains(&input.invoice_type.as_str()) && preceding(input).is_empty() {
        errors.push("Bezug zur Ursprungsrechnung fehlt.".into());
    }
    if let Some(amount) = &input.prepaid_amount {
        if decimal(amount).map_or(true, |value| value < Decimal::ZERO) {
            errors.push("Bereits gezahlter Betrag ist ungültig.".into());
        }
    }
    errors
}

fn validate_adjustment(item: &AllowanceCharge, errors: &mut Vec<String>) {
    if decimal(&item.amount).map_or(true, |value| value <= Decimal::ZERO || value.scale() > 2) {
        errors.push("Zu- oder Abschlag benötigt einen positiven Betrag mit höchstens zwei Nachkommastellen.".into());
    }
    validate_tax(&item.tax, errors);
}

fn validate_tax(tax: &Tax, errors: &mut Vec<String>) {
    let Ok(rate) = decimal(&tax.rate) else {
        errors.push("Steuersatz ist ungültig.".into());
        return;
    };
    match tax.category_code.as_str() {
        "S" if rate <= Decimal::ZERO => {
            errors.push("Standardsteuer benötigt einen Satz über 0 %.".into())
        }
        "Z" | "E" | "AE" | "K" if rate != Decimal::ZERO => {
            errors.push("Dieser Steuerfall benötigt 0 %.".into())
        }
        "E" | "AE" | "K" if tax.exemption_reason.as_deref().is_none_or(str::is_empty) => {
            errors.push("Begründung für steuerfreie Position fehlt.".into())
        }
        "S" | "Z" | "E" | "AE" | "K" => {}
        _ => errors.push("Steuerkategorie ist ungültig.".into()),
    }
}

fn valid_date(value: &str) -> bool {
    let parts: Vec<_> = value.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return false;
    }
    let Ok(year) = parts[0].parse::<i32>() else {
        return false;
    };
    let Ok(month) = parts[1].parse::<u32>() else {
        return false;
    };
    let Ok(day) = parts[2].parse::<u32>() else {
        return false;
    };
    if year < 1 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    (1..=days[month as usize - 1]).contains(&day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_mixed_taxes_and_prepayment() {
        let input: InvoiceInput = serde_json::from_str(r#"{
            "invoiceNumber":"R-1","invoiceType":"380","issueDate":"2026-10-16","currency":"EUR","buyerReference":"B-1",
            "seller":{"name":"A","address":{"line1":"A 1","city":"Berlin","postalCode":"10115","countryCode":"DE"}},
            "buyer":{"name":"B","address":{"line1":"B 1","city":"Bonn","postalCode":"53111","countryCode":"DE"}},
            "lines":[
                {"id":"1","name":"A","quantity":"2","unitCode":"C62","netUnitPrice":"10.005","tax":{"categoryCode":"S","rate":"19"}},
                {"id":"2","name":"B","quantity":"1","unitCode":"C62","netUnitPrice":"7.00","tax":{"categoryCode":"S","rate":"7"}}
            ],
            "payment":{"meansCode":"58"},"prepaidAmount":"5.00"
        }"#).unwrap();
        assert!(validate(&input).is_empty());
        let result = calculate(input).unwrap();
        assert_eq!(result.totals.line_net, "27.01");
        assert_eq!(result.totals.tax_total, "4.29");
        assert_eq!(result.totals.payable, "26.30");
    }
}
