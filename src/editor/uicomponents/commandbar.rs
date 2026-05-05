use std::{cmp::min, io::Error};

use crate::prelude::*;

use super::super::{command::Edit, Line, Terminal};
use super::UIComponent;

#[derive(Default)]
pub struct CommandBar {
    prompt: String,
    value: Line,
    caret_pos: GraphemeIdx,
    needs_redraw: bool,
    size: Size,
    history: Vec<String>,
    history_index: Option<usize>,
    current_input: Option<Line>,
}

impl CommandBar {
    pub fn handle_edit_command(&mut self, command: Edit) {
        match command {
            Edit::Insert(character) => {
                self.value.insert_char(character, self.caret_pos);
                self.caret_pos = self.caret_pos.saturating_add(1);
            }
            Edit::Delete => {
                if self.caret_pos < self.value.grapheme_count() {
                    self.value.delete(self.caret_pos);
                }
            }
            Edit::DeleteBackward => {
                if self.caret_pos > 0 {
                    self.caret_pos = self.caret_pos.saturating_sub(1);
                    self.value.delete(self.caret_pos);
                }
            }
            Edit::InsertNewline | Edit::Undo | Edit::Redo => {}
        }
        self.history_index = None;
        self.current_input = None;
        self.set_needs_redraw(true);
    }
    pub fn value(&self) -> String {
        self.value.to_string()
    }
    pub fn caret_position_col(&self) -> ColIdx {
        let col = self
            .prompt
            .len()
            .saturating_add(self.value.width_until(self.caret_pos));
        min(col, self.size.width)
    }
    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt = prompt.to_string();
        self.set_needs_redraw(true);
    }
    pub fn clear_value(&mut self) {
        self.value = Line::default();
        self.caret_pos = 0;
        self.history_index = None;
        self.current_input = None;
        self.set_needs_redraw(true);
    }
    pub fn add_to_history(&mut self, command: String) {
        if !command.is_empty() && self.history.last() != Some(&command) {
            self.history.push(command);
        }
        self.history_index = None;
        self.current_input = None;
    }
    pub fn navigate_history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index.is_none() {
            self.current_input = Some(self.value.clone());
        }
        let new_index = match self.history_index {
            Some(i) => i.saturating_sub(1),
            None => self.history.len().saturating_sub(1),
        };
        self.history_index = Some(new_index);
        self.value = Line::from(&self.history[new_index]);
        self.caret_pos = self.value.grapheme_count();
        self.set_needs_redraw(true);
    }
    pub fn navigate_history_down(&mut self) {
        if let Some(i) = self.history_index {
            if i + 1 < self.history.len() {
                let new_index = i + 1;
                self.history_index = Some(new_index);
                self.value = Line::from(&self.history[new_index]);
            } else {
                self.history_index = None;
                self.value = self.current_input.take().unwrap_or_default();
            }
            self.caret_pos = self.value.grapheme_count();
            self.set_needs_redraw(true);
        }
    }
    pub fn move_caret_left(&mut self) {
        self.caret_pos = self.caret_pos.saturating_sub(1);
        self.set_needs_redraw(true);
    }
    pub fn move_caret_right(&mut self) {
        self.caret_pos = min(
            self.caret_pos.saturating_add(1),
            self.value.grapheme_count(),
        );
        self.set_needs_redraw(true);
    }
}

impl UIComponent for CommandBar {
    fn set_needs_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }
    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }
    fn set_size(&mut self, size: Size) {
        self.size = size;
    }
    fn draw(&mut self, origin: RowIdx) -> Result<(), Error> {
        let area_for_value = self.size.width.saturating_sub(self.prompt.len()); //this is how much space there is between the right side of the prompt and the edge of the bar
        let value_end = self.value.width(); // we always want to show the left part of the value, therefore the end of the visible range we try to access will be equal to the full width
        let value_start = value_end.saturating_sub(area_for_value); //This should give us the start for the grapheme subrange we want to print out.
        let message = format!(
            "{}{}",
            self.prompt,
            self.value.get_visible_graphemes(value_start..value_end)
        );
        let to_print = if message.len() <= self.size.width {
            message
        } else {
            String::new()
        };
        Terminal::print_row(origin, &to_print)
    }
}
