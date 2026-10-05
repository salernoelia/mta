# Mta

A lightweight, robust, cross-platform metadata reader and inspector built with pure Rust and `egui`. `Mta` extracts exhaustive metadata from almost any file format—including images, PDFs, office documents, audio, video, archives, binaries, source code, and fonts—presents them in a clean, categorized table format, and allows instant one-click CSV export.

<div align="center">
  <img src="icon.png" width="180" alt="Mta Icon" />
  <p><em>Pure Rust • Zero External C Dependencies • Cross-Platform • Instant Drag & Drop</em></p>
</div>

---

## Features

* **Universal File Format Support**:
  * **Images**: JPEG, PNG, GIF, WebP, TIFF, BMP, ICO, AVIF, HEIC, SVG. Extracts full EXIF camera properties (Make, Model, Lens, ISO, F-Number, Exposure Time, White Balance, Focal Length), GPS coordinates with instant map links, dimensions, aspect ratios, color spaces, and megapixels.
  * **PDF Documents**: PDF version, page count, encryption status, and document metadata (Title, Author, Subject, Keywords, Creator Application, Producer, Creation Date, Modification Date).
  * **Office & OpenDocument**: Word (`.docx`), Excel (`.xlsx`), PowerPoint (`.pptx`), OpenDocument (`.odt`, `.ods`, `.odp`), and EPUB (`.epub`). Extracts core properties, revision cycles, total editing time, word/character/page counts, slide counts, and author information via pure Rust XML decompression.
  * **Audio Files**: MP3, FLAC, OGG, WAV, M4A, AAC, OPUS, AIFF, WMA. Reads ID3v1/ID3v2 and Vorbis tags (Title, Artist, Album, Year, Genre, Track/Disc numbers), plus audio stream metrics (duration, bitrate, sample rate, channels, bit depth).
  * **Video Containers**: MP4, MOV, MKV, WebM, AVI. Parses atom boxes and EBML structures for duration, video resolution, aspect ratio, audio/video track layout, timescales, and brands.
  * **Archives & Packages**: ZIP, TAR, GZ, TGZ, 7Z, JAR, APK. Computes total entry count, uncompressed vs compressed sizes, compression ratio savings, password protection status, and archive contents preview.
  * **Executables & Binaries**: ELF (Linux), PE / COFF (Windows .exe / .dll), and Mach-O (macOS / iOS fat and single binaries). Inspects bitness (32/64-bit), architecture, entry point addresses, subsystem, sections, and linked shared libraries / dylibs.
  * **Source Code & Text**: Plain text, Markdown, JSON, YAML, TOML, XML, CSV, TSV, and source code. Analyzes total lines, non-empty lines, word count, character count, encoding detection (UTF-8, UTF-16, ASCII, BOM), line endings (LF vs CRLF), and structural validation.
  * **Fonts**: TrueType (`.ttf`), OpenType (`.otf`), WOFF, and WOFF2. Parses the OpenType `name` table for font family, subfamily style, full name, PostScript name, version, designer, and license.
* **Cryptographic Hashes & Entropy**: Instant buffered streaming calculation of SHA-256, SHA-1, and MD5 hashes, alongside Shannon entropy (useful to identify encrypted or compressed data).
* **Drag-and-Drop Workflow**: Simply drag any file or group of files into the window from your system file manager to inspect it immediately.
* **Clean Table Format**: All properties are neatly organized into expandable/filterable categorized sections with alternating row stripes and one-click copy buttons for every individual value.
* **One-Click CSV & JSON Export**: Export single or batch inspection reports to cleanly structured CSV or JSON files, or copy CSV straight to your clipboard.
* **Batch Inspection**: Inspect whole directories or lists of files simultaneously in a dedicated batch table, with bulk CSV export capabilities.
* **Persistent History**: Keeps track of recently inspected files with fast re-inspection and removal.
* **Automatic Updates**: Built-in GitHub release detection and self-updater powered by `self-replace`, allowing seamless in-app upgrades.
* **100% Pure Rust**: Zero dependencies on external system C libraries or tools like ffmpeg or python. Builds and runs portably across macOS, Linux, and Windows.

---

## Installation

### Pre-built Binaries
Pre-compiled native binaries are available from the GitHub **Releases** tab:
* **macOS**: `Mta.app.tar.gz` (Universal bundle for Apple Silicon and Intel)
* **Linux**: `mta-linux` binary, `mta-linux.tar.gz`, and `mta-linux.deb` Debian package
* **Windows**: `mta-windows.exe` portable executable

### Building from Source
Ensure you have Rust installed (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`):

```bash
git clone https://github.com/salernoelia/mta.git
cd mta
cargo build --release
```

The resulting binary will be in `target/release/mta`.

---

## Usage

### Graphical Interface

Simply run `mta` or launch the desktop application:

```bash
mta
```

You can also pass a file directly on launch:

```bash
mta path/to/document.pdf
```

### CLI Quick Modes

`Mta` can also be used as a lightning-fast terminal metadata extractor without opening the GUI:

```bash
# Export file metadata as CSV to stdout
mta --csv sample.jpg

# Export file metadata as JSON to stdout
mta --json document.docx

# Export file metadata as Markdown table to stdout
mta --markdown track.flac
```

---

## Configuration & Local Storage

`Mta` saves settings and history locally in standard user data folders:

| Platform | Configuration (`config.json`) | Inspection History (`history.json`) |
| :--- | :--- | :--- |
| **macOS** | `~/Library/Application Support/mta/` | `~/Library/Application Support/mta/` |
| **Linux** | `~/.local/share/mta/` | `~/.local/share/mta/` |
| **Windows** | `%APPDATA%\mta\` | `%APPDATA%\mta\` |

---

## License

MIT License. Copyright (c) 2026 Elia Salerno.
