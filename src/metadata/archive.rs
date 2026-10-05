use std::fs::File;
use std::path::Path;
use super::general::format_bytes;
use super::model::MetadataSection;

pub fn analyze_archive_metadata(path: &Path) -> Vec<MetadataSection> {
    let mut sections = Vec::new();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mut arc_sec = MetadataSection::new("Archive Properties");
    let mut files_sec = MetadataSection::new("Archive Contents Preview");

    if ext == "zip" || ext == "jar" || ext == "apk" || ext == "war" {
        if let Ok(file) = File::open(path) {
            if let Ok(mut zip) = zip::ZipArchive::new(file) {
                let total_entries = zip.len();
                arc_sec.add("Archive Type", "ZIP Compressed Archive");
                arc_sec.add("Total Entries", total_entries.to_string());

                let mut uncompressed_total = 0u64;
                let mut compressed_total = 0u64;
                let mut is_encrypted = false;
                let mut file_count = 0usize;
                let mut dir_count = 0usize;

                let preview_limit = 10usize;
                let mut preview_list = Vec::new();

                for i in 0..total_entries {
                    if let Ok(entry) = zip.by_index(i) {
                        uncompressed_total += entry.size();
                        compressed_total += entry.compressed_size();
                        if entry.is_dir() {
                            dir_count += 1;
                        } else {
                            file_count += 1;
                        }
                        if entry.encrypted() {
                            is_encrypted = true;
                        }

                        if preview_list.len() < preview_limit {
                            let size_str = if entry.is_dir() {
                                "dir".to_string()
                            } else {
                                format_bytes(entry.size())
                            };
                            preview_list.push(format!("{} ({size_str})", entry.name()));
                        }
                    }
                }

                arc_sec.add("Files Count", file_count.to_string());
                arc_sec.add("Directories Count", dir_count.to_string());
                arc_sec.add("Uncompressed Size", format!("{uncompressed_total} bytes ({})", format_bytes(uncompressed_total)));
                arc_sec.add("Compressed Size", format!("{compressed_total} bytes ({})", format_bytes(compressed_total)));

                if uncompressed_total > 0 {
                    let ratio = (1.0 - (compressed_total as f64 / uncompressed_total as f64)) * 100.0;
                    arc_sec.add("Compression Savings", format!("{ratio:.1}%"));
                }

                arc_sec.add("Password Encrypted", if is_encrypted { "Yes" } else { "No" });

                let comment = zip.comment();
                if !comment.is_empty() {
                    let c_str = String::from_utf8_lossy(comment).trim().to_string();
                    if !c_str.is_empty() {
                        arc_sec.add("Archive Comment", c_str);
                    }
                }

                for (idx, item) in preview_list.iter().enumerate() {
                    files_sec.add(format!("#{}", idx + 1), item);
                }
                if total_entries > preview_limit {
                    files_sec.add("Note", format!("... and {} more items", total_entries - preview_limit));
                }
            }
        }
    } else if ext == "tar" {
        if let Ok(file) = File::open(path) {
            let mut archive = tar::Archive::new(file);
            arc_sec.add("Archive Type", "POSIX TAR Archive (Uncompressed)");
            if let Ok(entries) = archive.entries() {
                let mut count = 0usize;
                let mut total_size = 0u64;
                for entry in entries.flatten() {
                    count += 1;
                    total_size += entry.size();
                }
                arc_sec.add("Total Entries", count.to_string());
                arc_sec.add("Total Content Size", format!("{total_size} bytes ({})", format_bytes(total_size)));
            }
        }
    } else if ext == "gz" || ext == "tgz" {
        if let Ok(file) = File::open(path) {
            let gz = flate2::read::GzDecoder::new(file);
            arc_sec.add("Archive Type", "GZIP Compressed Data");
            if let Some(header) = gz.header() {
                if let Some(filename) = header.filename() {
                    arc_sec.add("Original Filename", String::from_utf8_lossy(filename).to_string());
                }
                if let Some(mtime) = header.mtime_as_datetime() {
                    let dt: chrono::DateTime<chrono::Local> = mtime.into();
                    arc_sec.add("Embedded MTime", dt.format("%Y-%m-%d %H:%M:%S").to_string());
                }
            }
        }
    }

    if !arc_sec.entries.is_empty() {
        sections.push(arc_sec);
    }
    if !files_sec.entries.is_empty() {
        sections.push(files_sec);
    }

    sections
}
