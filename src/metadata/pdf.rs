use std::path::Path;
use lopdf::{Document, Object};
use super::model::MetadataSection;

pub fn analyze_pdf_metadata(path: &Path) -> Vec<MetadataSection> {
    let mut sections = Vec::new();
    let mut pdf_sec = MetadataSection::new("PDF Document");

    match Document::load(path) {
        Ok(doc) => {
            pdf_sec.add("PDF Version", format!("PDF-{}", doc.version));
            pdf_sec.add("Total Pages", doc.get_pages().len().to_string());
            pdf_sec.add("Encrypted", if doc.is_encrypted() { "Yes (Protected)" } else { "No" });

            // Extract Info Dictionary
            let mut info_dict = None;
            if let Ok(info_obj) = doc.trailer.get(b"Info") {
                if let Ok(info_id) = info_obj.as_reference() {
                    if let Ok(obj) = doc.get_object(info_id) {
                        if let Ok(dict) = obj.as_dict() {
                            info_dict = Some(dict.clone());
                        }
                    }
                } else if let Ok(dict) = info_obj.as_dict() {
                    info_dict = Some(dict.clone());
                }
            }

            if let Some(dict) = info_dict {
                if let Ok(obj) = dict.get(b"Title") {
                    if let Some(title) = decode_pdf_object(obj) {
                        pdf_sec.add("Title", title);
                    }
                }
                if let Ok(obj) = dict.get(b"Author") {
                    if let Some(author) = decode_pdf_object(obj) {
                        pdf_sec.add("Author", author);
                    }
                }
                if let Ok(obj) = dict.get(b"Subject") {
                    if let Some(subject) = decode_pdf_object(obj) {
                        pdf_sec.add("Subject", subject);
                    }
                }
                if let Ok(obj) = dict.get(b"Keywords") {
                    if let Some(keywords) = decode_pdf_object(obj) {
                        pdf_sec.add("Keywords", keywords);
                    }
                }
                if let Ok(obj) = dict.get(b"Creator") {
                    if let Some(creator) = decode_pdf_object(obj) {
                        pdf_sec.add("Creator Application", creator);
                    }
                }
                if let Ok(obj) = dict.get(b"Producer") {
                    if let Some(producer) = decode_pdf_object(obj) {
                        pdf_sec.add("PDF Producer", producer);
                    }
                }
                if let Ok(obj) = dict.get(b"CreationDate") {
                    if let Some(created) = decode_pdf_object(obj) {
                        pdf_sec.add("Creation Date", format_pdf_date(&created));
                    }
                }
                if let Ok(obj) = dict.get(b"ModDate") {
                    if let Some(modified) = decode_pdf_object(obj) {
                        pdf_sec.add("Modification Date", format_pdf_date(&modified));
                    }
                }
                if let Ok(obj) = dict.get(b"Trapped") {
                    if let Some(trapped) = decode_pdf_object(obj) {
                        pdf_sec.add("Trapped", trapped);
                    }
                }
            }
        }
        Err(err) => {
            pdf_sec.add("Parse Warning", format!("Partial PDF read: {err}"));
        }
    }

    if !pdf_sec.entries.is_empty() {
        sections.push(pdf_sec);
    }

    sections
}

fn decode_pdf_object(obj: &Object) -> Option<String> {
    match obj {
        Object::String(bytes, _) => Some(decode_pdf_bytes(bytes)),
        Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
        _ => None,
    }
}

fn decode_pdf_bytes(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect();
        return String::from_utf16_lossy(&u16s);
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        return String::from_utf16_lossy(&u16s);
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        s.to_string()
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

fn format_pdf_date(date_str: &str) -> String {
    let clean = date_str.trim_start_matches("D:").replace('\'', "");
    if clean.len() >= 14 {
        let y = &clean[0..4];
        let m = &clean[4..6];
        let d = &clean[6..8];
        let h = &clean[8..10];
        let min = &clean[10..12];
        let s = &clean[12..14];
        let mut res = format!("{y}-{m}-{d} {h}:{min}:{s}");
        if clean.len() > 14 {
            let tz = &clean[14..];
            res.push(' ');
            res.push_str(tz);
        }
        res
    } else {
        date_str.to_string()
    }
}
