use druckwandel_native::recognition::{RecognitionMethod, RecognitionRegistry, RecognitionRequest};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = PathBuf::from(
        args.next()
            .ok_or("Aufruf: druckwandel-recognize quelle.pdf")?,
    );
    if args.next().is_some() {
        return Err("Zu viele Argumente.".into());
    }
    let request = RecognitionRequest::from_file(path)?;
    let result = RecognitionRegistry::default().recognize(RecognitionMethod::Schema, &request)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
