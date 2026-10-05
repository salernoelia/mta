use std::fs::File;
use std::io::Read;
use std::path::Path;
use super::model::MetadataSection;

pub fn analyze_text_metadata(path: &Path) -> Vec<MetadataSection> {
    let mut sections = Vec::new();
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return sections,
    };

    let metadata = match file.metadata() {
        Ok(m) => m,
        Err(_) => return sections,
    };

    let len = metadata.len();
    if len > 50 * 1024 * 1024 {
        // Skip huge files > 50MB for full text analysis
        return sections;
    }

    let mut raw_bytes = Vec::new();
    if file.read_to_end(&mut raw_bytes).is_err() {
        return sections;
    }

    let mut text_sec = MetadataSection::new("Text & Code Metrics");

    // Detect BOM
    let (encoding, text_slice) = if raw_bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        ("UTF-8 with BOM", &raw_bytes[3..])
    } else if raw_bytes.starts_with(&[0xFF, 0xFE]) {
        ("UTF-16 LE with BOM", &raw_bytes[2..])
    } else if raw_bytes.starts_with(&[0xFE, 0xFF]) {
        ("UTF-16 BE with BOM", &raw_bytes[2..])
    } else if raw_bytes.is_ascii() {
        ("ASCII", &raw_bytes[..])
    } else if std::str::from_utf8(&raw_bytes).is_ok() {
        ("UTF-8 (No BOM)", &raw_bytes[..])
    } else {
        ("Unknown / Binary", &raw_bytes[..])
    };

    text_sec.add("Character Encoding", encoding);

    // If it's valid UTF-8, analyze lines, words, chars
    if let Ok(text) = std::str::from_utf8(text_slice) {
        let mut total_lines = 0usize;
        let mut blank_lines = 0usize;
        let mut max_line_len = 0usize;
        let mut crlf_count = 0usize;
        let mut lf_count = 0usize;

        for line in text.split('\n') {
            total_lines += 1;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                blank_lines += 1;
            }
            if line.ends_with('\r') {
                crlf_count += 1;
                max_line_len = max_line_len.max(line.len().saturating_sub(1));
            } else {
                lf_count += 1;
                max_line_len = max_line_len.max(line.len());
            }
        }

        let non_blank_lines = total_lines.saturating_sub(blank_lines);
        let word_count = text.split_whitespace().count();
        let char_count = text.chars().count();

        text_sec.add("Total Lines", total_lines.to_string());
        text_sec.add("Non-Empty Lines", non_blank_lines.to_string());
        text_sec.add("Blank Lines", blank_lines.to_string());
        text_sec.add("Word Count", word_count.to_string());
        text_sec.add("Character Count", char_count.to_string());
        text_sec.add("Longest Line", format!("{max_line_len} chars"));

        let line_ending = match (crlf_count > 0, lf_count > 0) {
            (true, true) => "Mixed (CRLF & LF)",
            (true, false) => "Windows (CRLF)",
            (false, true) => "Unix (LF)",
            (false, false) => "Single Line",
        };
        text_sec.add("Line Endings", line_ending);

        // Format-specific deep analysis
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext == "json" {
            let mut json_sec = MetadataSection::new("JSON Structure");
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                json_sec.add("Valid JSON", "Yes");
                match parsed {
                    serde_json::Value::Object(map) => {
                        json_sec.add("Root Type", "Object / Map");
                        json_sec.add("Root Keys Count", map.len().to_string());
                        let keys: Vec<String> = map.keys().take(8).cloned().collect();
                        json_sec.add("Sample Keys", keys.join(", "));
                    }
                    serde_json::Value::Array(arr) => {
                        json_sec.add("Root Type", "Array / List");
                        json_sec.add("Array Items Count", arr.len().to_string());
                    }
                    other => {
                        json_sec.add("Root Type", format!("{other:?}"));
                    }
                }
            } else {
                json_sec.add("Valid JSON", "No (Syntax error)");
            }
            if !json_sec.entries.is_empty() {
                sections.push(json_sec);
            }
        } else if ext == "csv" || ext == "tsv" {
            let mut csv_sec = MetadataSection::new("Delimited Data");
            let delim = if ext == "tsv" { b'\t' } else { b',' };
            let mut rdr = csv::ReaderBuilder::new()
                .has_headers(true)
                .delimiter(delim)
                .from_reader(text.as_bytes());

            if let Ok(headers) = rdr.headers() {
                csv_sec.add("Column Count", headers.len().to_string());
                let header_names: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
                csv_sec.add("Headers", header_names.join(", "));
            }
            if !csv_sec.entries.is_empty() {
                sections.push(csv_sec);
            }
        } else if ext == "md" || ext == "markdown" {
            let mut md_sec = MetadataSection::new("Markdown Structure");
            let h1 = text.lines().filter(|l| l.starts_with("# ")).count();
            let h2 = text.lines().filter(|l| l.starts_with("## ")).count();
            let h3 = text.lines().filter(|l| l.starts_with("### ")).count();
            let code_blocks = text.lines().filter(|l| l.starts_with("```")).count() / 2;
            let links = text.matches("](").count();

            md_sec.add("Headings (H1/H2/H3)", format!("{h1} / {h2} / {h3}"));
            md_sec.add("Fenced Code Blocks", code_blocks.to_string());
            md_sec.add("Markdown Links", links.to_string());

            if !md_sec.entries.is_empty() {
                sections.push(md_sec);
            }
        }
    }

    if !text_sec.entries.is_empty() {
        sections.push(text_sec);
    }

    sections
}
