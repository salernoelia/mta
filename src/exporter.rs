use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use crate::metadata::FileMetadataReport;

pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())
}

pub fn save_single_csv(report: &FileMetadataReport, default_dir: &str) -> Result<Option<String>, String> {
    let default_name = format!("{}_metadata.csv", sanitize_filename(&report.file_name));
    let mut dialog = rfd::FileDialog::new()
        .set_file_name(&default_name)
        .add_filter("CSV Spreadsheet", &["csv"]);

    if !default_dir.is_empty() {
        dialog = dialog.set_directory(Path::new(default_dir));
    }

    if let Some(dest_path) = dialog.save_file() {
        let csv_content = report.to_csv()?;
        fs::write(&dest_path, csv_content).map_err(|e| e.to_string())?;
        Ok(Some(dest_path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

pub fn save_single_json(report: &FileMetadataReport, default_dir: &str) -> Result<Option<String>, String> {
    let default_name = format!("{}_metadata.json", sanitize_filename(&report.file_name));
    let mut dialog = rfd::FileDialog::new()
        .set_file_name(&default_name)
        .add_filter("JSON Document", &["json"]);

    if !default_dir.is_empty() {
        dialog = dialog.set_directory(Path::new(default_dir));
    }

    if let Some(dest_path) = dialog.save_file() {
        let json_content = report.to_json()?;
        fs::write(&dest_path, json_content).map_err(|e| e.to_string())?;
        Ok(Some(dest_path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

pub fn generate_batch_csv(reports: &[FileMetadataReport]) -> Result<String, String> {
    if reports.is_empty() {
        return Ok(String::new());
    }

    // Collect all unique property headers across all reports
    let standard_headers = vec![
        "File Name".to_string(),
        "Path".to_string(),
        "Size".to_string(),
        "Size (Bytes)".to_string(),
        "Format".to_string(),
        "MIME Type".to_string(),
    ];

    let mut custom_headers = BTreeSet::new();
    for report in reports {
        for sec in &report.sections {
            for entry in &sec.entries {
                let full_key = format!("{}: {}", sec.name, entry.key);
                custom_headers.insert(full_key);
            }
        }
    }

    let mut all_headers = standard_headers.clone();
    all_headers.extend(custom_headers);

    let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
    wtr.write_record(&all_headers).map_err(|e| e.to_string())?;

    for report in reports {
        let mut row = Vec::with_capacity(all_headers.len());
        row.push(report.file_name.clone());
        row.push(report.path.display().to_string());
        row.push(report.file_size_formatted.clone());
        row.push(report.file_size_bytes.to_string());
        row.push(report.file_type_label.clone());
        row.push(report.mime_type.clone());

        for header in &all_headers[standard_headers.len()..] {
            let mut found_val = String::new();
            if let Some((sec_name, prop_name)) = header.split_once(": ") {
                for sec in &report.sections {
                    if sec.name == sec_name {
                        for entry in &sec.entries {
                            if entry.key == prop_name {
                                found_val = entry.value.clone();
                                break;
                            }
                        }
                    }
                }
            }
            row.push(found_val);
        }

        wtr.write_record(&row).map_err(|e| e.to_string())?;
    }

    let bytes = wtr.into_inner().map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

pub fn save_batch_csv(reports: &[FileMetadataReport], default_dir: &str) -> Result<Option<String>, String> {
    let default_name = format!("batch_metadata_{}.csv", chrono::Local::now().format("%Y%m%d_%H%M%S"));
    let mut dialog = rfd::FileDialog::new()
        .set_file_name(&default_name)
        .add_filter("CSV Spreadsheet", &["csv"]);

    if !default_dir.is_empty() {
        dialog = dialog.set_directory(Path::new(default_dir));
    }

    if let Some(dest_path) = dialog.save_file() {
        let csv_content = generate_batch_csv(reports)?;
        fs::write(&dest_path, csv_content).map_err(|e| e.to_string())?;
        Ok(Some(dest_path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::model::MetadataSection;

    #[test]
    fn test_batch_csv_generation() {
        let mut r1 = FileMetadataReport::new(Path::new("photo1.jpg"));
        r1.file_size_formatted = "2.1 MB".to_string();
        r1.file_type_label = "JPEG".to_string();
        let mut s1 = MetadataSection::new("Camera");
        s1.add("Model", "Alpha 7");
        r1.sections.push(s1);

        let mut r2 = FileMetadataReport::new(Path::new("photo2.png"));
        r2.file_size_formatted = "4.5 MB".to_string();
        r2.file_type_label = "PNG".to_string();
        let mut s2 = MetadataSection::new("Camera");
        s2.add("Model", "EOS R5");
        r2.sections.push(s2);

        let csv = generate_batch_csv(&[r1, r2]).expect("Batch CSV failed");
        assert!(csv.contains("photo1.jpg"));
        assert!(csv.contains("photo2.png"));
        assert!(csv.contains("Camera: Model"));
        assert!(csv.contains("Alpha 7"));
        assert!(csv.contains("EOS R5"));
    }
}
