use std::fs::File;
use std::io::Read;
use std::path::Path;
use sha2::{Digest as Sha2Digest, Sha256};
use sha1::Sha1;
use super::model::MetadataSection;

pub struct FileBasicInfo {
    pub size_bytes: u64,
    pub size_formatted: String,
    pub mime_type: String,
    pub file_type_label: String,
}

pub fn analyze_general_metadata(
    path: &Path,
    calculate_hashes: bool,
) -> (FileBasicInfo, Vec<MetadataSection>) {
    let mut fs_section = MetadataSection::new("File System");
    let mut hash_section = MetadataSection::new("Hashes & Checksums");

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    fs_section.add("File Name", &file_name);
    fs_section.add("Full Path", path.display().to_string());

    if let Ok(canonical) = path.canonicalize() {
        if canonical != path {
            fs_section.add("Canonical Path", canonical.display().to_string());
        }
    }

    if let Some(parent) = path.parent() {
        fs_section.add("Directory", parent.display().to_string());
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        fs_section.add("Extension", ext.to_uppercase());
    }

    let mut size_bytes = 0u64;
    let mut size_formatted = "0 B".to_string();

    if let Ok(meta) = path.metadata() {
        size_bytes = meta.len();
        size_formatted = format_bytes(size_bytes);
        fs_section.add("File Size", format!("{size_formatted} ({size_bytes} bytes)"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            use std::os::unix::fs::PermissionsExt;
            let mode = meta.permissions().mode();
            let perms_octal = format!("{:04o}", mode & 0o7777);
            let perms_str = format_unix_permissions(mode);
            fs_section.add("Permissions", format!("{perms_str} ({perms_octal})"));
            fs_section.add("Inode", meta.ino().to_string());
            fs_section.add("Hard Links", meta.nlink().to_string());
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            let attrs = meta.file_attributes();
            let mut attr_list = Vec::new();
            if attrs & 0x1 != 0 { attr_list.push("ReadOnly"); }
            if attrs & 0x2 != 0 { attr_list.push("Hidden"); }
            if attrs & 0x4 != 0 { attr_list.push("System"); }
            if attrs & 0x20 != 0 { attr_list.push("Archive"); }
            if attrs & 0x800 != 0 { attr_list.push("Compressed"); }
            if attrs & 0x4000 != 0 { attr_list.push("Encrypted"); }
            if !attr_list.is_empty() {
                fs_section.add("Attributes", attr_list.join(", "));
            }
        }

        if let Ok(created) = meta.created() {
            let dt: chrono::DateTime<chrono::Local> = created.into();
            fs_section.add("Created", dt.format("%Y-%m-%d %H:%M:%S").to_string());
        }

        if let Ok(modified) = meta.modified() {
            let dt: chrono::DateTime<chrono::Local> = modified.into();
            fs_section.add("Modified", dt.format("%Y-%m-%d %H:%M:%S").to_string());
        }

        if let Ok(accessed) = meta.accessed() {
            let dt: chrono::DateTime<chrono::Local> = accessed.into();
            fs_section.add("Last Accessed", dt.format("%Y-%m-%d %H:%M:%S").to_string());
        }
    }

    // Detect MIME type and friendly label
    let (mime_type, file_type_label) = detect_mime_and_label(path);
    fs_section.add("MIME Type", &mime_type);
    fs_section.add("Detected Format", &file_type_label);

    // Calculate checksums & Shannon entropy
    if calculate_hashes && size_bytes > 0 && size_bytes <= 100 * 1024 * 1024 {
        if let Ok(mut file) = File::open(path) {
            let mut sha256_hasher = Sha256::new();
            let mut sha1_hasher = Sha1::new();
            let mut md5_context = md5::Context::new();

            let mut byte_counts = [0u64; 256];
            let mut buffer = [0u8; 64 * 1024];
            let mut total_read = 0u64;

            while let Ok(n) = file.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                total_read += n as u64;
                sha256_hasher.update(&buffer[..n]);
                sha1_hasher.update(&buffer[..n]);
                md5_context.consume(&buffer[..n]);

                for &byte in &buffer[..n] {
                    byte_counts[byte as usize] += 1;
                }
            }

            if total_read > 0 {
                let sha256_hex = format!("{:x}", sha256_hasher.finalize());
                let sha1_hex = format!("{:x}", sha1_hasher.finalize());
                let md5_hex = format!("{:x}", md5_context.compute());

                hash_section.add("SHA-256", sha256_hex);
                hash_section.add("SHA-1", sha1_hex);
                hash_section.add("MD5", md5_hex);

                let entropy = calculate_shannon_entropy(&byte_counts, total_read);
                let entropy_note = if entropy > 7.9 {
                    "Very high (likely encrypted or compressed)"
                } else if entropy > 7.0 {
                    "High (compressed media or binary)"
                } else if entropy > 4.0 {
                    "Moderate (standard text / code / structured data)"
                } else {
                    "Low (highly repetitive or sparse data)"
                };
                hash_section.add("Shannon Entropy", format!("{entropy:.4} bits/byte ({entropy_note})"));
            }
        }
    } else if size_bytes > 100 * 1024 * 1024 {
        hash_section.add("Hashes", "Skipped for files > 100 MB (can compute on demand)");
    }

    let mut sections = vec![fs_section];
    if !hash_section.entries.is_empty() {
        sections.push(hash_section);
    }

    (
        FileBasicInfo {
            size_bytes,
            size_formatted,
            mime_type,
            file_type_label,
        },
        sections,
    )
}

fn detect_mime_and_label(path: &Path) -> (String, String) {
    // 1. Try reading header bytes with infer
    if let Ok(Some(kind)) = infer::get_from_path(path) {
        let mime = kind.mime_type().to_string();
        let ext = kind.extension().to_uppercase();
        let label = format!("{ext} ({:?})", kind.matcher_type());
        return (mime, label);
    }

    // 2. Fallback to extension matching
    if let Some(ext) = path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()) {
        match ext.as_str() {
            "txt" => ("text/plain".into(), "Plain Text Document".into()),
            "md" | "markdown" => ("text/markdown".into(), "Markdown Document".into()),
            "rs" => ("text/x-rust".into(), "Rust Source Code".into()),
            "py" => ("text/x-python".into(), "Python Source Code".into()),
            "js" => ("application/javascript".into(), "JavaScript Source".into()),
            "ts" => ("application/typescript".into(), "TypeScript Source".into()),
            "json" => ("application/json".into(), "JSON Document".into()),
            "yaml" | "yml" => ("application/yaml".into(), "YAML Document".into()),
            "toml" => ("application/toml".into(), "TOML Document".into()),
            "xml" => ("application/xml".into(), "XML Document".into()),
            "html" | "htm" => ("text/html".into(), "HTML Webpage".into()),
            "css" => ("text/css".into(), "CSS Stylesheet".into()),
            "csv" => ("text/csv".into(), "CSV Spreadsheet Data".into()),
            "tsv" => ("text/tab-separated-values".into(), "TSV Spreadsheet Data".into()),
            "sql" => ("application/sql".into(), "SQL Database Script".into()),
            "sh" | "bash" | "zsh" => ("application/x-sh".into(), "Shell Script".into()),
            "c" | "h" => ("text/x-c".into(), "C Source Code".into()),
            "cpp" | "hpp" | "cc" | "cxx" => ("text/x-c++".into(), "C++ Source Code".into()),
            "go" => ("text/x-go".into(), "Go Source Code".into()),
            "java" => ("text/x-java".into(), "Java Source Code".into()),
            "kt" => ("text/x-kotlin".into(), "Kotlin Source Code".into()),
            "swift" => ("text/x-swift".into(), "Swift Source Code".into()),
            "pdf" => ("application/pdf".into(), "PDF Document".into()),
            "docx" => ("application/vnd.openxmlformats-officedocument.wordprocessingml.document".into(), "Word Document (DOCX)".into()),
            "xlsx" => ("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into(), "Excel Spreadsheet (XLSX)".into()),
            "pptx" => ("application/vnd.openxmlformats-officedocument.presentationml.presentation".into(), "PowerPoint Presentation (PPTX)".into()),
            "odt" => ("application/vnd.oasis.opendocument.text".into(), "OpenDocument Text (ODT)".into()),
            "ods" => ("application/vnd.oasis.opendocument.spreadsheet".into(), "OpenDocument Spreadsheet (ODS)".into()),
            "odp" => ("application/vnd.oasis.opendocument.presentation".into(), "OpenDocument Presentation (ODP)".into()),
            "epub" => ("application/epub+zip".into(), "EPUB E-Book".into()),
            "zip" => ("application/zip".into(), "ZIP Archive".into()),
            "tar" => ("application/x-tar".into(), "TAR Archive".into()),
            "gz" => ("application/gzip".into(), "GZIP Compressed File".into()),
            "7z" => ("application/x-7z-compressed".into(), "7-Zip Archive".into()),
            "mp3" => ("audio/mpeg".into(), "MP3 Audio".into()),
            "flac" => ("audio/flac".into(), "FLAC Lossless Audio".into()),
            "wav" => ("audio/wav".into(), "WAV Audio".into()),
            "m4a" => ("audio/mp4".into(), "M4A Audio".into()),
            "ogg" => ("audio/ogg".into(), "OGG Audio / Vorbis".into()),
            "opus" => ("audio/opus".into(), "Opus Audio".into()),
            "mp4" => ("video/mp4".into(), "MP4 Video".into()),
            "mov" => ("video/quicktime".into(), "QuickTime Video (MOV)".into()),
            "mkv" => ("video/x-matroska".into(), "Matroska Video (MKV)".into()),
            "webm" => ("video/webm".into(), "WebM Media".into()),
            "jpg" | "jpeg" => ("image/jpeg".into(), "JPEG Image".into()),
            "png" => ("image/png".into(), "PNG Image".into()),
            "gif" => ("image/gif".into(), "GIF Image / Animation".into()),
            "webp" => ("image/webp".into(), "WebP Image".into()),
            "svg" => ("image/svg+xml".into(), "Scalable Vector Graphics (SVG)".into()),
            "ico" => ("image/x-icon".into(), "Icon File (ICO)".into()),
            "ttf" => ("font/ttf".into(), "TrueType Font (TTF)".into()),
            "otf" => ("font/otf".into(), "OpenType Font (OTF)".into()),
            "woff" => ("font/woff".into(), "Web Open Font Format (WOFF)".into()),
            "woff2" => ("font/woff2".into(), "Web Open Font Format 2 (WOFF2)".into()),
            _ => ("application/octet-stream".into(), format!("{} File", ext.to_uppercase())),
        }
    } else {
        ("application/octet-stream".into(), "Binary Data".into())
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn calculate_shannon_entropy(byte_counts: &[u64; 256], total_bytes: u64) -> f64 {
    if total_bytes == 0 {
        return 0.0;
    }
    let total_f = total_bytes as f64;
    let mut entropy = 0.0;
    for &count in byte_counts {
        if count > 0 {
            let p = count as f64 / total_f;
            entropy -= p * p.log2();
        }
    }
    entropy
}

#[cfg(unix)]
fn format_unix_permissions(mode: u32) -> String {
    let mut s = String::with_capacity(9);
    s.push(if mode & 0o400 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o200 != 0 { 'w' } else { '-' });
    s.push(if mode & 0o100 != 0 { 'x' } else { '-' });
    s.push(if mode & 0o040 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o020 != 0 { 'w' } else { '-' });
    s.push(if mode & 0o010 != 0 { 'x' } else { '-' });
    s.push(if mode & 0o004 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o002 != 0 { 'w' } else { '-' });
    s.push(if mode & 0o001 != 0 { 'x' } else { '-' });
    s
}
