# Mta

<div align="center">
  <img src="icon.png" width="160" alt="Mta Icon" style="border-radius: 22%" />
  <p><em>Fast universal metadata reader with instant CSV export</em></p>
  <p>Pure Rust • Zero C Dependencies • Cross-Platform • Drag and Drop</p>
</div>

---

A lightweight desktop app and CLI to inspect metadata from any file. Drop in any image, document, audio, video, archive, font, or binary to view exhaustive properties and export them to CSV or JSON.

## Supported Formats

- **Images**: JPEG, PNG, GIF, WebP, TIFF, BMP, ICO, AVIF, HEIC, SVG (EXIF, camera, lens, GPS, dimensions)
- **PDFs**: Page count, version, encryption, title, author, producer
- **Office**: Word (.docx), Excel (.xlsx), PowerPoint (.pptx), OpenDocument (.odt, .ods, .odp), EPUB
- **Audio**: MP3, FLAC, OGG, WAV, M4A, AAC, OPUS (ID3, Vorbis, bitrate, sample rate, channels, duration)
- **Video**: MP4, MOV, MKV, WebM, AVI (codec, resolution, duration, tracks)
- **Archives**: ZIP, TAR, GZ, 7Z, APK (entry counts, compression ratio, file preview)
- **Binaries**: ELF, Mach-O, PE (architecture, bitness, entry point, linked libraries)
- **Code and Text**: Line counts, words, characters, line endings, encoding (UTF-8, UTF-16, ASCII)
- **Fonts**: TTF, OTF, WOFF, WOFF2 (family, subfamily, full name, PostScript name, designer)
- **Hashes and Entropy**: SHA-256, SHA-1, MD5, and Shannon entropy

## Quick Start

### GUI Mode

Run `mta` or launch the desktop application:

```bash
mta
```

You can also pass a file path directly on launch:

```bash
mta document.pdf
```

### CLI Quick Export

Inspect files directly from your terminal:

```bash
# Export metadata as CSV
mta --csv photo.jpg

# Export metadata as JSON
mta --json document.docx

# Export metadata as Markdown
mta --md track.flac
```

## Keyboard Shortcuts

- `Cmd/Ctrl + O`: Open file in Inspector
- `Cmd/Ctrl + Shift + O`: Add files to Batch
- `Cmd/Ctrl + W`: Close active file
- `Cmd/Ctrl + 1-4`: Switch between Inspector, Batch, History, and Settings
- `Esc`: Clear filter search

## Installation

### Pre-built Binaries

Download pre-compiled binaries from the GitHub Releases page:

- **macOS**: `mta-macos.app.tar.gz`
- **Linux**: `mta-linux`, `mta-linux.tar.gz`, `mta-linux.deb`
- **Windows**: `mta-windows.exe`

### From Source

```bash
git clone https://github.com/salernoelia/mta.git
cd mta
cargo build --release
```

The binary will be located in `target/release/mta`.

## License

MIT License. Copyright (c) 2026 Elia Salerno.
