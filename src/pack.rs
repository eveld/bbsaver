use std::path::Path;

use crate::ansi;
use crate::cell::{blank_row, Cell, Row};
use crate::sauce;

/// A loaded art file with its parsed content and metadata.
struct ArtFile {
    filename: String,
    sauce: Option<sauce::SauceRecord>,
    rows: Vec<Row>,
}

/// Load an art pack from a local path (directory or ZIP) or a URL.
/// `viewport_rows` is the number of rows visible on screen, used to pad between pieces
/// so each one scrolls fully off before the next appears.
pub fn load_pack(pack: &str, viewport_rows: usize) -> Vec<Row> {
    let files = if pack.starts_with("http://") || pack.starts_with("https://") {
        let path = download_pack(pack);
        load_zip(&path)
    } else {
        let path = Path::new(pack);
        if path.is_dir() {
            load_directory(path)
        } else {
            load_zip(path)
        }
    };

    if files.is_empty() {
        eprintln!("Warning: no .ANS/.ICE files found in pack");
        return Vec::new();
    }

    eprintln!("Loaded {} files from pack", files.len());

    let mut all_rows = Vec::new();
    for file in &files {
        all_rows.push(attribution_row(&file.filename, &file.sauce));
        all_rows.extend_from_slice(&file.rows);
        for _ in 0..viewport_rows {
            all_rows.push(blank_row());
        }
    }

    all_rows
}

/// Load .ANS/.ICE files from a directory, sorted alphabetically.
fn load_directory(dir: &Path) -> Vec<ArtFile> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("Failed to read pack directory")
        .filter_map(|e| e.ok())
        .filter(|e| is_ansi_file(e.file_name().to_str().unwrap_or("")))
        .collect();

    entries.sort_by_key(|e| e.file_name().to_ascii_lowercase());

    entries
        .iter()
        .filter_map(|entry| {
            let data = std::fs::read(entry.path()).ok()?;
            let filename = entry.file_name().to_string_lossy().to_string();
            Some(parse_art_file(filename, data))
        })
        .collect()
}

/// Load .ANS/.ICE files from a ZIP archive, sorted alphabetically.
fn load_zip(path: &Path) -> Vec<ArtFile> {
    let file = std::fs::File::open(path).expect("Failed to open ZIP file");
    let mut archive = zip::ZipArchive::new(file).expect("Failed to read ZIP archive");

    let mut names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let entry = archive.by_index(i).ok()?;
            let name = entry.name().to_string();
            if is_ansi_file(&name) {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    names.sort_by_key(|a| a.to_ascii_lowercase());

    let mut files = Vec::new();
    let mut total_bytes: u64 = 0;
    const MAX_BYTES: u64 = 64 * 1024 * 1024; // 64MB decompressed limit

    for name in &names {
        if let Ok(mut entry) = archive.by_name(name) {
            let size = entry.size();
            if total_bytes + size > MAX_BYTES {
                eprintln!("Warning: skipping remaining files (64MB decompressed limit)");
                break;
            }
            total_bytes += size;

            let mut data = Vec::with_capacity(size as usize);
            std::io::Read::read_to_end(&mut entry, &mut data).ok();

            // Use just the filename, not the full path inside the ZIP
            let filename = name.rsplit('/').next().unwrap_or(name).to_string();
            files.push(parse_art_file(filename, data));
        }
    }

    files
}

/// Download a ZIP pack from a URL to a temp file, return the path.
fn download_pack(url: &str) -> std::path::PathBuf {
    eprintln!("Downloading {}", url);

    let resp = ureq::get(url).call().expect("Failed to download pack");

    let mut body = resp.into_body();
    let mut tmp = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    std::io::copy(&mut body.as_reader(), &mut tmp).expect("Failed to write download");

    let path = tmp.into_temp_path();
    let owned = path.to_path_buf();
    // Keep the file alive by leaking the TempPath (cleaned up on process exit)
    std::mem::forget(path);
    owned
}

fn is_blank_row(row: &Row) -> bool {
    row.iter().all(|c| c.glyph == b' ' && c.bg == 0)
}

fn is_ansi_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".ans") || lower.ends_with(".ice")
}

fn parse_art_file(filename: String, data: Vec<u8>) -> ArtFile {
    let sauce_record = sauce::parse_sauce(&data);
    let content = sauce::strip_sauce(&data);
    let mut rows = ansi::parse_ansi(content);

    // Trim leading blank rows
    let first_nonblank = rows.iter().position(|r| !is_blank_row(r)).unwrap_or(0);
    rows.drain(..first_nonblank);

    // Trim trailing blank rows
    while rows.last().is_some_and(is_blank_row) {
        rows.pop();
    }

    ArtFile {
        filename,
        sauce: sauce_record,
        rows,
    }
}

/// Create an attribution row: --- "Title" by Author / Group ---
fn attribution_row(filename: &str, sauce: &Option<sauce::SauceRecord>) -> Row {
    let text = match sauce {
        Some(s) if !s.title.is_empty() || !s.author.is_empty() => {
            let title = if s.title.is_empty() { filename } else { &s.title };
            let author = if s.author.is_empty() { "?" } else { &s.author };
            if s.group.is_empty() {
                format!(" \"{}\" by {} ", title, author)
            } else {
                format!(" \"{}\" by {} / {} ", title, author, s.group)
            }
        }
        _ => format!(" {} ", filename),
    };

    let mut row = blank_row();

    // Fill with dashes, then place text centered
    for cell in row.iter_mut() {
        *cell = Cell {
            glyph: 0xC4, // horizontal box-drawing char
            fg: 8,        // dark gray
            bg: 0,
        };
    }

    // Center the text
    let text_bytes: Vec<u8> = text.bytes().collect();
    let start = if text_bytes.len() < 80 {
        (80 - text_bytes.len()) / 2
    } else {
        0
    };

    for (i, &b) in text_bytes.iter().enumerate() {
        let col = start + i;
        if col < 80 {
            row[col] = Cell {
                glyph: b,
                fg: 8, // dark gray
                bg: 0,
            };
        }
    }

    row
}
