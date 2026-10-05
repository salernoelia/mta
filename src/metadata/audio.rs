use std::path::Path;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use super::model::MetadataSection;

pub fn analyze_audio_metadata(path: &Path) -> Vec<MetadataSection> {
    let mut sections = Vec::new();
    let tagged_file = match Probe::open(path).and_then(|p| p.read()) {
        Ok(f) => f,
        Err(_) => return sections,
    };

    let mut tag_sec = MetadataSection::new("Audio Tags");
    let mut stream_sec = MetadataSection::new("Audio Stream & Format");

    let file_type = tagged_file.file_type();
    stream_sec.add("Audio Format", format!("{file_type:?}"));

    let props = tagged_file.properties();
    let duration = props.duration();
    let total_secs = duration.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    let millis = duration.subsec_millis();
    stream_sec.add("Duration", format!("{mins:02}:{secs:02}.{millis:03} ({total_secs}s)"));

    if let Some(bitrate) = props.overall_bitrate().or_else(|| props.audio_bitrate()) {
        stream_sec.add("Bitrate", format!("{bitrate} kbps"));
    }

    if let Some(sample_rate) = props.sample_rate() {
        stream_sec.add("Sample Rate", format!("{sample_rate} Hz"));
    }

    if let Some(channels) = props.channels() {
        let ch_str = match channels {
            1 => "1 (Mono)".to_string(),
            2 => "2 (Stereo)".to_string(),
            6 => "5.1 Surround".to_string(),
            8 => "7.1 Surround".to_string(),
            c => format!("{c} channels"),
        };
        stream_sec.add("Channels", ch_str);
    }

    if let Some(bit_depth) = props.bit_depth() {
        stream_sec.add("Bit Depth", format!("{bit_depth}-bit"));
    }

    // Tags
    if let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
        tag_sec.add_opt("Title", tag.title());
        tag_sec.add_opt("Artist", tag.artist());
        tag_sec.add_opt("Album", tag.album());
        tag_sec.add_opt("Genre", tag.genre());
        tag_sec.add_opt("Year / Date", tag.year());

        if let Some(track) = tag.track() {
            if let Some(total) = tag.track_total() {
                tag_sec.add("Track Number", format!("{track} of {total}"));
            } else {
                tag_sec.add("Track Number", track.to_string());
            }
        }

        if let Some(disc) = tag.disk() {
            if let Some(total) = tag.disk_total() {
                tag_sec.add("Disc Number", format!("{disc} of {total}"));
            } else {
                tag_sec.add("Disc Number", disc.to_string());
            }
        }

        tag_sec.add_opt("Comment", tag.comment());

        // Check for custom / extended items
        for item in tag.items() {
            let key = format!("{:?}", item.key());
            let val = match item.value() {
                lofty::tag::ItemValue::Text(s) => s.clone(),
                lofty::tag::ItemValue::Locator(s) => s.clone(),
                _ => continue,
            };
            if !val.trim().is_empty()
                && !["Title", "Artist", "Album", "Genre", "Year", "TrackNumber", "DiscNumber", "Comment"]
                    .iter()
                    .any(|k| key.contains(k))
            {
                tag_sec.add(key.replace("ItemKey::", ""), val);
            }
        }
    }

    if !tag_sec.entries.is_empty() {
        sections.push(tag_sec);
    }
    if !stream_sec.entries.is_empty() {
        sections.push(stream_sec);
    }

    sections
}
