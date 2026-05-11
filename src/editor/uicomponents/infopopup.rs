use super::UIComponent;
use crate::editor::Terminal;
use crate::prelude::*;
use crossterm::style::Color;
use std::io::Error;

pub struct InfoPopup {
    position: Position,
    needs_redraw: bool,
    size: Size,
    lines: Vec<String>,
}

impl InfoPopup {
    pub fn new(mut position: Position, terminal_size: Size, text: &str) -> Self {
        let mut lines = Vec::new();
        let max_width = terminal_size.width.saturating_mul(3).saturating_div(4);

        for line in text.lines() {
            let mut current_line = String::new();
            for word in line.split_whitespace() {
                if current_line.len() + word.len() + 1 > max_width {
                    lines.push(format!(" {current_line} "));
                    current_line = word.to_string();
                } else {
                    if !current_line.is_empty() {
                        current_line.push(' ');
                    }
                    current_line.push_str(word);
                }
            }
            if !current_line.is_empty() {
                lines.push(format!(" {current_line} "));
            }
        }

        let width = lines.iter().map(std::string::String::len).max().unwrap_or(0);
        let height = lines.len();

        let padded_lines: Vec<String> = lines
            .into_iter()
            .map(|l| format!("{l:<width$}"))
            .collect();

        let size = Size { height, width };

        // Adjust position if it would go out of bounds
        // Main editor area ends at terminal_size.height - 2 (for status and message bars)
        let max_height = terminal_size.height.saturating_sub(2);

        if position.col + size.width > terminal_size.width {
            position.col = terminal_size.width.saturating_sub(size.width);
        }
        if position.row + size.height > max_height {
            position.row = max_height.saturating_sub(size.height);
        } else {
            // Prefer showing above the cursor if it fits
            if position.row >= size.height {
                position.row = position.row.saturating_sub(size.height);
            } else {
                position.row = position.row.saturating_add(1);
            }
        }

        Self {
            position,
            needs_redraw: true,
            size,
            lines: padded_lines,
        }
    }
}

impl UIComponent for InfoPopup {
    fn set_needs_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    fn draw(&mut self, _origin_row: RowIdx) -> Result<(), Error> {
        let col = self.position.col;
        let row = self.position.row;

        for (i, line) in self.lines.iter().enumerate() {
            Terminal::set_background_color(Color::DarkGrey)?;
            Terminal::set_foreground_color(Color::White)?;
            Terminal::print_row_at_no_clear(row + i, col, line)?;
            Terminal::reset_attributes()?;
        }

        Ok(())
    }
}
