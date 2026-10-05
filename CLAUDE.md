# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                  # debug build
cargo build --release        # release build
cargo run                    # run GUI app
cargo run -- <file_path>     # run GUI with file loaded
cargo run -- --csv <file>    # dump metadata as CSV to stdout
cargo run -- --json <file>   # dump metadata as JSON to stdout
cargo run -- --md <file>     # dump metadata as Markdown to stdout
cargo test                   # run test suite
```

## Architecture

Single-binary pure Rust desktop app built with `egui` / `eframe`. Entry point: `src/main.rs`.

**Module overview:**

| Module | Role |
|---|---|
| `app.rs` | `MtaApp` — egui update loop, UI tabs (Inspector/Batch/History/Settings), drag-and-drop handling |
| `metadata/mod.rs` | Unified file inspector dispatcher — routes path to specialized extractors |
| `metadata/model.rs` | Core report structures (`FileMetadataReport`, `MetadataSection`, `MetadataEntry`, CSV/JSON serializers) |
| `metadata/general.rs` | File system properties, timestamps, permissions, hashes (MD5, SHA-1, SHA-256), Shannon entropy, MIME sniffing |
| `metadata/image.rs` | Image geometry (dimensions, megapixels, aspect ratio), EXIF camera data & GPS coordinates via `kamadak-exif` |
| `metadata/pdf.rs` | PDF document metadata (version, page count, encryption, author, title, creation date) via `lopdf` |
| `metadata/office.rs` | Office OpenXML (DOCX, XLSX, PPTX), OpenDocument (ODT, ODS, ODP), and EPUB parsing via `zip` + `quick-xml` |
| `metadata/audio.rs` | Audio tags (ID3, Vorbis) and stream properties (duration, bitrate, channels, sample rate) via `lofty` |
| `metadata/video.rs` | Video container & track metadata (MP4/MOV atom boxes, resolution, timescale, duration, MKV/WebM) |
| `metadata/archive.rs` | Archive inspection (ZIP, TAR, GZ) with file counts, uncompressed/compressed ratios, contents preview |
| `metadata/binary.rs` | Binary & executable inspection (ELF, PE, Mach-O) via `goblin` (bitness, CPU type, entry point, linked libraries) |
| `metadata/text.rs` | Text, source code, JSON, CSV, and Markdown metrics (lines, words, characters, encoding, line endings) |
| `metadata/font.rs` | OpenType and TrueType font metadata (`name` table parsing: family, style, designer, license) |
| `exporter.rs` | CSV, JSON, Markdown generation, file dialog saving via `rfd`, and clipboard integration |
| `history.rs` | Persistent JSON inspection history (`~/.local/share/mta/history.json` or `%APPDATA%\mta\history.json`) |
| `config.rs` | App configuration (theme, auto-copy, checksum calculation) |
| `updater.rs` | Auto-update checker & self-updater from GitHub releases via `self-replace` and `reqwest` |
| `icon.rs` | Window icon loader with anti-aliased squircle inset |

**Key Features:**
- 100% pure Rust — zero external C/C++ libraries or system toolchain requirements.
- Drag-and-drop file inspection from anywhere in the OS.
- Tabular display with categorized metadata sections, search/filter, and per-row copy buttons.
- Single and batch CSV / JSON export with native file dialogs.
- Automatic self-updating from GitHub releases.
