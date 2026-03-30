/// SAUCE (Standard Architecture for Universal Comment Extensions) metadata.
/// 128-byte record appended to the end of ANSI art files.
#[allow(dead_code)]
pub struct SauceRecord {
    pub title: String,
    pub author: String,
    pub group: String,
    pub width: u16,
    pub height: u16,
    pub ice_colors: bool,
    pub font_name: String,
}

/// Parse SAUCE metadata from file data. Returns None if no valid SAUCE record found.
pub fn parse_sauce(data: &[u8]) -> Option<SauceRecord> {
    if data.len() < 128 {
        return None;
    }

    let sauce = &data[data.len() - 128..];

    // Check "SAUCE" magic at offset 0
    if &sauce[0..5] != b"SAUCE" {
        return None;
    }

    let title = cp437_string(&sauce[7..42]);
    let author = cp437_string(&sauce[42..62]);
    let group = cp437_string(&sauce[62..82]);
    let width = u16::from_le_bytes([sauce[96], sauce[97]]);
    let height = u16::from_le_bytes([sauce[98], sauce[99]]);
    let tflags = sauce[105];
    let ice_colors = tflags & 1 != 0;
    let font_name = cp437_string(&sauce[106..128]);

    Some(SauceRecord {
        title,
        author,
        group,
        width,
        height,
        ice_colors,
        font_name,
    })
}

/// Strip SAUCE record and trailing EOF (0x1A) from file data, returning just the content bytes.
pub fn strip_sauce(data: &[u8]) -> &[u8] {
    if data.len() >= 128 && &data[data.len() - 128..data.len() - 123] == b"SAUCE" {
        let content = &data[..data.len() - 128];
        // Strip trailing EOF character (0x1A) if present
        if content.last() == Some(&0x1A) {
            &content[..content.len() - 1]
        } else {
            content
        }
    } else {
        data
    }
}

/// Decode a CP437 byte slice as a trimmed string.
/// Strips trailing spaces and null bytes.
fn cp437_string(bytes: &[u8]) -> String {
    use crate::cp437::CP437_TO_UNICODE;

    let s: String = bytes.iter().map(|&b| CP437_TO_UNICODE[b as usize]).collect();
    s.trim_end_matches([' ', '\0']).to_string()
}
