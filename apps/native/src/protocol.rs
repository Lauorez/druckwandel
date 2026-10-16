/// Keep the installed printer's existing URI contract. Accept only one UUID;
/// query/fragment metadata never becomes a filesystem path or shell command.
pub fn print_job_id(uri: &str) -> Result<String, String> {
    const PREFIX: &str = "erechnung-review://print-job/";
    let prefix = uri
        .get(..PREFIX.len())
        .ok_or("Ungültiger Druckauftrag-Link")?;
    if !prefix.eq_ignore_ascii_case(PREFIX) {
        return Err("Ungültiger Druckauftrag-Link".into());
    }
    let id = uri[PREFIX.len()..]
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    if id.len() != 36 {
        return Err("Ungültige Druckauftrag-ID".into());
    }
    let id = uuid::Uuid::parse_str(id).map_err(|_| "Ungültige Druckauftrag-ID")?;
    Ok(id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_legacy_uri_and_rejects_paths() {
        let id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        assert_eq!(
            print_job_id(&format!("ERECHNUNG-REVIEW://print-job/{id}?source=printer")).unwrap(),
            id
        );
        for uri in [
            "erechnung-review://print-job/../../file.pdf",
            "https://print-job/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "erechnung-review://print-job/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee/extra",
            "erechnung-review://print-job/%2e%2e",
        ] {
            assert!(print_job_id(uri).is_err());
        }
    }
}
