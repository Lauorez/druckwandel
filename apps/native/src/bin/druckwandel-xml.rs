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
    let data = druckwandel_native::file_io::read_limited(
        &source,
        druckwandel_native::file_io::MAX_DRAFT_BYTES,
    )?;
    let invoice: InvoiceInput = serde_json::from_slice(&data)?;
    let bundle = ValidatorBundle::discover()?;
    let saved = save_xrechnung(invoice, &destination, &bundle)?;
    println!("Geprüfte XRechnung gespeichert: {}", saved.path().display());
    Ok(())
}
