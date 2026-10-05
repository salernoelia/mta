use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataEntry {
    pub key: String,
    pub value: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl MetadataEntry {
    pub fn new(category: impl Into<String>, key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            key: key.into(),
            value: value.into(),
            description: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataSection {
    pub name: String,
    pub entries: Vec<MetadataEntry>,
}

impl MetadataSection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entries: Vec::new(),
        }
    }

    pub fn add(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key_str = key.into();
        let val_str = value.into();
        if !val_str.trim().is_empty() {
            self.entries.push(MetadataEntry::new(&self.name, key_str, val_str));
        }
    }

    pub fn add_opt(&mut self, key: impl Into<String>, value: Option<impl ToString>) {
        if let Some(v) = value {
            let s = v.to_string();
            if !s.trim().is_empty() {
                self.add(key, s);
            }
        }
    }

    #[allow(dead_code)]
    pub fn add_with_desc(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
        desc: impl Into<String>,
    ) {
        let key_str = key.into();
        let val_str = value.into();
        if !val_str.trim().is_empty() {
            self.entries.push(
                MetadataEntry::new(&self.name, key_str, val_str).with_description(desc),
            );
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadataReport {
    pub path: PathBuf,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub file_size_formatted: String,
    pub mime_type: String,
    pub file_type_label: String,
    pub sections: Vec<MetadataSection>,
    pub analyzed_at: String,
}

impl FileMetadataReport {
    pub fn new(path: &Path) -> Self {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        Self {
            path: path.to_path_buf(),
            file_name,
            file_size_bytes: 0,
            file_size_formatted: String::new(),
            mime_type: "application/octet-stream".to_string(),
            file_type_label: "Unknown".to_string(),
            sections: Vec::new(),
            analyzed_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }

    pub fn total_entries_count(&self) -> usize {
        self.sections.iter().map(|s| s.entries.len()).sum()
    }

    pub fn to_csv(&self) -> Result<String, String> {
        let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
        wtr.write_record(["Category", "Property", "Value", "Description"])
            .map_err(|e| e.to_string())?;

        for sec in &self.sections {
            for entry in &sec.entries {
                wtr.write_record([
                    &entry.category,
                    &entry.key,
                    &entry.value,
                    entry.description.as_deref().unwrap_or(""),
                ])
                .map_err(|e| e.to_string())?;
            }
        }

        let bytes = wtr.into_inner().map_err(|e| e.to_string())?;
        String::from_utf8(bytes).map_err(|e| e.to_string())
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# Metadata Report: {}\n\n", self.file_name));
        md.push_str(&format!("- **Path**: `{}`\n", self.path.display()));
        md.push_str(&format!("- **Size**: {}\n", self.file_size_formatted));
        md.push_str(&format!("- **Type**: {} ({})\n", self.file_type_label, self.mime_type));
        md.push_str(&format!("- **Analyzed**: {}\n\n", self.analyzed_at));

        for section in &self.sections {
            if section.entries.is_empty() {
                continue;
            }
            md.push_str(&format!("## {}\n\n", section.name));
            md.push_str("| Property | Value |\n|---|---|\n");
            for entry in &section.entries {
                let escaped_val = entry.value.replace('|', "\\|").replace('\n', " ");
                md.push_str(&format!("| {} | {} |\n", entry.key, escaped_val));
            }
            md.push('\n');
        }

        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_serialization() {
        let mut report = FileMetadataReport::new(Path::new("/tmp/test_file.png"));
        report.file_size_formatted = "1.5 MB".to_string();
        report.file_type_label = "PNG Image".to_string();

        let mut sec = MetadataSection::new("Image Properties");
        sec.add("Dimensions", "1920 × 1080");
        sec.add("Color Space", "sRGB");
        report.sections.push(sec);

        let csv = report.to_csv().expect("CSV generation failed");
        assert!(csv.contains("Category,Property,Value,Description"));
        assert!(csv.contains("Image Properties,Dimensions,1920 × 1080,"));
        assert!(csv.contains("Image Properties,Color Space,sRGB,"));

        let json = report.to_json().expect("JSON generation failed");
        assert!(json.contains("test_file.png"));

        let md = report.to_markdown();
        assert!(md.contains("# Metadata Report: test_file.png"));
        assert!(md.contains("| Dimensions | 1920 × 1080 |"));
    }
}
