//! Keep abort-prone inputs in a subprocess: catch_unwind cannot catch stack overflow.
use std::process::Command;

#[test]
fn deeply_nested_pdf_is_rejected_without_aborting_the_process() {
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let catalog_offset = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /X ");
    pdf.extend(std::iter::repeat_n(b'[', 10_500));
    pdf.extend(std::iter::repeat_n(b']', 10_500));
    pdf.extend_from_slice(b" >>\nendobj\n");
    let pages_offset = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\n");
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!(
        "xref\n0 3\n0000000000 65535 f \n{catalog_offset:010} 00000 n \n{pages_offset:010} 00000 n \ntrailer\n<< /Size 3 /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
    ).as_bytes());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nested.pdf");
    std::fs::write(&path, pdf).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_druckwandel-recognize"))
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "must report a normal error, not abort: {:?}",
        output.status
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("Error:"));
}
