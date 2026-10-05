use lofty::file::AudioFile;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use super::model::MetadataSection;

pub fn analyze_video_metadata(path: &Path) -> Vec<MetadataSection> {
    let mut sections = Vec::new();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mut video_sec = MetadataSection::new("Video & Container");

    if ext == "mp4" || ext == "mov" || ext == "m4v" {
        parse_mp4_mov_metadata(path, &mut video_sec);
    } else if ext == "mkv" || ext == "webm" {
        parse_ebml_metadata(path, &mut video_sec);
    }

    // Also check lofty tags for video files (MP4/MKV often have title/artist/duration)
    if let Ok(tagged_file) = lofty::probe::Probe::open(path).and_then(|p| p.read()) {
        let props = tagged_file.properties();
        let dur = props.duration();
        if dur.as_secs() > 0 && !video_sec.entries.iter().any(|e| e.key == "Duration") {
            let total = dur.as_secs();
            let h = total / 3600;
            let m = (total % 3600) / 60;
            let s = total % 60;
            let dur_str = if h > 0 {
                format!("{h:02}:{m:02}:{s:02}")
            } else {
                format!("{m:02}:{s:02}")
            };
            video_sec.add("Duration", dur_str);
        }
    }

    if !video_sec.entries.is_empty() {
        sections.push(video_sec);
    }

    sections
}

fn parse_mp4_mov_metadata(path: &Path, sec: &mut MetadataSection) {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };

    let mut buffer = [0u8; 8];
    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut current_offset = 0u64;

    while current_offset + 8 <= file_len {
        if file.seek(SeekFrom::Start(current_offset)).is_err() {
            break;
        }
        if file.read_exact(&mut buffer).is_err() {
            break;
        }

        let size = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as u64;
        let fourcc = &buffer[4..8];
        let tag = String::from_utf8_lossy(fourcc).to_string();

        let atom_size = if size == 1 {
            // 64-bit large size
            let mut large_buf = [0u8; 8];
            if file.read_exact(&mut large_buf).is_err() {
                break;
            }
            u64::from_be_bytes(large_buf)
        } else if size == 0 {
            file_len - current_offset
        } else {
            size
        };

        if atom_size < 8 {
            break;
        }

        match tag.as_str() {
            "ftyp" => {
                let mut data = vec![0u8; (atom_size - 8).min(256) as usize];
                if file.read_exact(&mut data).is_ok() && data.len() >= 8 {
                    let major_brand = String::from_utf8_lossy(&data[0..4]).trim().to_string();
                    let minor_ver = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                    sec.add("Major Brand", major_brand);
                    sec.add("Minor Version", minor_ver.to_string());
                }
            }
            "moov" => {
                // Parse inside moov
                parse_moov_atom(&mut file, current_offset + 8, atom_size - 8, sec);
            }
            _ => {}
        }

        current_offset += atom_size;
    }
}

fn parse_moov_atom(file: &mut File, start: u64, len: u64, sec: &mut MetadataSection) {
    let mut offset = start;
    let end = start + len;
    let mut buffer = [0u8; 8];

    while offset + 8 <= end {
        if file.seek(SeekFrom::Start(offset)).is_err() {
            break;
        }
        if file.read_exact(&mut buffer).is_err() {
            break;
        }

        let size = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as u64;
        let tag = String::from_utf8_lossy(&buffer[4..8]).to_string();

        let atom_size = if size == 1 {
            let mut large_buf = [0u8; 8];
            if file.read_exact(&mut large_buf).is_err() {
                break;
            }
            u64::from_be_bytes(large_buf)
        } else if size == 0 {
            end - offset
        } else {
            size
        };

        if atom_size < 8 {
            break;
        }

        if tag == "mvhd" {
            // Movie header atom
            let mut data = vec![0u8; (atom_size - 8).min(128) as usize];
            if file.read_exact(&mut data).is_ok() && !data.is_empty() {
                let version = data[0];
                if version == 0 && data.len() >= 24 {
                    let timescale = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
                    let duration_units = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
                    if timescale > 0 {
                        let secs = duration_units as f64 / timescale as f64;
                        let m = (secs / 60.0).floor() as u64;
                        let s = (secs % 60.0).floor() as u64;
                        sec.add("Duration", format!("{m:02}:{s:02} ({secs:.2} s)"));
                        sec.add("Timescale", format!("{timescale} Hz"));
                    }
                } else if version == 1 && data.len() >= 36 {
                    let timescale = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
                    let duration_units = u64::from_be_bytes([
                        data[24], data[25], data[26], data[27], data[28], data[29], data[30], data[31],
                    ]);
                    if timescale > 0 {
                        let secs = duration_units as f64 / timescale as f64;
                        let m = (secs / 60.0).floor() as u64;
                        let s = (secs % 60.0).floor() as u64;
                        sec.add("Duration", format!("{m:02}:{s:02} ({secs:.2} s)"));
                        sec.add("Timescale", format!("{timescale} Hz"));
                    }
                }
            }
        } else if tag == "trak" {
            parse_trak_atom(file, offset + 8, atom_size - 8, sec);
        }

        offset += atom_size;
    }
}

fn parse_trak_atom(file: &mut File, start: u64, len: u64, sec: &mut MetadataSection) {
    let mut offset = start;
    let end = start + len;
    let mut buffer = [0u8; 8];

    while offset + 8 <= end {
        if file.seek(SeekFrom::Start(offset)).is_err() {
            break;
        }
        if file.read_exact(&mut buffer).is_err() {
            break;
        }

        let size = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as u64;
        let tag = String::from_utf8_lossy(&buffer[4..8]).to_string();

        let atom_size = if size == 1 {
            let mut large_buf = [0u8; 8];
            if file.read_exact(&mut large_buf).is_err() {
                break;
            }
            u64::from_be_bytes(large_buf)
        } else {
            size
        };

        if atom_size < 8 {
            break;
        }

        if tag == "tkhd" {
            let mut data = vec![0u8; (atom_size - 8).min(96) as usize];
            if file.read_exact(&mut data).is_ok() && data.len() >= 84 {
                // width and height are 16.16 fixed point numbers at the end of tkhd
                let w_idx = data.len() - 8;
                let h_idx = data.len() - 4;
                let w = u16::from_be_bytes([data[w_idx], data[w_idx + 1]]);
                let h = u16::from_be_bytes([data[h_idx], data[h_idx + 1]]);
                if w > 0 && h > 0 && !sec.entries.iter().any(|e| e.key == "Video Resolution") {
                    sec.add("Video Resolution", format!("{w} × {h}"));
                    sec.add("Aspect Ratio", format!("{:.2}:1", w as f64 / h as f64));
                }
            }
        } else if tag == "mdia" {
            // Find hdlr or minf
            parse_mdia_atom(file, offset + 8, atom_size - 8, sec);
        }

        offset += atom_size;
    }
}

fn parse_mdia_atom(file: &mut File, start: u64, len: u64, sec: &mut MetadataSection) {
    let mut offset = start;
    let end = start + len;
    let mut buffer = [0u8; 8];

    while offset + 8 <= end {
        if file.seek(SeekFrom::Start(offset)).is_err() {
            break;
        }
        if file.read_exact(&mut buffer).is_err() {
            break;
        }
        let size = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as u64;
        let tag = String::from_utf8_lossy(&buffer[4..8]).to_string();
        if size < 8 {
            break;
        }

        if tag == "hdlr" {
            let mut data = vec![0u8; (size - 8).min(64) as usize];
            if file.read_exact(&mut data).is_ok() && data.len() >= 12 {
                let handler_type = String::from_utf8_lossy(&data[8..12]).to_string();
                let handler_desc = match handler_type.as_str() {
                    "vide" => "Video Track",
                    "soun" => "Audio Track",
                    "subt" | "text" | "sbtl" => "Subtitle Track",
                    _ => &handler_type,
                };
                sec.add("Track Component", handler_desc);
            }
        }

        offset += size;
    }
}

fn parse_ebml_metadata(path: &Path, sec: &mut MetadataSection) {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut header = [0u8; 4096];
    let n = file.read(&mut header).unwrap_or(0);
    if n < 4 {
        return;
    }

    // EBML magic bytes: 0x1A 0x45 0xDF 0xA3
    if header[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        sec.add("Container Format", "EBML (Extensible Binary Meta Language)");
        let text_repr = String::from_utf8_lossy(&header[..n]);
        if text_repr.contains("matroska") {
            sec.add("Media Subtype", "Matroska (MKV)");
        } else if text_repr.contains("webm") {
            sec.add("Media Subtype", "WebM");
        }
    }
}
