use druckwandel_native::file_io::{MAX_DRAFT_BYTES, MAX_PDF_BYTES, read_limited};
use druckwandel_native::{
    invoice::InvoiceInput, output::save_zugferd, validation::ValidatorBundle,
};
use std::path::{Path, PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("Aufruf: druckwandel-zugferd entwurf.json quelle.pdf ausgabe.pdf".into());
    }
    druckwandel_native::output::ensure_distinct_paths(Path::new(&args[1]), Path::new(&args[2]))?;
    let invoice: InvoiceInput =
        serde_json::from_slice(&read_limited(Path::new(&args[0]), MAX_DRAFT_BYTES)?)?;
    let source = read_limited(Path::new(&args[1]), MAX_PDF_BYTES)?;
    let bundle = ValidatorBundle::discover()?;
    let output = save_zugferd(invoice, &source, &PathBuf::from(&args[2]), &bundle)?;
    println!(
        "Geprüfte ZUGFeRD-Datei gespeichert: {}",
        output.path().display()
    );
    Ok(())
}
