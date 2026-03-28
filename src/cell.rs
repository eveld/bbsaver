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

/// A row of 80 cells.
pub type Row = [Cell; 80];

pub fn blank_row() -> Row {
    [Cell::BLANK; 80]
}
