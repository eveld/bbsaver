/// A single character cell in the text buffer.
#[derive(Clone, Copy)]
pub struct Cell {
    pub glyph: u8,
    pub fg: u8,
    pub bg: u8,
}

impl Cell {
    pub const BLANK: Cell = Cell {
        glyph: b' ',
        fg: 7,  // light gray
        bg: 0,  // black
    };
}

/// A row of cells with variable width.
pub type Row = Vec<Cell>;

pub fn blank_row(cols: usize) -> Row {
    vec![Cell::BLANK; cols]
}

/// Pad or truncate a row to a target width.
pub fn resize_row(row: &mut Row, cols: usize) {
    row.resize(cols, Cell::BLANK);
}
