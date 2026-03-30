use crate::cell::{blank_row, Cell, Row};

/// Parse ANSI-encoded CP437 bytes into a grid of cell rows.
/// `cols` is the canvas width (from SAUCE or default 80).
pub fn parse_ansi(data: &[u8], cols: usize) -> Vec<Row> {
    let mut canvas = Canvas::new(cols);
    let mut state = ParserState::Ground;
    let mut params: Vec<u16> = Vec::new();
    let mut current_param: Option<u16> = None;

    for &byte in data {
        match state {
            ParserState::Ground => {
                match byte {
                    0x1B => {
                        state = ParserState::Escape;
                    }
                    0x0D => {
                        canvas.cursor_col = 0;
                    }
                    0x0A => {
                        canvas.cursor_row += 1;
                        canvas.ensure_row(canvas.cursor_row);
                    }
                    _ => {
                        canvas.put_char(byte);
                    }
                }
            }
            ParserState::Escape => {
                if byte == b'[' {
                    state = ParserState::CsiParam;
                    params.clear();
                    current_param = None;
                } else {
                    state = ParserState::Ground;
                }
            }
            ParserState::CsiParam => {
                if byte.is_ascii_digit() {
                    let digit = (byte - b'0') as u16;
                    current_param = Some(current_param.unwrap_or(0) * 10 + digit);
                } else if byte == b';' {
                    params.push(current_param.unwrap_or(0));
                    current_param = None;
                } else if (0x40..=0x7E).contains(&byte) {
                    params.push(current_param.unwrap_or(0));
                    canvas.dispatch_csi(byte, &params);
                    state = ParserState::Ground;
                } else {
                    state = ParserState::Ground;
                }
            }
        }
    }

    canvas.rows
}

enum ParserState {
    Ground,
    Escape,
    CsiParam,
}

struct Canvas {
    cols: usize,
    rows: Vec<Row>,
    cursor_row: usize,
    cursor_col: usize,
    fg: u8,
    bg: u8,
    bold: bool,
    saved_row: usize,
    saved_col: usize,
}

impl Canvas {
    fn new(cols: usize) -> Self {
        Canvas {
            cols,
            rows: vec![blank_row(cols)],
            cursor_row: 0,
            cursor_col: 0,
            fg: 7,
            bg: 0,
            bold: false,
            saved_row: 0,
            saved_col: 0,
        }
    }

    fn ensure_row(&mut self, row: usize) {
        while self.rows.len() <= row {
            self.rows.push(blank_row(self.cols));
        }
    }

    fn put_char(&mut self, glyph: u8) {
        self.ensure_row(self.cursor_row);

        if self.cursor_col < self.cols {
            let fg = if self.bold { self.fg | 8 } else { self.fg };
            self.rows[self.cursor_row][self.cursor_col] = Cell {
                glyph,
                fg,
                bg: self.bg,
            };
            self.cursor_col += 1;
        }

        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.cursor_row += 1;
            self.ensure_row(self.cursor_row);
        }
    }

    fn dispatch_csi(&mut self, cmd: u8, params: &[u16]) {
        match cmd {
            b'm' => self.sgr(params),

            b'A' => {
                let n = params.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }

            b'B' => {
                let n = params.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_row += n;
                self.ensure_row(self.cursor_row);
            }

            b'C' => {
                let n = params.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_col = (self.cursor_col + n).min(self.cols - 1);
            }

            b'D' => {
                let n = params.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }

            b'H' | b'f' => {
                let row = params.first().copied().unwrap_or(1).max(1) as usize - 1;
                let col = params.get(1).copied().unwrap_or(1).max(1) as usize - 1;
                self.cursor_row = row;
                self.cursor_col = col.min(self.cols - 1);
                self.ensure_row(self.cursor_row);
            }

            b'J' => {
                let mode = params.first().copied().unwrap_or(0);
                match mode {
                    2 => {
                        for row in self.rows.iter_mut() {
                            *row = blank_row(self.cols);
                        }
                        self.cursor_row = 0;
                        self.cursor_col = 0;
                    }
                    0 => {
                        self.ensure_row(self.cursor_row);
                        for col in self.cursor_col..self.cols {
                            self.rows[self.cursor_row][col] = Cell::BLANK;
                        }
                        for row in (self.cursor_row + 1)..self.rows.len() {
                            self.rows[row] = blank_row(self.cols);
                        }
                    }
                    _ => {}
                }
            }

            b'K' => {
                let mode = params.first().copied().unwrap_or(0);
                self.ensure_row(self.cursor_row);
                match mode {
                    0 => {
                        for col in self.cursor_col..self.cols {
                            self.rows[self.cursor_row][col] = Cell::BLANK;
                        }
                    }
                    1 => {
                        for col in 0..=self.cursor_col.min(self.cols - 1) {
                            self.rows[self.cursor_row][col] = Cell::BLANK;
                        }
                    }
                    2 => {
                        self.rows[self.cursor_row] = blank_row(self.cols);
                    }
                    _ => {}
                }
            }

            b'L' => {
                let n = params.first().copied().unwrap_or(1).max(1) as usize;
                self.ensure_row(self.cursor_row);
                for _ in 0..n {
                    self.rows.insert(self.cursor_row, blank_row(self.cols));
                }
            }

            b'M' => {
                let n = params.first().copied().unwrap_or(1).max(1) as usize;
                for _ in 0..n {
                    if self.cursor_row < self.rows.len() {
                        self.rows.remove(self.cursor_row);
                    }
                }
                if self.rows.is_empty() {
                    self.rows.push(blank_row(self.cols));
                }
            }

            b's' => {
                self.saved_row = self.cursor_row;
                self.saved_col = self.cursor_col;
            }

            b'u' => {
                self.cursor_row = self.saved_row;
                self.cursor_col = self.saved_col;
                self.ensure_row(self.cursor_row);
            }

            _ => {}
        }
    }

    fn sgr(&mut self, params: &[u16]) {
        if params.is_empty() || (params.len() == 1 && params[0] == 0) {
            self.fg = 7;
            self.bg = 0;
            self.bold = false;
            return;
        }

        for &p in params {
            match p {
                0 => {
                    self.fg = 7;
                    self.bg = 0;
                    self.bold = false;
                }
                1 => self.bold = true,
                5 => self.bg |= 8,
                22 => self.bold = false,
                25 => self.bg &= 7,
                30..=37 => self.fg = (p - 30) as u8,
                40..=47 => self.bg = (p - 40) as u8,
                _ => {}
            }
        }
    }
}
