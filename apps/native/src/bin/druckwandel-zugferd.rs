use druckwandel_native::{
    invoice::InvoiceInput, output::save_zugferd, validation::ValidatorBundle,
};
use std::{fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("Aufruf: druckwandel-zugferd entwurf.json quelle.pdf ausgabe.pdf".into());
    }
    let invoice: InvoiceInput = serde_json::from_slice(&fs::read(&args[0])?)?;
    let source = fs::read(&args[1])?;
    let bundle = ValidatorBundle::discover()?;
    let output = save_zugferd(invoice, &source, &PathBuf::from(&args[2]), &bundle)?;
    println!(
        "Geprüfte ZUGFeRD-Datei gespeichert: {}",
        output.path().display()
    );
    Ok(())
}
