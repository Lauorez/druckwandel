//! Compatibility for compressed inline images not supported by lopdf's content
//! parser. Images have no text tokens, so omit them only from the extraction copy.
use flate2::{Decompress, FlushDecompress, Status};
use lopdf::{Document, Stream, dictionary};

fn whitespace(byte: u8) -> bool {
    matches!(byte, 0 | 9 | 10 | 12 | 13 | 32)
}
fn delimiter(byte: u8) -> bool {
    whitespace(byte) || b"()<>[]{}/%".contains(&byte)
}

// Tokenize outside PDF literal strings, hex strings and comments. In particular,
// the letters BI in invoice text must never be interpreted as an image operator.
fn token(data: &[u8], position: &mut usize) -> Option<std::ops::Range<usize>> {
    while *position < data.len() {
        if whitespace(data[*position]) {
            *position += 1;
        } else if data[*position] == b'%' {
            while *position < data.len() && !matches!(data[*position], b'\r' | b'\n') {
                *position += 1;
            }
        } else {
            break;
        }
    }
    if *position == data.len() {
        return None;
    }
    let start = *position;
    let byte = data[*position];
    *position += 1;
    if byte == b'(' {
        let mut depth = 1;
        while *position < data.len() && depth > 0 {
            match data[*position] {
                b'\\' => {
                    *position = (*position + 2).min(data.len());
                    continue;
                }
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            *position += 1;
        }
    } else if byte == b'<' && data.get(*position) != Some(&b'<') {
        while *position < data.len() && data[*position] != b'>' {
            *position += 1;
        }
        *position = (*position + 1).min(data.len());
    } else if matches!(byte, b'<' | b'>') && data.get(*position) == Some(&byte) {
        *position += 1;
    } else if byte == b'/' || !delimiter(byte) {
        while *position < data.len() && !delimiter(data[*position]) {
            *position += 1;
        }
    }
    Some(start..*position)
}

fn omit_flate_images(data: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let mut position = 0;
    let mut copied = 0;
    let mut output = Vec::new();
    while let Some(range) = token(data, &mut position) {
        if &data[range.clone()] != b"BI" {
            continue;
        }
        let start = range.start;
        let mut header = Vec::new();
        loop {
            let range =
                token(data, &mut position).ok_or("Unvollständiger Inline-Bildkopf in der PDF.")?;
            if &data[range.clone()] == b"ID" {
                break;
            }
            header.push(range);
        }
        let flate = header.windows(2).any(|pair| {
            matches!(&data[pair[0].clone()], b"/F" | b"/Filter")
                && matches!(&data[pair[1].clone()], b"/Fl" | b"/FlateDecode")
        });
        // Other image encodings stay with the original parser. Do not tokenize
        // binary image bytes, where apparent operators have no meaning.
        if !flate {
            break;
        }
        if !data.get(position).is_some_and(|byte| whitespace(*byte)) {
            return Err("Inline-Bilddaten sind nicht korrekt abgegrenzt.".into());
        }
        position += if data.get(position..position + 2) == Some(b"\r\n") {
            2
        } else {
            1
        };
        let compressed_start = position;
        let mut decoder = Decompress::new(true);
        let mut scratch = [0_u8; 8192];
        loop {
            let before = (decoder.total_in(), decoder.total_out());
            let status = decoder
                .decompress(
                    &data[compressed_start + decoder.total_in() as usize..],
                    &mut scratch,
                    FlushDecompress::None,
                )
                .map_err(|_| "Das komprimierte Inline-Bild ist beschädigt.")?;
            if decoder.total_out() > 100 * 1024 * 1024 {
                return Err("Inline-Bild überschreitet die Lesegrenze von 100 MB.".into());
            }
            if status == Status::StreamEnd {
                break;
            }
            if before == (decoder.total_in(), decoder.total_out()) {
                return Err("Komprimiertes Inline-Bild ist unvollständig.".into());
            }
        }
        position = compressed_start + decoder.total_in() as usize;
        let end = token(data, &mut position).ok_or("Inline-Bildende fehlt.")?;
        if &data[end] != b"EI" {
            return Err("Inline-Bildende ist nicht korrekt abgegrenzt.".into());
        }
        output.extend_from_slice(&data[copied..start]);
        output.push(b'\n');
        copied = position;
    }
    if copied == 0 {
        return Ok(None);
    }
    output.extend_from_slice(&data[copied..]);
    Ok(Some(output))
}

pub(crate) fn prepare_text_content(doc: &mut Document) -> Result<(), String> {
    for (_, page) in doc.get_pages() {
        let content = doc.get_page_content(page).map_err(|e| e.to_string())?;
        if let Some(content) = omit_flate_images(&content)? {
            let id = doc.add_object(Stream::new(dictionary! {}, content));
            doc.get_object_mut(page)
                .and_then(lopdf::Object::as_dict_mut)
                .map_err(|e| e.to_string())?
                .set("Contents", id);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn skips_only_real_flate_images_and_preserves_text_operators() {
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&[0, 25, 255]).unwrap();
        let mut content = b"BT (BI is invoice text) Tj ET\n% BI in a comment\nBI /W 1 /H 1 /BPC 8 /CS /RGB /F /Fl ID ".to_vec();
        content.extend(encoder.finish().unwrap());
        content.extend(b"\nEI\nBT (after image) Tj ET");
        let result = omit_flate_images(&content).unwrap().unwrap();
        assert_eq!(
            result,
            b"BT (BI is invoice text) Tj ET\n% BI in a comment\n\n\nBT (after image) Tj ET"
        );
        assert!(lopdf::content::Content::decode(&result).is_ok());
    }
    #[test]
    fn rejects_truncated_image_instead_of_guessing_its_end() {
        assert!(omit_flate_images(b"BI /F /Fl ID invalid EI BT (text) Tj ET").is_err());
    }
}
