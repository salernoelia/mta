use std::fs::File;
use std::io::Read;
use std::path::Path;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use super::model::MetadataSection;

pub fn analyze_office_metadata(path: &Path) -> Vec<MetadataSection> {
    let mut sections = Vec::new();
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return sections,
    };

    let mut zip = match zip::ZipArchive::new(file) {
        Ok(z) => z,
        Err(_) => return sections,
    };

    let mut doc_sec = MetadataSection::new("Document Metadata");
    let mut stats_sec = MetadataSection::new("Document Statistics");

    // 1. Try Office OpenXML (DOCX, XLSX, PPTX)
    let mut core_xml = String::new();
    if let Ok(mut f) = zip.by_name("docProps/core.xml") {
        let _ = f.read_to_string(&mut core_xml);
    }
    let mut app_xml = String::new();
    if let Ok(mut f) = zip.by_name("docProps/app.xml") {
        let _ = f.read_to_string(&mut app_xml);
    }

    if !core_xml.is_empty() || !app_xml.is_empty() {
        parse_office_openxml_core(&core_xml, &mut doc_sec);
        parse_office_openxml_app(&app_xml, &mut doc_sec, &mut stats_sec);
    }

    // 2. Try OpenDocument (ODT, ODS, ODP)
    let mut meta_xml = String::new();
    if let Ok(mut f) = zip.by_name("meta.xml") {
        let _ = f.read_to_string(&mut meta_xml);
    }
    if !meta_xml.is_empty() {
        parse_opendocument_meta(&meta_xml, &mut doc_sec, &mut stats_sec);
    }

    // 3. Try EPUB
    let mut opf_path: Option<String> = None;
    if let Ok(mut f) = zip.by_name("META-INF/container.xml") {
        let mut container_xml = String::new();
        if f.read_to_string(&mut container_xml).is_ok() {
            opf_path = extract_epub_opf_path(&container_xml);
        }
    }

    let mut opf_xml = String::new();
    if let Some(ref path) = opf_path {
        if let Ok(mut f) = zip.by_name(path) {
            let _ = f.read_to_string(&mut opf_xml);
        }
    } else {
        // Fallback search for any .opf in the archive
        let mut found_opf_name = None;
        for i in 0..zip.len() {
            if let Ok(f) = zip.by_index(i) {
                if f.name().ends_with(".opf") {
                    found_opf_name = Some(f.name().to_string());
                    break;
                }
            }
        }
        if let Some(name) = found_opf_name {
            if let Ok(mut f) = zip.by_name(&name) {
                let _ = f.read_to_string(&mut opf_xml);
            }
        }
    }

    if !opf_xml.is_empty() {
        parse_epub_opf(&opf_xml, &mut doc_sec);
    }

    if !doc_sec.entries.is_empty() {
        sections.push(doc_sec);
    }
    if !stats_sec.entries.is_empty() {
        sections.push(stats_sec);
    }

    sections
}

fn parse_office_openxml_core(xml: &str, sec: &mut MetadataSection) {
    if xml.is_empty() {
        return;
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut current_tag = String::new();
    let mut buf = Vec::new();

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(ref e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_tag = name;
            }
            Event::Text(ref e) => {
                if let Ok(text) = e.unescape() {
                    let text = text.trim();
                    if !text.is_empty() {
                        let local_tag = current_tag.split(':').last().unwrap_or(&current_tag);
                        match local_tag {
                            "title" => sec.add("Title", text),
                            "creator" => sec.add("Author / Creator", text),
                            "lastModifiedBy" => sec.add("Last Modified By", text),
                            "revision" => sec.add("Revision", text),
                            "description" => sec.add("Description", text),
                            "subject" => sec.add("Subject", text),
                            "keywords" => sec.add("Keywords", text),
                            "category" => sec.add("Category", text),
                            "created" => sec.add("Creation Date", text),
                            "modified" => sec.add("Modification Date", text),
                            _ => {}
                        }
                    }
                }
            }
            Event::End(_) => {
                current_tag.clear();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
}

fn parse_office_openxml_app(xml: &str, doc_sec: &mut MetadataSection, stats_sec: &mut MetadataSection) {
    if xml.is_empty() {
        return;
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut current_tag = String::new();
    let mut buf = Vec::new();

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(ref e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_tag = name;
            }
            Event::Text(ref e) => {
                if let Ok(text) = e.unescape() {
                    let text = text.trim();
                    if !text.is_empty() {
                        let local_tag = current_tag.split(':').last().unwrap_or(&current_tag);
                        match local_tag {
                            "Application" => doc_sec.add("Creating Application", text),
                            "AppVersion" => doc_sec.add("Application Version", text),
                            "Company" => doc_sec.add("Company / Organization", text),
                            "Template" => doc_sec.add("Template", text),
                            "TotalTime" => {
                                if let Ok(mins) = text.parse::<u64>() {
                                    stats_sec.add("Total Editing Time", format!("{mins} minutes"));
                                } else {
                                    stats_sec.add("Total Editing Time", text);
                                }
                            }
                            "Pages" => stats_sec.add("Page Count", text),
                            "Words" => stats_sec.add("Word Count", text),
                            "Characters" => stats_sec.add("Character Count", text),
                            "CharactersWithSpaces" => stats_sec.add("Characters (with spaces)", text),
                            "Paragraphs" => stats_sec.add("Paragraph Count", text),
                            "Lines" => stats_sec.add("Line Count", text),
                            "Slides" => stats_sec.add("Slide Count", text),
                            "Notes" => stats_sec.add("Notes Count", text),
                            _ => {}
                        }
                    }
                }
            }
            Event::End(_) => {
                current_tag.clear();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
}

fn parse_opendocument_meta(xml: &str, doc_sec: &mut MetadataSection, stats_sec: &mut MetadataSection) {
    if xml.is_empty() {
        return;
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut current_tag = String::new();
    let mut buf = Vec::new();

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_tag = name.clone();

                if name.ends_with("document-statistic") {
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(&attr.value).to_string();
                        let local_key = key.split(':').last().unwrap_or(&key);
                        match local_key {
                            "page-count" => stats_sec.add("Page Count", val),
                            "word-count" => stats_sec.add("Word Count", val),
                            "character-count" => stats_sec.add("Character Count", val),
                            "paragraph-count" => stats_sec.add("Paragraph Count", val),
                            "table-count" => stats_sec.add("Table Count", val),
                            "image-count" => stats_sec.add("Image Count", val),
                            _ => {}
                        }
                    }
                }
            }
            Event::Text(ref e) => {
                if let Ok(text) = e.unescape() {
                    let text = text.trim();
                    if !text.is_empty() {
                        let local_tag = current_tag.split(':').last().unwrap_or(&current_tag);
                        match local_tag {
                            "title" => doc_sec.add("Title", text),
                            "description" => doc_sec.add("Description", text),
                            "creator" => doc_sec.add("Author / Creator", text),
                            "initial-creator" => doc_sec.add("Initial Creator", text),
                            "creation-date" => doc_sec.add("Creation Date", text),
                            "date" => doc_sec.add("Modification Date", text),
                            "generator" => doc_sec.add("Generating Software", text),
                            "editing-cycles" => stats_sec.add("Editing Cycles", text),
                            "editing-duration" => stats_sec.add("Editing Duration", text),
                            _ => {}
                        }
                    }
                }
            }
            Event::End(_) => {
                current_tag.clear();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
}

fn extract_epub_opf_path(container_xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(container_xml);
    let mut buf = Vec::new();
    while let Ok(event) = reader.read_event_into(&mut buf) {
        if let Event::Empty(ref e) | Event::Start(ref e) = event {
            let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
            if name.ends_with("rootfile") {
                for attr in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(attr.key.as_ref());
                    if k == "full-path" {
                        return Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
            }
        }
        if let Event::Eof = event {
            break;
        }
        buf.clear();
    }
    None
}

fn parse_epub_opf(xml: &str, sec: &mut MetadataSection) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut current_tag = String::new();
    let mut buf = Vec::new();

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(ref e) => {
                current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
            }
            Event::Text(ref e) => {
                if let Ok(text) = e.unescape() {
                    let text = text.trim();
                    if !text.is_empty() {
                        let local_tag = current_tag.split(':').last().unwrap_or(&current_tag);
                        match local_tag {
                            "title" => sec.add("Title", text),
                            "creator" => sec.add("Author / Creator", text),
                            "language" => sec.add("Language", text),
                            "identifier" => sec.add("Identifier / ISBN", text),
                            "publisher" => sec.add("Publisher", text),
                            "date" => sec.add("Release Date", text),
                            "rights" => sec.add("Rights / License", text),
                            _ => {}
                        }
                    }
                }
            }
            Event::End(_) => {
                current_tag.clear();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
}
