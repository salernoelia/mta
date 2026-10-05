use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub path: String,
    pub file_name: String,
    pub file_size_formatted: String,
    pub file_type_label: String,
    pub timestamp: String,
    pub properties_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InspectionHistory {
    pub records: Vec<HistoryRecord>,
}

impl InspectionHistory {
    pub fn history_file_path() -> PathBuf {
        crate::config::config_dir().join("history.json")
    }

    pub fn load() -> Self {
        let path = Self::history_file_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(history) = serde_json::from_str::<Self>(&content) {
                return history;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::history_file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn add(&mut self, report: &crate::metadata::FileMetadataReport, max_records: usize) {
        let path_str = report.path.to_string_lossy().to_string();
        // Remove existing entry for the same path so it bubbles to the top
        self.records.retain(|r| r.path != path_str);

        let rec = HistoryRecord {
            path: path_str,
            file_name: report.file_name.clone(),
            file_size_formatted: report.file_size_formatted.clone(),
            file_type_label: report.file_type_label.clone(),
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            properties_count: report.total_entries_count(),
        };

        self.records.insert(0, rec);
        if self.records.len() > max_records {
            self.records.truncate(max_records);
        }

        let _ = self.save();
    }

    pub fn clear(&mut self) {
        self.records.clear();
        let _ = self.save();
    }

    pub fn remove(&mut self, path: &str) {
        self.records.retain(|r| r.path != path);
        let _ = self.save();
    }
}
