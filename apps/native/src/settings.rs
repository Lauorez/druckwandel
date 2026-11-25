//! Local preferences and reusable issuer data, independent of invoice drafts.
use crate::invoice::{Contact, ElectronicAddress, InvoiceInput, Party};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub appearance: i32,
    pub recognition_method: i32,
    pub company: CompanyProfile,
    pub ocr: crate::ocr::OcrSettings,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct CompanyProfile {
    pub name: String,
    pub street: String,
    pub postal: String,
    pub city: String,
    pub country: String,
    pub vat: String,
    pub tax_number: String,
    pub endpoint: String,
    pub contact_name: String,
    pub contact_phone: String,
    pub contact_email: String,
    pub iban: String,
}

pub fn electronic_address(value: String) -> Option<ElectronicAddress> {
    optional(&value).map(|value| ElectronicAddress {
        scheme_id: if value.contains('@') { "EM" } else { "0204" }.into(),
        value,
    })
}

fn optional(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

impl Settings {
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join("settings.json");
        let mut settings: Self = if !path.exists() {
            let appearance = crate::file_io::read_limited(&root.join("appearance.json"), 1024)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<i32>(&bytes).ok())
                .unwrap_or(0);
            Self {
                appearance,
                ..Self::default()
            }
        } else {
            let bytes = crate::file_io::read_limited(&path, crate::file_io::MAX_DRAFT_BYTES)
                .map_err(|e| format!("Einstellungen konnten nicht geladen werden: {e}"))?;
            serde_json::from_slice(&bytes)
                .map_err(|e| format!("Einstellungen sind beschädigt: {e}"))?
        };
        settings.appearance = settings.appearance.clamp(0, 2);
        settings.recognition_method = settings.recognition_method.clamp(0, 1);
        if !["deu+eng", "deu", "eng"].contains(&settings.ocr.languages.as_str()) {
            settings.ocr.languages = "deu+eng".into();
        }
        Ok(settings)
    }

    pub fn save(&self, root: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(root).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(root.join("settings.json"))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl CompanyProfile {
    pub fn party(&self) -> Party {
        Party {
            name: self.name.trim().into(),
            address: crate::invoice::Address {
                line1: self.street.trim().into(),
                postal_code: self.postal.trim().into(),
                city: self.city.trim().into(),
                country_code: self.country.trim().to_uppercase(),
                ..Default::default()
            },
            vat_id: optional(&self.vat),
            tax_registration_id: optional(&self.tax_number),
            electronic_address: electronic_address(self.endpoint.clone()),
            contact: {
                let contact = Contact {
                    name: optional(&self.contact_name),
                    phone: optional(&self.contact_phone),
                    email: optional(&self.contact_email),
                };
                (contact.name.is_some() || contact.phone.is_some() || contact.email.is_some())
                    .then_some(contact)
            },
        }
    }

    /// Never replace document evidence or a user's existing correction implicitly.
    pub fn fill_missing(&self, invoice: &mut InvoiceInput) {
        let defaults = self.party();
        let seller = &mut invoice.seller;
        for (value, default) in [
            (&mut seller.name, defaults.name),
            (&mut seller.address.line1, defaults.address.line1),
            (
                &mut seller.address.postal_code,
                defaults.address.postal_code,
            ),
            (&mut seller.address.city, defaults.address.city),
            (
                &mut seller.address.country_code,
                defaults.address.country_code,
            ),
        ] {
            if value.trim().is_empty() {
                *value = default;
            }
        }
        for (value, default) in [
            (&mut seller.vat_id, defaults.vat_id),
            (
                &mut seller.tax_registration_id,
                defaults.tax_registration_id,
            ),
            (&mut invoice.payment.iban, optional(&self.iban)),
        ] {
            if value.as_deref().is_none_or(|s| s.trim().is_empty()) {
                *value = default;
            }
        }
        if seller
            .electronic_address
            .as_ref()
            .is_none_or(|e| e.value.trim().is_empty())
        {
            seller.electronic_address = defaults.electronic_address;
        }
        if let Some(default) = defaults.contact {
            let contact = seller.contact.get_or_insert_with(Contact::default);
            for (value, default) in [
                (&mut contact.name, default.name),
                (&mut contact.phone, default.phone),
                (&mut contact.email, default.email),
            ] {
                if value.as_deref().is_none_or(|s| s.trim().is_empty()) {
                    *value = default;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn preferences_migrate_and_survive_replacement() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("appearance.json"), "2").unwrap();
        let mut settings = Settings::load(root.path()).unwrap();
        assert_eq!(settings.appearance, 2);
        settings.company.name = "Beispiel GmbH".into();
        settings.ocr.enabled = false;
        settings.ocr.languages = "eng".into();
        settings.save(root.path()).unwrap();
        settings.recognition_method = 1;
        settings.save(root.path()).unwrap();
        let loaded = Settings::load(root.path()).unwrap();
        assert_eq!(loaded.company.name, "Beispiel GmbH");
        assert_eq!(loaded.recognition_method, 1);
        assert!(!loaded.ocr.enabled);
        assert_eq!(loaded.ocr.languages, "eng");
        fs::write(root.path().join("settings.json"), b"invalid").unwrap();
        assert!(Settings::load(root.path()).is_err());
    }

    #[test]
    fn issuer_defaults_preserve_recognized_data_and_manual_corrections() {
        let profile = CompanyProfile {
            name: "Vorlage".into(),
            city: "Berlin".into(),
            contact_email: "kontakt@example.org".into(),
            iban: "DE123".into(),
            ..Default::default()
        };
        let mut invoice = InvoiceInput::blank();
        invoice.seller.name = "Belegfirma".into();
        invoice.seller.contact = Some(Contact {
            name: Some("Korrigiert".into()),
            ..Default::default()
        });
        profile.fill_missing(&mut invoice);
        assert_eq!(invoice.seller.name, "Belegfirma");
        assert_eq!(invoice.seller.address.city, "Berlin");
        assert_eq!(
            invoice.seller.contact.as_ref().unwrap().name.as_deref(),
            Some("Korrigiert")
        );
        assert_eq!(
            invoice.seller.contact.as_ref().unwrap().email.as_deref(),
            Some("kontakt@example.org")
        );
        assert_eq!(invoice.payment.iban.as_deref(), Some("DE123"));
    }
}
