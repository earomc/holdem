//! The symbol picker: a window with every shape and color at once, opened
//! from the *Your symbol* row of the setup and join screens.

use ratatui::crossterm::event::KeyCode;

use holdem::player::achievements;
use holdem::player::symbol::{COLORS, GLYPHS, Symbol};

use super::App;
use crate::keys::{self, Cmd, Place};
use crate::sound::Cue;

/// Shapes per row of the grid.
pub const COLUMNS: usize = 10;

/// The picker while it is open.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picker {
    /// The cell under the cursor: the shapes first (by index into `GLYPHS`),
    /// then the colors.
    pub cursor: usize,
    /// The symbol as it would be taken now.
    pub preview: Symbol,
}

/// Rows of shapes in the grid.
pub fn shape_rows() -> usize {
    GLYPHS.len().div_ceil(COLUMNS)
}

impl Picker {
    /// The shape under the cursor, if it is on one.
    pub fn shape(&self) -> Option<u8> {
        (self.cursor < GLYPHS.len()).then_some(self.cursor as u8)
    }

    /// The color under the cursor, if it is on the color row.
    pub fn color(&self) -> Option<u8> {
        self.cursor.checked_sub(GLYPHS.len()).map(|c| c as u8)
    }

    /// The cursor as (row, column); the colors are the row after the shapes.
    fn place(&self) -> (usize, usize) {
        match self.color() {
            Some(c) => (shape_rows(), usize::from(c)),
            None => (self.cursor / COLUMNS, self.cursor % COLUMNS),
        }
    }

    /// Cells in `row`.
    fn row_len(row: usize) -> usize {
        if row == shape_rows() {
            COLORS.len()
        } else {
            (GLYPHS.len() - row * COLUMNS).min(COLUMNS)
        }
    }

    fn set_place(&mut self, row: usize, col: usize) {
        let col = col.min(Self::row_len(row) - 1);
        self.cursor = if row == shape_rows() {
            GLYPHS.len() + col
        } else {
            row * COLUMNS + col
        };
    }

    /// Moves the cursor by rows and columns, staying inside the grid.
    fn step(&mut self, rows: i32, cols: i32) {
        let (row, col) = self.place();
        let last = shape_rows() as i32;
        let row = (row as i32 + rows).clamp(0, last) as usize;
        let col = (col as i32 + cols).clamp(0, Self::row_len(row) as i32 - 1) as usize;
        self.set_place(row, col);
    }
}

impl App {
    /// Opens the picker on your current symbol.
    pub(super) fn open_picker(&mut self) {
        let preview = self.shown_symbol();
        self.picker = Some(Picker {
            cursor: usize::from(preview.glyph),
            preview,
        });
        self.sound.play(Cue::Tick, 0);
    }

    /// What locks the shape under the cursor, if it is locked: "Locked: Table
    /// champion III unlocks this shape". The shape itself stays hidden.
    pub fn picker_locked_note(&self) -> Option<String> {
        let shape = self.picker?.shape()?;
        if self.available_glyphs().contains(&shape) {
            return None;
        }
        Some(match achievements::unlocked_by(shape) {
            Some(stage) => format!("Locked: {stage} unlocks this shape"),
            None => "Locked".to_string(),
        })
    }

    pub(super) fn picker_key(&mut self, code: KeyCode) {
        let Some(mut p) = self.picker else {
            return;
        };
        let moved = match keys::command(Place::SymbolPicker, code) {
            Some(Cmd::Left) => (0, -1),
            Some(Cmd::Right) => (0, 1),
            Some(Cmd::Up) => (-1, 0),
            Some(Cmd::Down) => (1, 0),
            Some(Cmd::Select) => return self.picker_select(p),
            Some(Cmd::Close) => {
                self.picker = None;
                return;
            }
            _ => return,
        };
        p.step(moved.0, moved.1);
        // Only the color previews as the cursor moves; a shape is picked with Enter.
        if let Some(color) = p.color() {
            p.preview.color = color;
        }
        self.picker = Some(p);
        self.sound.play(Cue::Tick, 0);
    }

    /// Enter: on a shape, picks it and goes on to the colors; on a color,
    /// takes the symbol and closes the window. A locked shape is refused.
    fn picker_select(&mut self, mut p: Picker) {
        if self.picker_locked_note().is_some() {
            self.sound.play(Cue::Deny, 0);
            return;
        }
        if let Some(shape) = p.shape() {
            p.preview.glyph = shape;
            p.cursor = GLYPHS.len() + usize::from(p.preview.color);
            self.picker = Some(p);
            self.sound.play(Cue::Tick, 0);
        } else if let Some(color) = p.color() {
            p.preview.color = color;
            self.symbol = Some(p.preview);
            self.picker = None;
            self.sound.play(Cue::Tick, 0);
        }
    }
}
