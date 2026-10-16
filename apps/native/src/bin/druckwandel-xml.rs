use druckwandel_native::invoice::InvoiceInput;
use druckwandel_native::output::save_xrechnung;
use druckwandel_native::validation::ValidatorBundle;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let source = args
        .next()
        .map(PathBuf::from)
        .ok_or("Aufruf: druckwandel-xml <rechnung.json> <ausgabe.xml>")?;
    let destination = args
        .next()
        .map(PathBuf::from)
        .ok_or("Aufruf: druckwandel-xml <rechnung.json> <ausgabe.xml>")?;
    if args.next().is_some() {
        return Err("Zu viele Argumente.".into());
    }
    let data = std::fs::read(&source)?;
    if data.len() > 1024 * 1024 {
        return Err("Der Rechnungsentwurf überschreitet 1 MB.".into());
    }
    let invoice: InvoiceInput = serde_json::from_slice(&data)?;
    let bundle = ValidatorBundle::discover()?;
    let saved = save_xrechnung(invoice, &destination, &bundle)?;
    println!("Geprüfte XRechnung gespeichert: {}", saved.path().display());
    Ok(())
}
