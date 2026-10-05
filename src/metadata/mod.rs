pub mod archive;
pub mod audio;
pub mod binary;
pub mod font;
pub mod general;
pub mod image;
pub mod model;
pub mod office;
pub mod pdf;
pub mod text;
pub mod video;

pub use model::FileMetadataReport;

use std::fs::File;
use std::io::Read;
use std::path::Path;

pub fn inspect_file(path: &Path, calculate_hashes: bool) -> Result<FileMetadataReport, String> {
    if !path.exists() {
        return Err(format!("File does not exist: {}", path.display()));
    }
    if path.is_dir() {
        return Err(format!("Path is a directory, not a file: {}", path.display()));
    }

    let mut report = FileMetadataReport::new(path);

    // 1. General filesystem & hash metadata
    let (basic_info, general_sections) = general::analyze_general_metadata(path, calculate_hashes);
    report.file_size_bytes = basic_info.size_bytes;
    report.file_size_formatted = basic_info.size_formatted;
    report.mime_type = basic_info.mime_type.clone();
    report.file_type_label = basic_info.file_type_label.clone();

    // 2. Read first few bytes to sniff signatures
    let mut header_magic = [0u8; 16];
    if let Ok(mut f) = File::open(path) {
        let _ = f.read(&mut header_magic);
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mut specialized_sections = Vec::new();

    // 3. Format-specific analyzers
    let is_pdf = ext == "pdf" || header_magic.starts_with(b"%PDF-");
    let is_image = matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "tiff" | "tif" | "bmp" | "ico" | "heic" | "avif"
    ) || basic_info.mime_type.starts_with("image/");

    let is_office = matches!(
        ext.as_str(),
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" | "epub"
    );

    let is_audio = matches!(
        ext.as_str(),
        "mp3" | "flac" | "ogg" | "wav" | "m4a" | "aac" | "opus" | "aiff" | "aif" | "wma"
    ) || basic_info.mime_type.starts_with("audio/");

    let is_video = matches!(
        ext.as_str(),
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v"
    ) || basic_info.mime_type.starts_with("video/");

    let is_archive = matches!(
        ext.as_str(),
        "zip" | "tar" | "gz" | "tgz" | "7z" | "jar" | "apk"
    ) || (header_magic.starts_with(b"PK\x03\x04") && !is_office);

    let is_binary = matches!(
        ext.as_str(),
        "exe" | "dll" | "so" | "dylib" | "bin" | "o" | "a"
    ) || header_magic.starts_with(b"\x7fELF")
        || header_magic.starts_with(b"MZ")
        || header_magic.starts_with(&[0xFE, 0xED, 0xFA, 0xCE])
        || header_magic.starts_with(&[0xFE, 0xED, 0xFA, 0xCF])
        || header_magic.starts_with(&[0xCE, 0xFA, 0xED, 0xFE])
        || header_magic.starts_with(&[0xCF, 0xFA, 0xED, 0xFE])
        || header_magic.starts_with(&[0xCA, 0xFE, 0xBA, 0xBE])
        || basic_info.mime_type.contains("mach-binary")
        || basic_info.mime_type.contains("executable");

    let is_font = matches!(ext.as_str(), "ttf" | "otf" | "woff" | "woff2")
        || header_magic.starts_with(b"OTTO")
        || header_magic.starts_with(&[0x00, 0x01, 0x00, 0x00])
        || header_magic.starts_with(b"wOFF")
        || header_magic.starts_with(b"wOF2");

    let is_text = matches!(
        ext.as_str(),
        "txt" | "md" | "markdown" | "rs" | "py" | "js" | "ts" | "json" | "yaml" | "yml"
            | "toml" | "xml" | "html" | "css" | "csv" | "tsv" | "sql" | "sh" | "c" | "cpp"
            | "h" | "hpp" | "go" | "java" | "swift" | "kt"
    ) || basic_info.mime_type.starts_with("text/")
        || basic_info.mime_type.contains("json")
        || basic_info.mime_type.contains("xml")
        || basic_info.mime_type.contains("yaml")
        || basic_info.mime_type.contains("javascript");

    if is_image {
        let secs = image::analyze_image_metadata(path);
        specialized_sections.extend(secs);
    }

    if is_pdf {
        let secs = pdf::analyze_pdf_metadata(path);
        specialized_sections.extend(secs);
    }

    if is_office {
        let secs = office::analyze_office_metadata(path);
        specialized_sections.extend(secs);
    }

    if is_audio {
        let secs = audio::analyze_audio_metadata(path);
        specialized_sections.extend(secs);
    }

    if is_video {
        let secs = video::analyze_video_metadata(path);
        specialized_sections.extend(secs);
    }

    if is_archive {
        let secs = archive::analyze_archive_metadata(path);
        specialized_sections.extend(secs);
    }

    if is_binary {
        let secs = binary::analyze_binary_metadata(path);
        specialized_sections.extend(secs);
    }

    if is_font {
        let secs = font::analyze_font_metadata(path);
        specialized_sections.extend(secs);
    }

    if is_text {
        let secs = text::analyze_text_metadata(path);
        specialized_sections.extend(secs);
    }

    // Assemble report: General first, then specialized, then hashes
    let mut final_sections = Vec::new();
    if let Some(fs) = general_sections.iter().find(|s| s.name == "File System") {
        final_sections.push(fs.clone());
    }
    final_sections.extend(specialized_sections);
    if let Some(hs) = general_sections.iter().find(|s| s.name == "Hashes & Checksums") {
        final_sections.push(hs.clone());
    }

    report.sections = final_sections;
    Ok(report)
}
