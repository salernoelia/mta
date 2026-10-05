use std::fs::File;
use std::io::Read;
use std::path::Path;
use super::model::MetadataSection;

pub fn analyze_font_metadata(path: &Path) -> Vec<MetadataSection> {
    let mut sections = Vec::new();
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return sections,
    };

    let mut data = Vec::new();
    if file.read_to_end(&mut data).is_err() || data.len() < 12 {
        return sections;
    }

    let mut font_sec = MetadataSection::new("Font Metadata");

    let sfnt_version = &data[0..4];
    let type_label = match sfnt_version {
        b"OTTO" => "OpenType Font (CFF / PostScript outlines)",
        [0x00, 0x01, 0x00, 0x00] | b"true" | b"typ1" => "TrueType Font (TrueType outlines)",
        b"wOFF" => "WOFF (Web Open Font Format 1)",
        b"wOF2" => "WOFF2 (Web Open Font Format 2)",
        _ => return sections,
    };
    font_sec.add("Font Format", type_label);

    if sfnt_version == b"wOFF" || sfnt_version == b"wOF2" {
        sections.push(font_sec);
        return sections;
    }

    let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    font_sec.add("SFNT Tables Count", num_tables.to_string());

    // Search for 'name' table
    let mut name_offset = 0usize;
    let mut name_length = 0usize;

    for i in 0..num_tables {
        let entry_offset = 12 + i * 16;
        if entry_offset + 16 > data.len() {
            break;
        }
        let tag = &data[entry_offset..entry_offset + 4];
        if tag == b"name" {
            name_offset = u32::from_be_bytes([
                data[entry_offset + 8],
                data[entry_offset + 9],
                data[entry_offset + 10],
                data[entry_offset + 11],
            ]) as usize;
            name_length = u32::from_be_bytes([
                data[entry_offset + 12],
                data[entry_offset + 13],
                data[entry_offset + 14],
                data[entry_offset + 15],
            ]) as usize;
            break;
        }
    }

    if name_offset > 0 && name_offset + name_length <= data.len() && name_length >= 6 {
        let name_data = &data[name_offset..name_offset + name_length];
        let count = u16::from_be_bytes([name_data[2], name_data[3]]) as usize;
        let string_storage_offset = u16::from_be_bytes([name_data[4], name_data[5]]) as usize;

        for i in 0..count {
            let rec_offset = 6 + i * 12;
            if rec_offset + 12 > name_data.len() {
                break;
            }
            let platform_id = u16::from_be_bytes([name_data[rec_offset], name_data[rec_offset + 1]]);
            let name_id = u16::from_be_bytes([name_data[rec_offset + 6], name_data[rec_offset + 7]]);
            let len = u16::from_be_bytes([name_data[rec_offset + 8], name_data[rec_offset + 9]]) as usize;
            let str_off = u16::from_be_bytes([name_data[rec_offset + 10], name_data[rec_offset + 11]]) as usize;

            let abs_str_offset = string_storage_offset + str_off;
            if abs_str_offset + len > name_data.len() {
                continue;
            }

            let raw_str_bytes = &name_data[abs_str_offset..abs_str_offset + len];
            let val = if platform_id == 3 || platform_id == 0 {
                // UTF-16 BE
                let u16s: Vec<u16> = raw_str_bytes
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16_lossy(&u16s)
            } else {
                String::from_utf8_lossy(raw_str_bytes).to_string()
            };

            let clean_val = val.trim().to_string();
            if clean_val.is_empty() {
                continue;
            }

            let field_name = match name_id {
                0 => "Copyright",
                1 => "Font Family",
                2 => "Font Subfamily (Style)",
                3 => "Unique Identifier",
                4 => "Full Font Name",
                5 => "Version",
                6 => "PostScript Name",
                7 => "Trademark",
                8 => "Manufacturer",
                9 => "Designer",
                11 => "Vendor URL",
                13 => "License",
                _ => continue,
            };

            if !font_sec.entries.iter().any(|e| e.key == field_name) {
                font_sec.add(field_name, clean_val);
            }
        }
    }

    if !font_sec.entries.is_empty() {
        sections.push(font_sec);
    }

    sections
}
