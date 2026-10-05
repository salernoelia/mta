use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use super::model::MetadataSection;

pub fn analyze_image_metadata(path: &Path) -> Vec<MetadataSection> {
    let mut sections = Vec::new();
    let mut geom_sec = MetadataSection::new("Image Properties");
    let mut exif_sec = MetadataSection::new("Camera & EXIF");
    let mut gps_sec = MetadataSection::new("GPS & Geolocation");

    // Dimensions, Aspect ratio, Megapixels
    if let Ok((width, height)) = image::image_dimensions(path) {
        geom_sec.add("Dimensions", format!("{width} × {height} pixels"));
        geom_sec.add("Width", format!("{width} px"));
        geom_sec.add("Height", format!("{height} px"));

        let mp = (width as f64 * height as f64) / 1_000_000.0;
        geom_sec.add("Megapixels", format!("{mp:.2} MP"));

        let gcd_val = gcd(width, height);
        if gcd_val > 0 {
            let aspect_w = width / gcd_val;
            let aspect_h = height / gcd_val;
            // Simplify common aspect ratios
            let ratio_str = match (aspect_w, aspect_h) {
                (16, 9) | (8, 5) | (4, 3) | (3, 2) | (1, 1) | (5, 4) | (21, 9) => {
                    format!("{aspect_w}:{aspect_h}")
                }
                (9, 16) | (5, 8) | (3, 4) | (2, 3) | (4, 5) => {
                    format!("{aspect_w}:{aspect_h} (Portrait)")
                }
                _ => {
                    let decimal = width as f64 / height as f64;
                    format!("{aspect_w}:{aspect_h} ({decimal:.2}:1)")
                }
            };
            geom_sec.add("Aspect Ratio", ratio_str);
        }
    }

    // Try reading image format header details
    if let Ok(reader) = image::ImageReader::open(path) {
        if let Some(format) = reader.format() {
            geom_sec.add("Image Format", format!("{format:?}"));
        }
    }

    // EXIF Extraction
    if let Ok(file) = File::open(path) {
        let mut bufreader = BufReader::new(file);
        let exifreader = exif::Reader::new();
        if let Ok(exif) = exifreader.read_from_container(&mut bufreader) {
            let mut lat: Option<f64> = None;
            let mut lat_ref: Option<String> = None;
            let mut lon: Option<f64> = None;
            let mut lon_ref: Option<String> = None;
            let mut alt: Option<f64> = None;

            for field in exif.fields() {
                let tag = field.tag;
                let val_str = field.display_value().with_unit(&exif).to_string();

                match tag {
                    exif::Tag::Make => exif_sec.add("Camera Make", &val_str),
                    exif::Tag::Model => exif_sec.add("Camera Model", &val_str),
                    exif::Tag::LensMake => exif_sec.add("Lens Make", &val_str),
                    exif::Tag::LensModel => exif_sec.add("Lens Model", &val_str),
                    exif::Tag::Software => exif_sec.add("Software", &val_str),
                    exif::Tag::Artist => exif_sec.add("Artist", &val_str),
                    exif::Tag::Copyright => exif_sec.add("Copyright", &val_str),
                    exif::Tag::DateTimeOriginal => exif_sec.add("Date Taken", &val_str),
                    exif::Tag::DateTimeDigitized => exif_sec.add("Date Digitized", &val_str),
                    exif::Tag::ExposureTime => exif_sec.add("Exposure Time", &val_str),
                    exif::Tag::FNumber => exif_sec.add("F-Number", &val_str),
                    exif::Tag::PhotographicSensitivity => exif_sec.add("ISO", &val_str),
                    exif::Tag::FocalLength => exif_sec.add("Focal Length", &val_str),
                    exif::Tag::FocalLengthIn35mmFilm => exif_sec.add("Focal Length (35mm equivalent)", &val_str),
                    exif::Tag::ExposureProgram => exif_sec.add("Exposure Program", &val_str),
                    exif::Tag::MeteringMode => exif_sec.add("Metering Mode", &val_str),
                    exif::Tag::Flash => exif_sec.add("Flash", &val_str),
                    exif::Tag::WhiteBalance => exif_sec.add("White Balance", &val_str),
                    exif::Tag::ColorSpace => exif_sec.add("Color Space", &val_str),
                    exif::Tag::Orientation => exif_sec.add("Orientation", &val_str),
                    exif::Tag::ImageWidth => exif_sec.add("Exif Image Width", &val_str),
                    exif::Tag::ImageLength => exif_sec.add("Exif Image Height", &val_str),
                    exif::Tag::XResolution => exif_sec.add("Horizontal Resolution", &val_str),
                    exif::Tag::YResolution => exif_sec.add("Vertical Resolution", &val_str),
                    exif::Tag::ResolutionUnit => exif_sec.add("Resolution Unit", &val_str),
                    exif::Tag::ExposureBiasValue => exif_sec.add("Exposure Bias", &val_str),
                    exif::Tag::MaxApertureValue => exif_sec.add("Max Aperture", &val_str),
                    exif::Tag::LightSource => exif_sec.add("Light Source", &val_str),
                    exif::Tag::SensingMethod => exif_sec.add("Sensing Method", &val_str),
                    exif::Tag::DigitalZoomRatio => exif_sec.add("Digital Zoom Ratio", &val_str),
                    exif::Tag::SceneCaptureType => exif_sec.add("Scene Capture Type", &val_str),
                    exif::Tag::GainControl => exif_sec.add("Gain Control", &val_str),
                    exif::Tag::Contrast => exif_sec.add("Contrast", &val_str),
                    exif::Tag::Saturation => exif_sec.add("Saturation", &val_str),
                    exif::Tag::Sharpness => exif_sec.add("Sharpness", &val_str),
                    exif::Tag::SubjectDistanceRange => exif_sec.add("Subject Distance", &val_str),

                    // GPS Tags
                    exif::Tag::GPSLatitude => {
                        lat = parse_dms_to_degrees(&field.value);
                    }
                    exif::Tag::GPSLatitudeRef => {
                        lat_ref = Some(val_str);
                    }
                    exif::Tag::GPSLongitude => {
                        lon = parse_dms_to_degrees(&field.value);
                    }
                    exif::Tag::GPSLongitudeRef => {
                        lon_ref = Some(val_str);
                    }
                    exif::Tag::GPSAltitude => {
                        if let exif::Value::Rational(ref rationals) = field.value {
                            if let Some(r) = rationals.first() {
                                alt = Some(r.to_f64());
                            }
                        }
                    }
                    exif::Tag::GPSDateStamp => gps_sec.add("GPS Date", &val_str),
                    exif::Tag::GPSTimeStamp => gps_sec.add("GPS Time (UTC)", &val_str),
                    _ => {}
                }
            }

            // Synthesize formatted GPS Coordinates if present
            if let (Some(mut lat_deg), Some(mut lon_deg)) = (lat, lon) {
                if let Some(ref r) = lat_ref {
                    if r.trim().eq_ignore_ascii_case("S") {
                        lat_deg = -lat_deg;
                    }
                }
                if let Some(ref r) = lon_ref {
                    if r.trim().eq_ignore_ascii_case("W") {
                        lon_deg = -lon_deg;
                    }
                }
                gps_sec.add("Latitude", format!("{lat_deg:.6}°"));
                gps_sec.add("Longitude", format!("{lon_deg:.6}°"));
                gps_sec.add("Coordinates", format!("{lat_deg:.6}, {lon_deg:.6}"));
                gps_sec.add(
                    "OpenStreetMap",
                    format!("https://www.openstreetmap.org/?mlat={lat_deg}&mlon={lon_deg}#map=16/{lat_deg}/{lon_deg}"),
                );
                if let Some(a) = alt {
                    gps_sec.add("Altitude", format!("{a:.1} m"));
                }
            }
        }
    }

    if !geom_sec.entries.is_empty() {
        sections.push(geom_sec);
    }
    if !exif_sec.entries.is_empty() {
        sections.push(exif_sec);
    }
    if !gps_sec.entries.is_empty() {
        sections.push(gps_sec);
    }

    sections
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn parse_dms_to_degrees(val: &exif::Value) -> Option<f64> {
    if let exif::Value::Rational(ref rationals) = val {
        if rationals.len() >= 3 {
            let deg = rationals[0].to_f64();
            let min = rationals[1].to_f64();
            let sec = rationals[2].to_f64();
            return Some(deg + (min / 60.0) + (sec / 3600.0));
        }
    }
    None
}
