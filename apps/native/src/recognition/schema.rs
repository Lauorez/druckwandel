use super::*;
use chrono::{Duration, NaiveDate};
use regex::Regex;
use rust_decimal::{Decimal, RoundingStrategy};
use std::str::FromStr;

pub struct SchemaRecognizer;
fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static recognition pattern")
}
fn decimal(value: &str) -> Option<String> {
    let compact = value.trim().replace(['€', '%', ' '], "");
    let normalized = if compact.contains(',') {
        compact.replace('.', "").replace(',', ".")
    } else {
        compact
    };
    Decimal::from_str(&normalized)
        .ok()
        .map(|n| n.normalize().to_string())
}
fn money(value: &str) -> Option<String> {
    decimal(value)
        .and_then(|v| Decimal::from_str(&v).ok())
        .map(|n| {
            format!(
                "{:.2}",
                n.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
            )
        })
}
fn date(value: &str) -> Option<String> {
    if let Some(c) = re(r"\b(\d{4})-(\d{2})-(\d{2})\b").captures(value) {
        return NaiveDate::from_ymd_opt(c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?)
            .map(|date| date.to_string());
    }
    if let Some(c) = re(r"\b(\d{1,2})[./-](\d{1,2})[./-](\d{2,4})\b").captures(value) {
        let mut year = c[3].parse::<i32>().ok()?;
        if year < 100 {
            year += 2000;
        }
        return NaiveDate::from_ymd_opt(year, c[2].parse().ok()?, c[1].parse().ok()?)
            .map(|date| date.to_string());
    }
    let c = re(r"(?i)\b(\d{1,2})\.?\s*([a-zäöü]+)\.?\s+(\d{4})\b").captures(value)?;
    let months = [
        "jan", "feb", "mär", "apr", "mai", "jun", "jul", "aug", "sep", "okt", "nov", "dez",
    ];
    let name = c[2].to_lowercase().replace("maerz", "mär");
    let month = months.iter().position(|month| name.starts_with(month))? as u32 + 1;
    NaiveDate::from_ymd_opt(c[3].parse().ok()?, month, c[1].parse().ok()?)
        .map(|date| date.to_string())
}

impl RecognitionProvider for SchemaRecognizer {
    fn method(&self) -> RecognitionMethod {
        RecognitionMethod::Schema
    }
    fn recognize(&self, request: &RecognitionRequest) -> Result<RecognitionResult, String> {
        let lines = reconstruct_lines(&request.pages);
        let mut result = RecognitionResult {
            schema_version: CONTRACT_VERSION,
            document_sha256: request.document_sha256.clone(),
            method: self.method(),
            provider_version: "schema-rust-1".into(),
            fields: BTreeMap::new(),
            lines: vec![],
            warnings: vec![],
        };
        let amount = r"(-?[0-9][0-9. ]*(?:,[0-9]{1,4})|-?[0-9]+(?:\.[0-9]{1,4})?)";
        let rules = [
            (
                "invoiceNumber",
                r"(?i)(?:rechnungs(?:nummer|nr\.?|\s*nr\.?)|beleg[- ]?nr\.?|invoice\s*(?:number|no\.?))",
                r"[:#\s]*([A-Z0-9][A-Z0-9/_-]{1,})",
                0.94,
            ),
            (
                "buyerReference",
                r"(?i)(?:leitweg[- ]?id|bestell(?:nummer|nr\.?)|buyer reference|käuferreferenz)",
                r"[:#\s]*([A-Z0-9][A-Z0-9._/-]{1,})",
                0.90,
            ),
            (
                "sellerVatId",
                r"(?i)(?:ust\.?-?id(?:nr)?\.?|umsatzsteuer-id|vat id)",
                r"(?i)\b([A-Z]{2}[A-Z0-9 ]{8,16})\b",
                0.96,
            ),
            (
                "sellerContact",
                r"(?i)(?:ansprechpartner(?:in)?|kontakt(?:person)?)",
                r"[:#\s]*(.+)",
                0.90,
            ),
            (
                "sellerPhone",
                r"(?i)(?:telefon|tel\.?|fon)\b",
                r"[:#\s]*(\+?[0-9][0-9 /().-]{6,})",
                0.90,
            ),
            (
                "sellerEmail",
                r"(?i)e-?mail",
                r"(?i)([A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,})",
                0.96,
            ),
            (
                "iban",
                r"(?i)\bIBAN\b",
                r"(?i)\b([A-Z]{2}\s?[0-9]{2}(?:\s?[A-Z0-9]){11,30})\b",
                0.98,
            ),
            (
                "bic",
                r"(?i)\b(?:BIC|SWIFT)\b",
                r"(?i)[:#\s]*([A-Z]{6}[A-Z0-9]{2}(?:[A-Z0-9]{3})?)",
                0.96,
            ),
            (
                "lineNet",
                r"(?i)(?:nettobetrag|summe netto|net total|^\s*netto\s*:)",
                amount,
                0.91,
            ),
            (
                "taxTotal",
                r"(?i)\b(?:umsatzsteuer|mwst|ust|vat)\b\.?(?:\s+\d+[,.]?\d*\s*%)?",
                amount,
                0.89,
            ),
            (
                "taxInclusive",
                r"(?i)(?:bruttobetrag|summe brutto|gross total|^\s*gesamt\s*:)",
                amount,
                0.92,
            ),
            (
                "payable",
                r"(?i)(?:zahlbetrag|rechnungsbetrag|gesamtbetrag|zu zahlen|restbetrag|noch zu zahlen|amount due|^\s*gesamt\s*:)",
                amount,
                0.95,
            ),
            (
                "paymentTerms",
                r"(?i)^(?:zahlungsbedingungen|zahlungsziel|payment terms)\s*:?",
                r"\s*(.+)",
                0.86,
            ),
        ];
        for (name, label, pattern, confidence) in rules {
            let label = re(label);
            let pattern = re(&format!("(?i){pattern}"));
            for line in &lines {
                let Some(label) = label.find(&line.text) else {
                    continue;
                };
                if name == "taxTotal"
                    && line.text[label.end()..]
                        .trim_start_matches([' ', '.', '-'])
                        .to_lowercase()
                        .starts_with("id")
                {
                    continue;
                }
                let Some(c) = pattern.captures(&line.text[label.end()..]) else {
                    continue;
                };
                let raw = c[1].trim();
                let value = if ["lineNet", "taxTotal", "taxInclusive", "payable"].contains(&name) {
                    money(raw)
                } else if ["sellerVatId", "iban", "bic"].contains(&name) {
                    let compact = raw.replace(' ', "").to_uppercase();
                    Some(if name == "iban" && compact.starts_with("DE") {
                        compact.chars().take(22).collect()
                    } else {
                        compact
                    })
                } else {
                    Some(raw.to_owned())
                };
                if let Some(value) = value {
                    result
                        .fields
                        .insert(name.into(), proposal(value, line, confidence));
                    break;
                }
            }
        }
        for (name, pattern, confidence) in [
            (
                "issueDate",
                r"(?i)(?:rechnungsdatum|belegdatum|ausstellungsdatum|invoice date|^\s*datum\s*:)",
                0.88,
            ),
            (
                "dueDate",
                r"(?i)(?:fällig(?:keit| am)?|faellig(?:keit| am)?|zahlbar bis|due date)",
                0.92,
            ),
            (
                "serviceDate",
                r"(?i)(?:leistungsdatum|lieferdatum|delivery date)",
                0.91,
            ),
        ] {
            let label = re(pattern);
            if let Some((line, value)) = lines
                .iter()
                .filter(|line| label.is_match(&line.text))
                .find_map(|line| date(&line.text).map(|value| (line, value)))
            {
                result
                    .fields
                    .insert(name.into(), proposal(value, line, confidence));
            }
        }
        if !result.fields.contains_key("dueDate") {
            if let Some(issue) = result
                .fields
                .get("issueDate")
                .and_then(|field| NaiveDate::parse_from_str(&field.value, "%Y-%m-%d").ok())
            {
                let relative = re(
                    r"(?i)(?:zahlungsziel\s*:?|zahlbar\s*(?:innerhalb(?:\s+von)?|binnen|in)?|innerhalb(?:\s+von)?|binnen|netto)\s*(\d{1,3})\s+tagen?|\b(\d{1,3})\s+tage\s+netto",
                );
                for line in &lines {
                    if let Some(c) = relative.captures(&line.text) {
                        let days = c
                            .get(1)
                            .or_else(|| c.get(2))
                            .and_then(|m| m.as_str().parse::<i64>().ok())
                            .unwrap_or(0);
                        if (1..=366).contains(&days) {
                            result.fields.insert(
                                "dueDate".into(),
                                proposal((issue + Duration::days(days)).to_string(), line, 0.86),
                            );
                            break;
                        }
                    }
                }
            }
        }
        let currency = re(r"(?i)\b(EUR|USD|GBP|CHF)\b|€|\$|£");
        for line in &lines {
            if let Some(c) = currency.find(&line.text) {
                let v = c.as_str().to_uppercase();
                let v = match v.as_str() {
                    "€" => "EUR",
                    "$" => "USD",
                    "£" => "GBP",
                    _ => &v,
                };
                result
                    .fields
                    .insert("currency".into(), proposal(v.into(), line, 0.90));
                break;
            }
        }
        classify_parties(&lines, &mut result);
        classify_tables(&lines, &mut result);
        if request.pages.iter().all(|page| page.tokens.is_empty()) {
            result.warnings.push("Die PDF enthält keinen lesbaren Text. Bitte Daten manuell erfassen; OCR ist noch nicht angebunden.".into());
        }
        let amounts: Option<Vec<Decimal>> = ["lineNet", "taxTotal", "taxInclusive"]
            .iter()
            .map(|key| {
                result
                    .fields
                    .get(*key)
                    .and_then(|field| Decimal::from_str(&field.value).ok())
            })
            .collect();
        if let Some(values) = amounts {
            if values[0].checked_add(values[1]) != Some(values[2]) {
                result.warnings.push("Erkanntes Netto plus Steuer entspricht nicht dem Brutto. Manuelle Prüfung erforderlich.".into());
                for key in ["lineNet", "taxTotal", "taxInclusive"] {
                    if let Some(field) = result.fields.get_mut(key) {
                        field.confidence = field.confidence.min(0.6);
                    }
                }
            }
        }
        if let Some(net) = result
            .fields
            .get("lineNet")
            .and_then(|f| Decimal::from_str(&f.value).ok())
        {
            let sum: Option<Decimal> = result
                .lines
                .iter()
                .filter_map(|line| Decimal::from_str(&line.net_amount).ok())
                .try_fold(Decimal::ZERO, |sum, value| sum.checked_add(value));
            if !result.lines.is_empty() && Some(net) != sum {
                result.warnings.push(
                    "Positionssumme weicht vom erkannten Netto ab. Manuelle Prüfung erforderlich."
                        .into(),
                );
                for line in &mut result.lines {
                    line.confidence = line.confidence.min(0.6);
                }
            }
        }
        Ok(result)
    }
}

fn classify_parties(lines: &[crate::extraction::TextLine], result: &mut RecognitionResult) {
    let postal = re(r"^\s*(?:[A-Z]{2}[- ]?)?(\d{4,6})\s+([\p{L}][\p{L} .-]{1,39})\s*$");
    let street = re(
        r"(?i)(?:str(?:a(?:ss|ß)e)?\.?|weg|platz|gasse|allee|ring|damm|ufer|chaussee|hof|markt)\b.*\d|\d+[a-z]?\s*$",
    );
    let skip = re(
        r"(?i)^(?:z\.?\s*h|c/o|tel\.?|telefon|fax|mobil|e-?mail|www\.|https?:|ust\.?-?id|iban|bic|steuernr|rechnung(?:snummer|sdatum)?)\b",
    );
    let marker = re(
        r"(?i)^(?:rechnung\s+an|rechnungsempfänger(?:in)?|rechnungsempfaenger(?:in)?|rechnungsadresse|kundenadresse|lieferanschrift|bill(?:ed)?\s+to|empfänger(?:in)?|empfaenger(?:in)?|kunde|an)\s*:?\s*(.*)$",
    );
    let buyer = lines
        .iter()
        .position(|line| marker.is_match(line.text.trim()));
    for (prefix, from, until, confidence) in [
        (
            "seller",
            0,
            buyer.unwrap_or(lines.len().min(16)),
            if buyer.is_some() { 0.82 } else { 0.74 },
        ),
        (
            "buyer",
            buyer.map_or(lines.len(), |i| i + 1),
            buyer.map_or(lines.len(), |i| (i + 8).min(lines.len())),
            0.84,
        ),
    ] {
        let range = &lines[from..until];
        let Some((city_index, city)) = range
            .iter()
            .enumerate()
            .find(|(_, line)| postal.is_match(&line.text))
        else {
            continue;
        };
        let Some((street_index, address)) = range[..city_index]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, line)| street.is_match(&line.text) && !skip.is_match(&line.text))
        else {
            continue;
        };
        let inline = if prefix == "buyer" {
            buyer.and_then(|i| {
                marker
                    .captures(lines[i].text.trim())
                    .and_then(|c| c.get(1))
                    .filter(|m| !m.as_str().is_empty())
                    .map(|m| (i, m.as_str().to_owned()))
            })
        } else {
            None
        };
        let name = range[..street_index]
            .iter()
            .rev()
            .find(|line| !skip.is_match(&line.text) && line.text.chars().any(char::is_alphabetic));
        if let Some((index, name)) = inline {
            result.fields.insert(
                format!("{prefix}Name"),
                proposal(name, &lines[index], confidence),
            );
        } else if let Some(name) = name {
            result.fields.insert(
                format!("{prefix}Name"),
                proposal(name.text.clone(), name, confidence),
            );
        } else {
            continue;
        }
        let c = postal.captures(&city.text).unwrap();
        result.fields.insert(
            format!("{prefix}AddressLine1"),
            proposal(address.text.clone(), address, confidence),
        );
        result.fields.insert(
            format!("{prefix}PostalCode"),
            proposal(c[1].into(), city, confidence),
        );
        result.fields.insert(
            format!("{prefix}City"),
            proposal(c[2].into(), city, confidence),
        );
        let country = range.get(city_index + 1).and_then(|line| {
            match line.text.trim().to_uppercase().as_str() {
                "DEUTSCHLAND" | "GERMANY" | "DE" => Some(("DE", line)),
                "ÖSTERREICH" | "OESTERREICH" | "AUSTRIA" | "AT" => Some(("AT", line)),
                "SCHWEIZ" | "SWITZERLAND" | "CH" => Some(("CH", line)),
                _ => None,
            }
        });
        let (code, source) = country.unwrap_or(("DE", city));
        result.fields.insert(
            format!("{prefix}CountryCode"),
            proposal(
                code.into(),
                source,
                if country.is_some() { confidence } else { 0.72 },
            ),
        );
    }
}

fn classify_tables(lines: &[crate::extraction::TextLine], result: &mut RecognitionResult) {
    let tax_pattern = re(r"(?i)(?:umsatzsteuer|mwst\.?|ust\.?|vat)[^%\d]*(\d+(?:[.,]\d+)?)\s*%");
    let tax_rate = lines.iter().find_map(|line| {
        tax_pattern
            .captures(&line.text)
            .and_then(|c| decimal(&c[1]))
    });
    let summary = re(
        r"(?i)^(?:netto|zwischensumme|umsatzsteuer|mwst\.?|ust\.?|gesamt|zahlbetrag|zahlungsbedingungen)\b",
    );
    let headers = [
        re(r"(?i)^(?:beschreibung|leistung|artikel|description)$"),
        re(r"(?i)^(?:menge|anzahl|qty\.?|quantity)$"),
        re(r"(?i)^(?:einzelpreis|stückpreis|preis|unit.?price)$"),
        re(r"(?i)^(?:gesamt|betrag|summe|total)$"),
    ];
    let quantity = re(r"^(.*?)\s+(-?\d+(?:[.,]\d+)?)\s*([\p{L}]{1,8}\.?)?$");
    let mut guide = None;
    for line in lines {
        let positions: Vec<_> = headers
            .iter()
            .map(|pattern| {
                line.tokens
                    .iter()
                    .find(|t| pattern.is_match(&t.text))
                    .map(|t| t.bounds.x)
            })
            .collect();
        if positions.iter().all(Option::is_some) {
            guide = Some((
                line.page,
                positions
                    .into_iter()
                    .map(Option::unwrap)
                    .collect::<Vec<_>>(),
            ));
            continue;
        }
        if summary.is_match(line.text.trim()) {
            guide = None;
            continue;
        }
        let Some((page, x)) = &guide else { continue };
        if *page != line.page {
            guide = None;
            continue;
        }
        let price_boundary = (x[1] + x[2]) / 2.0;
        let total_boundary = (x[2] + x[3]) / 2.0;
        let description = line
            .tokens
            .iter()
            .filter(|t| t.bounds.x >= x[0] - 4.0 && t.bounds.x < price_boundary)
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let Some(c) = quantity.captures(&description) else {
            continue;
        };
        let Some(price) = line
            .tokens
            .iter()
            .filter(|t| t.bounds.x >= price_boundary && t.bounds.x < total_boundary)
            .find_map(|t| decimal(&t.text))
        else {
            continue;
        };
        let Some((total_token, total)) = line
            .tokens
            .iter()
            .filter(|t| t.bounds.x >= total_boundary)
            .find_map(|t| money(&t.text).map(|value| (t, value)))
        else {
            continue;
        };
        let Some(qty) = decimal(&c[2]) else { continue };
        let explicit_rate = line
            .tokens
            .iter()
            .filter(|t| {
                t.bounds.x > total_token.bounds.x + total_token.bounds.width && t.text.contains('%')
            })
            .find_map(|t| decimal(&t.text));
        let rate = explicit_rate.or_else(|| tax_rate.clone());
        if rate.is_none() {
            result.warnings.push(
                "Steuersatz einer Position konnte nicht erkannt werden. Bitte ergänzen.".into(),
            );
        }
        let rate = rate.unwrap_or_else(|| "19".into());
        let name = re(r"^\d+[.)]?\s+").replace(c[1].trim(), "").into_owned();
        let unit = match c
            .get(3)
            .map(|m| m.as_str().trim_end_matches('.').to_lowercase())
            .as_deref()
        {
            Some("std" | "h" | "stunden") => "HUR",
            Some("m") => "MTR",
            Some("kg") => "KGM",
            _ => "C62",
        };
        let calculated = Decimal::from_str(&qty)
            .ok()
            .zip(Decimal::from_str(&price).ok())
            .and_then(|(quantity, price)| quantity.checked_mul(price))
            .map(|amount| amount.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero));
        let arithmetic_matches = calculated
            .zip(Decimal::from_str(&total).ok())
            .is_some_and(|(a, b)| a == b);
        if !arithmetic_matches {
            result.warnings.push(format!("Position {}: Menge × Einzelpreis weicht vom gedruckten Positionsbetrag ab. Bitte den ungerundeten Einzelpreis oder die Menge prüfen.",result.lines.len()+1));
        }
        result.lines.push(LineProposal {
            line: InvoiceLine {
                id: (result.lines.len() + 1).to_string(),
                name,
                description: None,
                quantity: qty,
                unit_code: unit.into(),
                net_unit_price: price,
                tax: Tax {
                    category_code: if rate == "0" { "Z" } else { "S" }.into(),
                    rate,
                    exemption_reason: None,
                    exemption_reason_code: None,
                },
                allowances: vec![],
            },
            net_amount: total,
            confidence: if arithmetic_matches { 0.91 } else { 0.60 },
            source_token_ids: line.tokens.iter().map(|t| t.id.clone()).collect(),
            source_text: line.text.clone(),
        });
    }
}
