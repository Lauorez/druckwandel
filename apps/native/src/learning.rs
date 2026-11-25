//! Explicitly learned source assignments, scoped to a document layout and issuer.
//! Rules contain regions/anchors, never a memorized invoice number or date.
use crate::{
    extraction::{BoundingBox, SourceToken, TextLine, reconstruct_lines},
    invoice::{InvoiceInput, InvoiceLine, Tax},
    recognition::{
        FieldProposal, LineProposal, RecognitionRequest, RecognitionResult,
        schema::{date, decimal},
    },
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::Path,
    str::FromStr,
};

const FIELDS: &[(&str, &[&str])] = &[
    ("invoiceNumber", &["invoiceNumber"]),
    ("issueDate", &["issueDate"]),
    ("dueDate", &["dueDate"]),
    ("serviceDate", &["serviceDate"]),
    ("currency", &["currency"]),
    ("buyerReference", &["buyerReference"]),
    ("sellerName", &["seller", "name"]),
    ("sellerAddressLine1", &["seller", "address", "line1"]),
    ("sellerPostalCode", &["seller", "address", "postalCode"]),
    ("sellerCity", &["seller", "address", "city"]),
    ("sellerCountryCode", &["seller", "address", "countryCode"]),
    ("sellerVatId", &["seller", "vatId"]),
    ("buyerName", &["buyer", "name"]),
    ("buyerAddressLine1", &["buyer", "address", "line1"]),
    ("buyerPostalCode", &["buyer", "address", "postalCode"]),
    ("buyerCity", &["buyer", "address", "city"]),
    ("buyerCountryCode", &["buyer", "address", "countryCode"]),
    ("buyerVatId", &["buyer", "vatId"]),
    ("iban", &["payment", "iban"]),
    ("bic", &["payment", "bic"]),
    ("paymentTerms", &["payment", "terms"]),
];

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Memory {
    pub fields: Vec<FieldRule>,
    pub tables: Vec<TableRule>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub memory: Memory,
    pub undo: Option<Memory>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ProfileStore {
    pub version: u32,
    pub active: String,
    pub profiles: Vec<Profile>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Scope {
    pub identities: Vec<String>,
    pub anchors: BTreeSet<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct FieldRule {
    pub id: String,
    pub field: String,
    pub enabled: bool,
    pub scope: Scope,
    pub page: u32,
    pub region: BoundingBox,
    pub right: f64,
    pub prefix: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct TableRule {
    pub id: String,
    pub enabled: bool,
    pub scope: Scope,
    pub page: u32,
    pub start_y: f64,
    pub columns: [f64; 4],
    pub tax: Tax,
    pub unit: String,
}
impl Default for ProfileStore {
    fn default() -> Self {
        Self {
            version: 1,
            active: "standard".into(),
            profiles: vec![Profile {
                id: "standard".into(),
                name: "Standard".into(),
                memory: Memory::default(),
                undo: None,
            }],
        }
    }
}

impl ProfileStore {
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join("learning-profiles.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = crate::file_io::read_limited(&path, 10 * 1024 * 1024)
            .map_err(|_| "Lernprofile sind zu groß oder ungültig.".to_string())?;
        let store: Self = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Lernprofile konnten nicht geladen werden: {e}"))?;
        store.validate()?;
        Ok(store)
    }
    fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.profiles.is_empty()
            || self.profiles.len() > 20
            || !self.profiles.iter().any(|p| p.id == self.active)
        {
            return Err("Ungültiger Lernprofil-Speicher.".into());
        }
        let mut ids = BTreeSet::new();
        for profile in &self.profiles {
            if !ids.insert(&profile.id)
                || profile.name.trim().is_empty()
                || profile.name.len() > 320
            {
                return Err("Ungültiges Lernprofil.".into());
            }
            for memory in std::iter::once(&profile.memory).chain(profile.undo.iter()) {
                if memory.fields.len() > 250 || memory.tables.len() > 50 {
                    return Err("Zu viele Lernregeln.".into());
                }
                for rule in &memory.fields {
                    if !FIELDS.iter().any(|(name, _)| *name == rule.field)
                        || rule.page == 0
                        || ![
                            rule.region.x,
                            rule.region.y,
                            rule.region.width,
                            rule.region.height,
                            rule.right,
                        ]
                        .iter()
                        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                        || rule.right <= rule.region.x
                    {
                        return Err("Ungültige gelernte Fundstelle.".into());
                    }
                }
                for rule in &memory.tables {
                    if rule.page == 0
                        || !rule.start_y.is_finite()
                        || !(0.0..=1.0).contains(&rule.start_y)
                        || !rule
                            .columns
                            .iter()
                            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                        || !rule.columns.windows(2).all(|v| v[0] < v[1])
                    {
                        return Err("Ungültige gelernte Tabelle.".into());
                    }
                }
            }
        }
        Ok(())
    }
    pub fn save(&self, root: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        if bytes.len() > 10 * 1024 * 1024 {
            return Err("Lernprofile überschreiten 10 MB.".into());
        }
        let mut file = tempfile::NamedTempFile::new_in(root).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(root.join("learning-profiles.json"))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn active(&self) -> &Profile {
        self.profiles
            .iter()
            .find(|p| p.id == self.active)
            .expect("validated profile store")
    }
    pub fn active_mut(&mut self) -> &mut Profile {
        self.profiles
            .iter_mut()
            .find(|p| p.id == self.active)
            .expect("validated profile store")
    }
    pub fn create(&mut self, name: &str) -> Result<(), String> {
        let name = clean_name(name)?;
        if self.profiles.len() >= 20 {
            return Err("Höchstens 20 Lernprofile sind möglich.".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.active = id.clone();
        self.profiles.push(Profile {
            id,
            name,
            memory: Memory::default(),
            undo: None,
        });
        Ok(())
    }
    pub fn rename(&mut self, name: &str) -> Result<(), String> {
        self.active_mut().name = clean_name(name)?;
        Ok(())
    }
    pub fn delete(&mut self) -> Result<(), String> {
        if self.profiles.len() <= 1 {
            return Err("Das letzte Lernprofil kann nicht gelöscht werden.".into());
        }
        self.profiles.retain(|p| p.id != self.active);
        self.active = self.profiles[0].id.clone();
        Ok(())
    }
    pub fn clear(&mut self) {
        let p = self.active_mut();
        p.undo = Some(p.memory.clone());
        p.memory = Memory::default();
    }
    pub fn undo(&mut self) -> Result<(), String> {
        let p = self.active_mut();
        p.memory = p
            .undo
            .take()
            .ok_or("Keine Änderung zum Rückgängigmachen vorhanden.")?;
        Ok(())
    }
    pub fn edit_rule(&mut self, id: &str, remove: bool) -> Result<(), String> {
        let p = self.active_mut();
        p.undo = Some(p.memory.clone());
        if let Some(index) = p.memory.fields.iter().position(|r| r.id == id) {
            if remove {
                p.memory.fields.remove(index);
            } else {
                p.memory.fields[index].enabled = !p.memory.fields[index].enabled;
            }
        } else if let Some(index) = p.memory.tables.iter().position(|r| r.id == id) {
            if remove {
                p.memory.tables.remove(index);
            } else {
                p.memory.tables[index].enabled = !p.memory.tables[index].enabled;
            }
        } else {
            return Err("Lernregel wurde nicht gefunden.".into());
        }
        Ok(())
    }
    pub fn learn(
        &mut self,
        request: &RecognitionRequest,
        invoice: &InvoiceInput,
    ) -> Result<String, String> {
        if request.pages.iter().map(|p| p.tokens.len()).sum::<usize>() > 30_000 {
            return Err("Zu viele Wörter zum Lernen. Bitte die Rechnung aufteilen.".into());
        }
        let scope = Scope::new(request, invoice);
        if scope.anchors.len() < 4 {
            return Err("Zu wenig Text für ein verlässliches Lernprofil vorhanden.".into());
        }
        let json = serde_json::to_value(invoice).map_err(|e| e.to_string())?;
        let lines = reconstruct_lines(&request.pages);
        let mut learned = vec![];
        for (field, path) in FIELDS {
            let Some(value) = path
                .iter()
                .try_fold(&json, |value, key| value.get(*key))
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
            else {
                continue;
            };
            if let Some((line, start, end)) = locate(&lines, field, value) {
                let page = request
                    .pages
                    .iter()
                    .find(|p| p.page == line.page)
                    .ok_or("Unbekannte Seite.")?;
                if !page.width.is_finite()
                    || !page.height.is_finite()
                    || page.width <= 0.0
                    || page.height <= 0.0
                {
                    continue;
                }
                let tokens = &line.tokens[start..end];
                let first = &tokens[0].bounds;
                let last = &tokens[tokens.len() - 1].bounds;
                let x = (first.x / page.width).clamp(0.0, 1.0);
                let y = (first.y / page.height).clamp(0.0, 1.0);
                let width = ((last.x + last.width - first.x) / page.width).clamp(0.0, 1.0);
                let height = (first.height / page.height).clamp(0.0, 1.0);
                let right = line.tokens.get(end).map_or(1.0, |t| {
                    ((last.x + last.width + t.bounds.x) / 2.0 / page.width).clamp(0.0, 1.0)
                });
                if ![x, y, width, height, right].iter().all(|v| v.is_finite()) || right <= x {
                    continue;
                }
                let prefix = line.tokens[..start]
                    .iter()
                    .rev()
                    .take(2)
                    .map(|t| normalize(&t.text))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                learned.push(FieldRule {
                    id: uuid::Uuid::new_v4().to_string(),
                    field: (*field).into(),
                    enabled: true,
                    scope: scope.clone(),
                    page: page.page,
                    region: BoundingBox {
                        x,
                        y,
                        width,
                        height,
                    },
                    right,
                    prefix,
                });
            }
        }
        let tables = learn_tables(request, invoice, &scope, &lines);
        let count = learned.len();
        let table_count = tables.len();
        if count == 0 && table_count == 0 {
            return Err("Keine korrigierten Werte konnten eindeutig im Beleg gefunden werden. Es werden keine festen Rechnungswerte auswendig gespeichert.".into());
        }
        let profile = self.active_mut();
        profile.undo = Some(profile.memory.clone());
        for rule in learned {
            profile.memory.fields.retain(|r| {
                !(r.field == rule.field
                    && r.page == rule.page
                    && r.scope.anchors == rule.scope.anchors
                    && r.scope.identities == rule.scope.identities)
            });
            profile.memory.fields.push(rule);
        }
        for rule in tables {
            profile.memory.tables.retain(|r| {
                !(r.page == rule.page
                    && r.scope.anchors == rule.scope.anchors
                    && r.scope.identities == rule.scope.identities)
            });
            profile.memory.tables.push(rule);
        }
        if profile.memory.fields.len() > 250 {
            let n = profile.memory.fields.len() - 250;
            profile.memory.fields.drain(..n);
        }
        if profile.memory.tables.len() > 50 {
            let n = profile.memory.tables.len() - 50;
            profile.memory.tables.drain(..n);
        }
        Ok(format!(
            "Profil {}: {count} Feldzuordnungen und {table_count} Tabellen gelernt. Nur am Beleg gefundene Angaben wurden übernommen.",
            profile.name
        ))
    }

    pub fn apply(&self, request: &RecognitionRequest, result: &mut RecognitionResult) {
        let lines = reconstruct_lines(&request.pages);
        let context = Context::new(request);
        let profile = self.active();
        let mut count = 0;
        for rule in &profile.memory.fields {
            if !rule.enabled || !rule.scope.matches(&context) {
                continue;
            }
            let Some(page) = request.pages.iter().find(|p| {
                p.page == rule.page
                    && p.width.is_finite()
                    && p.height.is_finite()
                    && p.width > 0.0
                    && p.height > 0.0
            }) else {
                continue;
            };
            let candidate = lines
                .iter()
                .filter(|line| line.page == rule.page)
                .filter_map(|line| {
                    let start = if rule.prefix.is_empty() {
                        if (line.tokens[0].bounds.y / page.height - rule.region.y).abs() > 0.02 {
                            return None;
                        }
                        line.tokens
                            .iter()
                            .position(|t| t.bounds.x / page.width >= rule.region.x - 0.015)?
                    } else {
                        line.tokens.windows(rule.prefix.len()).position(|tokens| {
                            tokens
                                .iter()
                                .map(|t| normalize(&t.text))
                                .eq(rule.prefix.iter().cloned())
                        })? + rule.prefix.len()
                    };
                    let tokens = line.tokens[start..]
                        .iter()
                        .take_while(|t| {
                            (t.bounds.x + t.bounds.width / 2.0) / page.width < rule.right
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    if tokens.is_empty() {
                        return None;
                    }
                    let raw = tokens
                        .iter()
                        .map(|t| t.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let value = normalize_value(&rule.field, &raw)?;
                    Some((
                        (line.tokens[0].bounds.y / page.height - rule.region.y).abs(),
                        FieldProposal {
                            value,
                            confidence: if tokens.iter().any(|t| t.id.contains("-ocr")) {
                                0.75
                            } else {
                                0.85
                            },
                            source_token_ids: tokens.iter().map(|t| t.id.clone()).collect(),
                            source_text: raw,
                        },
                    ))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, proposal)) = candidate {
                result.fields.insert(rule.field.clone(), proposal);
                count += 1;
            }
        }
        let mut table_count = 0;
        for rule in &profile.memory.tables {
            if !rule.enabled || !rule.scope.matches(&context) {
                continue;
            }
            let parsed = read_table(request, rule, &lines);
            if !parsed.is_empty() {
                result.lines.retain(|line| {
                    !line.source_token_ids.iter().any(|id| {
                        request
                            .pages
                            .iter()
                            .find(|p| p.page == rule.page)
                            .is_some_and(|p| p.tokens.iter().any(|t| &t.id == id))
                    })
                });
                result.lines.extend(parsed);
                table_count += 1;
            }
        }
        if count + table_count > 0 {
            result.warnings.push(format!("Lernprofil {}: {count} Feldzuordnungen, {table_count} Tabellen angewendet. Gelernte Tabellen verwenden den bestätigten Profil-Steuerfall; bitte am Original prüfen.", profile.name));
        }
        if table_count > 0 {
            let positions: BTreeMap<_, _> = request
                .pages
                .iter()
                .flat_map(|page| {
                    page.tokens
                        .iter()
                        .map(move |token| (token.id.as_str(), (page.page, token.bounds.y)))
                })
                .collect();
            result.lines.sort_by(|a, b| {
                let key = |line: &LineProposal| {
                    line.source_token_ids
                        .first()
                        .and_then(|id| positions.get(id.as_str()))
                        .copied()
                        .unwrap_or((u32::MAX, f64::MAX))
                };
                let a = key(a);
                let b = key(b);
                a.0.cmp(&b.0).then(a.1.total_cmp(&b.1))
            });
            for (index, line) in result.lines.iter_mut().enumerate() {
                line.line.id = (index + 1).to_string();
            }
        }
    }
}

fn clean_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        Err("Profilname muss 1 bis 80 Zeichen lang sein.".into())
    } else {
        Ok(name.into())
    }
}
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn anchors(request: &RecognitionRequest) -> BTreeSet<String> {
    request
        .pages
        .iter()
        .filter(|page| page.width.is_finite() && page.width > 0.0)
        .flat_map(|page| {
            page.tokens
                .iter()
                .filter(|t| {
                    !t.text.chars().any(|c| c.is_numeric())
                        && t.text.chars().filter(|c| c.is_alphabetic()).count() >= 3
                })
                .map(|t| {
                    format!(
                        "{}:{}:{}",
                        page.page,
                        (t.bounds.x / page.width * 20.0).round() as i64,
                        normalize(&t.text)
                    )
                })
        })
        .take(1500)
        .collect()
}
struct Context {
    text: String,
    anchors: BTreeSet<String>,
}
impl Context {
    fn new(request: &RecognitionRequest) -> Self {
        Self {
            text: normalize(
                &request
                    .pages
                    .iter()
                    .flat_map(|p| &p.tokens)
                    .map(|t| t.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            anchors: anchors(request),
        }
    }
}
impl Scope {
    fn new(request: &RecognitionRequest, invoice: &InvoiceInput) -> Self {
        let text = Context::new(request).text;
        let identities = [
            invoice.seller.vat_id.as_deref(),
            invoice.payment.iban.as_deref(),
            Some(invoice.seller.name.as_str()),
        ]
        .into_iter()
        .flatten()
        .map(normalize)
        .filter(|key| key.len() >= 8 && text.contains(key))
        .collect();
        Self {
            identities,
            anchors: anchors(request),
        }
    }
    fn matches(&self, context: &Context) -> bool {
        if self.anchors.len() < 4 {
            return false;
        }
        let overlap =
            self.anchors.intersection(&context.anchors).count() as f64 / self.anchors.len() as f64;
        if self.identities.is_empty() {
            self.anchors.len() >= 12 && overlap >= 0.9
        } else {
            self.identities.iter().any(|id| context.text.contains(id)) && overlap >= 0.65
        }
    }
}
fn normalize_value(field: &str, text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.len() > 1000 {
        return None;
    }
    match field {
        "issueDate" | "dueDate" | "serviceDate" => {
            if text.len() > 64 || !text.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                None
            } else {
                date(text)
            }
        }
        "currency" => (text.len() == 3 && text.bytes().all(|c| c.is_ascii_alphabetic()))
            .then(|| text.to_uppercase()),
        "sellerCountryCode" | "buyerCountryCode" => (text.len() == 2
            && text.bytes().all(|c| c.is_ascii_alphabetic()))
        .then(|| text.to_uppercase()),
        "iban" | "bic" | "sellerVatId" | "buyerVatId" => Some(text.replace(' ', "").to_uppercase()),
        _ => Some(text.into()),
    }
}
fn locate<'a>(
    lines: &'a [TextLine],
    field: &str,
    value: &str,
) -> Option<(&'a TextLine, usize, usize)> {
    let needle = normalize(&normalize_value(field, value)?);
    if needle.is_empty() {
        return None;
    }
    let mut matches = vec![];
    for line in lines {
        for start in 0..line.tokens.len().min(200) {
            for end in start + 1..=(start + 16).min(line.tokens.len()) {
                let raw = line.tokens[start..end]
                    .iter()
                    .map(|t| t.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                if raw.len() > value.len() * 3 + 32 {
                    break;
                }
                if normalize_value(field, &raw).is_some_and(|v| normalize(&v) == needle) {
                    matches.push((line, start, end));
                    break;
                }
            }
        }
    }
    if let Some(length) = matches.iter().map(|(_, start, end)| end - start).min() {
        matches.retain(|(_, start, end)| end - start == length);
    }
    (matches.len() == 1).then(|| matches.remove(0))
}
fn numeric(text: &str) -> Option<String> {
    let raw = text
        .trim()
        .trim_end_matches(|c: char| c.is_alphabetic() || c == '.');
    decimal(raw)
}
fn learn_tables(
    request: &RecognitionRequest,
    invoice: &InvoiceInput,
    scope: &Scope,
    lines: &[TextLine],
) -> Vec<TableRule> {
    type LocatedRow = ([f64; 4], f64, Tax, String);
    let mut rows: BTreeMap<u32, Vec<LocatedRow>> = BTreeMap::new();
    for item in &invoice.lines {
        if !item.allowances.is_empty() {
            continue;
        }
        let Some((line, start, end)) = locate(lines, "text", &item.name) else {
            continue;
        };
        let Some(page) = request.pages.iter().find(|p| {
            p.page == line.page
                && p.width.is_finite()
                && p.height.is_finite()
                && p.width > 0.0
                && p.height > 0.0
        }) else {
            continue;
        };
        let Some(qty) = decimal(&item.quantity) else {
            continue;
        };
        let Some(price) = decimal(&item.net_unit_price) else {
            continue;
        };
        let Some(total) = Decimal::from_str(&qty).ok().and_then(|q| {
            Decimal::from_str(&price)
                .ok()
                .and_then(|p| q.checked_mul(p))
        }) else {
            continue;
        };
        let center =
            |t: &SourceToken| ((t.bounds.x + t.bounds.width / 2.0) / page.width).clamp(0.0, 1.0);
        let Some(q) = (end..line.tokens.len())
            .find(|i| numeric(&line.tokens[*i].text).as_deref() == Some(&qty))
        else {
            continue;
        };
        let Some(p) = (q + 1..line.tokens.len())
            .find(|i| numeric(&line.tokens[*i].text).as_deref() == Some(&price))
        else {
            continue;
        };
        let Some(t) = (p + 1..line.tokens.len()).find(|i| {
            numeric(&line.tokens[*i].text)
                .and_then(|s| Decimal::from_str(&s).ok())
                .is_some_and(|v| {
                    v.checked_sub(total)
                        .is_some_and(|delta| delta.abs() <= Decimal::new(2, 2))
                })
        }) else {
            continue;
        };
        let row_y = (line.tokens[start].bounds.y / page.height).clamp(0.0, 1.0);
        let cols = [
            center(&line.tokens[start]),
            center(&line.tokens[q]),
            center(&line.tokens[p]),
            center(&line.tokens[t]),
        ];
        if !row_y.is_finite()
            || !cols.iter().all(|v| v.is_finite())
            || !cols.windows(2).all(|v| v[0] < v[1])
        {
            continue;
        }
        rows.entry(page.page).or_default().push((
            cols,
            row_y,
            item.tax.clone(),
            item.unit_code.clone(),
        ));
    }
    rows.into_iter()
        .filter_map(|(page, rows)| {
            if rows.iter().any(|r| r.2 != rows[0].2) {
                return None;
            }
            let columns = std::array::from_fn(|i| {
                rows.iter().map(|r| r.0[i]).sum::<f64>() / rows.len() as f64
            });
            let start_y = rows.iter().map(|r| r.1).fold(1.0, f64::min);
            if !start_y.is_finite()
                || !(0.0..=1.0).contains(&start_y)
                || !columns
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                || !columns.windows(2).all(|v| v[0] < v[1])
            {
                return None;
            }
            Some(TableRule {
                id: uuid::Uuid::new_v4().to_string(),
                enabled: true,
                scope: scope.clone(),
                page,
                start_y,
                columns,
                tax: rows[0].2.clone(),
                unit: rows[0].3.clone(),
            })
        })
        .collect()
}
fn read_table(
    request: &RecognitionRequest,
    rule: &TableRule,
    lines: &[TextLine],
) -> Vec<LineProposal> {
    let Some(page) = request.pages.iter().find(|p| {
        p.page == rule.page
            && p.width.is_finite()
            && p.height.is_finite()
            && p.width > 0.0
            && p.height > 0.0
    }) else {
        return vec![];
    };
    let bounds = [
        (rule.columns[0] + rule.columns[1]) / 2.0,
        (rule.columns[1] + rule.columns[2]) / 2.0,
        (rule.columns[2] + rule.columns[3]) / 2.0,
    ];
    let mut found = vec![];
    for line in lines.iter().filter(|l| {
        l.page == rule.page && l.tokens[0].bounds.y / page.height >= rule.start_y - 0.02
    }) {
        let lower = line.text.to_lowercase();
        if [
            "netto",
            "brutto",
            "gesamt",
            "zahlbetrag",
            "umsatzsteuer",
            "zahlungsbedingungen",
        ]
        .iter()
        .any(|s| lower.starts_with(s))
        {
            if !found.is_empty() {
                break;
            }
            continue;
        }
        let mut cells = [vec![], vec![], vec![], vec![]];
        for token in &line.tokens {
            let x = (token.bounds.x + token.bounds.width / 2.0) / page.width;
            if x < rule.columns[0] - 0.06 {
                continue;
            }
            let index = if x < bounds[0] {
                0
            } else if x < bounds[1] {
                1
            } else if x < bounds[2] {
                2
            } else {
                3
            };
            cells[index].push(token);
        }
        let name = cells[0]
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let Some(qty) = cells[1].iter().find_map(|t| numeric(&t.text)) else {
            continue;
        };
        let Some(price) = cells[2].iter().find_map(|t| numeric(&t.text)) else {
            continue;
        };
        let Some(total) = cells[3].iter().find_map(|t| numeric(&t.text)) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let valid = Decimal::from_str(&qty)
            .ok()
            .and_then(|q| {
                Decimal::from_str(&price)
                    .ok()
                    .and_then(|p| q.checked_mul(p))
            })
            .zip(Decimal::from_str(&total).ok())
            .is_some_and(|(a, b)| {
                a.checked_sub(b)
                    .is_some_and(|delta| delta.abs() <= Decimal::new(2, 2))
            });
        if !valid {
            continue;
        }
        let unit = cells[1]
            .iter()
            .find_map(|t| match t.text.to_lowercase().as_str() {
                "std." | "std" | "h" => Some("HUR"),
                "m" => Some("MTR"),
                "kg" => Some("KGM"),
                "stk" | "stk." | "st" => Some("C62"),
                _ => None,
            })
            .unwrap_or(&rule.unit)
            .to_string();
        found.push(LineProposal {
            line: InvoiceLine {
                id: (found.len() + 1).to_string(),
                name,
                description: None,
                quantity: qty,
                unit_code: unit,
                net_unit_price: price,
                tax: rule.tax.clone(),
                allowances: vec![],
            },
            net_amount: total,
            confidence: 0.75,
            source_token_ids: line.tokens.iter().map(|t| t.id.clone()).collect(),
            source_text: line.text.clone(),
        });
    }
    found
}
