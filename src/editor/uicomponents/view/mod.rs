use arboard::Clipboard;
use std::ops::Deref;
use std::{cmp::min, io::Error};
use unicode_segmentation::UnicodeSegmentation;

use crate::editor::RowIdx;
use crate::prelude::*;

use super::UIComponent;
use crate::editor::command::{Edit, Move};
use crate::editor::{AnnotationType, DocumentStatus, Line, Terminal};
mod buffer;
use buffer::Buffer;
mod searchdirection;
use searchdirection::SearchDirection;
mod highlighter;
use highlighter::{create_syntax_highlighter, Highlighter, SyntaxHighlighter};
mod fileinfo;
use fileinfo::FileInfo;
mod searchinfo;
use searchinfo::SearchInfo;

pub struct View {
    buffer: Buffer,
    needs_redraw: bool,
    size: Size,
    text_location: Location,
    selection_start: Option<Location>,
    scroll_offset: Position,
    search_info: Option<SearchInfo>,
    last_search_query: Option<Line>,
    syntax_highlighter: Option<Box<dyn SyntaxHighlighter>>,
    syntax_enabled: bool,
    diagnostics: Vec<lsp_types::Diagnostic>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            buffer: Buffer::default(),
            needs_redraw: true,
            size: Size::default(),
            text_location: Location::default(),
            selection_start: None,
            scroll_offset: Position::default(),
            search_info: None,
            last_search_query: None,
            syntax_highlighter: None,
            syntax_enabled: false,
            diagnostics: Vec::new(),
        }
    }
}

impl View {
    pub fn new_with_content(content: &str, name: &str, size: Size) -> Self {
        let mut view = Self::default();
        view.resize(size);
        view.buffer = Buffer::new_with_content(content, name);
        view
    }
    pub fn can_undo(&self) -> bool {
        self.buffer.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.buffer.can_redo()
    }
    pub fn has_selection(&self) -> bool {
        self.selection_start.is_some()
    }
    pub fn can_paste(&self, internal_clipboard: &str, system_clipboard: &mut Clipboard) -> bool {
        if !internal_clipboard.is_empty() {
            return true;
        }
        if let Ok(text) = system_clipboard.get_text() {
            return !text.is_empty();
        }
        false
    }
    pub fn get_status(&self, mode: &str) -> DocumentStatus {
        let file_info = self.buffer.get_file_info();
        DocumentStatus {
            total_lines: self.buffer.height(),
            current_line_idx: self.text_location.line_idx,
            current_col_idx: self.text_location_to_position().col,
            file_name: format!("{file_info}"),
            is_modified: self.buffer.is_dirty(),
            file_type: file_info.get_file_type(),
            mode: mode.to_string(),
        }
    }

    pub fn get_uri(&self) -> String {
        if let Some(path) = self.buffer.get_file_info().get_path() {
            if let Ok(abs_path) = std::fs::canonicalize(path) {
                return format!("file://{}", abs_path.display());
            }
        }
        String::new()
    }

    pub fn get_path(&self) -> Option<&std::path::Path> {
        self.buffer.get_file_info().get_path()
    }

    pub fn get_lsp_position(&self) -> lsp_types::Position {
        lsp_types::Position {
            line: self.text_location.line_idx as u32,
            character: self.text_location.grapheme_idx as u32,
        }
    }

    pub fn update_diagnostics(&mut self, diagnostics: Vec<lsp_types::Diagnostic>) {
        self.diagnostics = diagnostics;
        self.set_needs_redraw(true);
    }

    pub fn reload(&mut self) -> Result<bool, Error> {
        if let Some(path) = self.buffer.get_file_info().get_path() {
            let content = std::fs::read_to_string(path)?;
            let new_lines: Vec<Line> = content.lines().map(Line::from).collect();
            let new_lines = if new_lines.is_empty() {
                vec![Line::default()]
            } else {
                new_lines
            };
            if self.buffer.matches_content(&new_lines) {
                return Ok(false);
            }
            self.buffer.replace_lines(new_lines, self.text_location);
            self.buffer.set_saved();
            self.set_needs_redraw(true);
            Ok(true)
        } else {
            Err(Error::new(std::io::ErrorKind::Other, "No file path"))
        }
    }

    pub fn get_text(&self) -> String {
        self.buffer.as_string()
    }

    pub fn apply_lsp_edits(&mut self, edits: Vec<lsp_types::TextEdit>) {
        // Sort edits in reverse order to not invalidate indices as we apply them
        let mut sorted_edits = edits;
        sorted_edits.sort_by(|a, b| b.range.start.cmp(&a.range.start));

        for edit in sorted_edits {
            let start = Location {
                line_idx: edit.range.start.line as usize,
                grapheme_idx: edit.range.start.character as usize,
                preferred_grapheme_idx: 0,
            };
            let end = Location {
                line_idx: edit.range.end.line as usize,
                grapheme_idx: edit.range.end.character as usize,
                preferred_grapheme_idx: 0,
            };
            self.buffer.delete_range(start, end);
            self.buffer.insert_string(&edit.new_text, start);
        }
        self.set_needs_redraw(true);
    }

    pub fn start_selection(&mut self) {
        self.selection_start = Some(self.text_location);
        self.set_needs_redraw(true);
    }

    pub fn select_line_down(&mut self) {
        self.snap_to_valid_line();
        let height = self.buffer.height();
        if height == 0 {
            return;
        }
        if let Some(start) = self.selection_start {
            if self.text_location >= start {
                if self.text_location.line_idx < height.saturating_sub(1) {
                    self.text_location.line_idx = self.text_location.line_idx.saturating_add(1);
                    self.text_location.grapheme_idx =
                        self.buffer.grapheme_count(self.text_location.line_idx);
                } else {
                    self.text_location.grapheme_idx =
                        self.buffer.grapheme_count(self.text_location.line_idx);
                }
            } else {
                self.text_location.line_idx = min(
                    self.text_location.line_idx.saturating_add(1),
                    height.saturating_sub(1),
                );
                if self.text_location >= start {
                    self.text_location.grapheme_idx =
                        self.buffer.grapheme_count(self.text_location.line_idx);
                } else {
                    self.text_location.grapheme_idx = 0;
                }
            }
        } else {
            self.selection_start = Some(Location {
                line_idx: self.text_location.line_idx,
                grapheme_idx: 0,
                preferred_grapheme_idx: 0,
            });
            self.text_location.grapheme_idx =
                self.buffer.grapheme_count(self.text_location.line_idx);
        }
        self.text_location.preferred_grapheme_idx = self.text_location.grapheme_idx;
        self.scroll_text_location_into_view();
        self.set_needs_redraw(true);
    }

    pub fn select_line_up(&mut self) {
        self.snap_to_valid_line();
        let height = self.buffer.height();
        if height == 0 {
            return;
        }
        if let Some(start) = self.selection_start {
            if self.text_location <= start {
                if self.text_location.line_idx > 0 {
                    self.text_location.line_idx = self.text_location.line_idx.saturating_sub(1);
                    self.text_location.grapheme_idx = 0;
                } else {
                    self.text_location.grapheme_idx = 0;
                }
            } else {
                self.text_location.line_idx = self.text_location.line_idx.saturating_sub(1);
                if self.text_location <= start {
                    self.text_location.grapheme_idx = 0;
                } else {
                    self.text_location.grapheme_idx =
                        self.buffer.grapheme_count(self.text_location.line_idx);
                }
            }
        } else {
            self.selection_start = Some(Location {
                line_idx: self.text_location.line_idx,
                grapheme_idx: self.buffer.grapheme_count(self.text_location.line_idx),
                preferred_grapheme_idx: 0,
            });
            self.text_location.grapheme_idx = 0;
        }
        self.text_location.preferred_grapheme_idx = self.text_location.grapheme_idx;
        self.scroll_text_location_into_view();
        self.set_needs_redraw(true);
    }

    pub fn select_all(&mut self) {
        self.selection_start = Some(Location::default());
        self.text_location.line_idx = self.buffer.height().saturating_sub(1);
        self.text_location.grapheme_idx = self.buffer.grapheme_count(self.text_location.line_idx);
        self.text_location.preferred_grapheme_idx = self.text_location.grapheme_idx;
        self.set_needs_redraw(true);
        self.scroll_text_location_into_view();
    }

    pub fn clear_selection(&mut self) {
        if self.selection_start.is_some() {
            self.selection_start = None;
            self.set_needs_redraw(true);
        }
    }

    pub fn get_selection(&self) -> Option<(Location, Location)> {
        self.selection_start
            .map(|start| (start, self.text_location))
    }

    pub fn get_selected_text(&self) -> Option<String> {
        self.get_selection().map(|(mut start, mut end)| {
            if start > end {
                std::mem::swap(&mut start, &mut end);
            }
            self.buffer.get_range(start, end)
        })
    }

    pub fn get_current_character(&self) -> String {
        self.buffer
            .get_range(self.text_location, self.text_location)
    }

    pub fn delete_selection(&mut self) {
        if let Some((mut start, mut end)) = self.get_selection() {
            if start > end {
                std::mem::swap(&mut start, &mut end);
            }
            let end = self.next_location(end);
            self.buffer.delete_range(start, end);
            self.text_location = start;
            self.selection_start = None;
            self.set_needs_redraw(true);
        }
    }

    pub fn paste(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let (start, end) = self
            .get_selection()
            .unwrap_or((self.text_location, self.text_location));
        let mut at = if start <= end { end } else { start };
        self.clear_selection();

        // Paste after: move one grapheme forward
        let line_len = self.buffer.grapheme_count(at.line_idx);
        if at.grapheme_idx < line_len {
            at.grapheme_idx += 1;
        } else {
            at.line_idx = at.line_idx.saturating_add(1);
            at.grapheme_idx = 0;
        }

        self.buffer.insert_string(text, at);
        self.jump_to_end_of_pasted_text(at, text);
        self.set_needs_redraw(true);
    }

    pub fn paste_backward(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let (start, end) = self
            .get_selection()
            .unwrap_or((self.text_location, self.text_location));
        let at = if start <= end { start } else { end };
        self.clear_selection();

        self.buffer.insert_string(text, at);
        self.jump_to_end_of_pasted_text(at, text);
        self.set_needs_redraw(true);
    }

    fn jump_to_end_of_pasted_text(&mut self, start: Location, text: &str) {
        let lines: Vec<&str> = text.split('\n').collect();
        let line_count = lines.len();
        let last_line_graphemes = lines.last().unwrap_or(&"").grapheme_indices(true).count();

        self.text_location.line_idx = start.line_idx.saturating_add(line_count.saturating_sub(1));
        if line_count > 1 {
            self.text_location.grapheme_idx = last_line_graphemes.saturating_sub(1);
        } else {
            self.text_location.grapheme_idx = start
                .grapheme_idx
                .saturating_add(last_line_graphemes)
                .saturating_sub(1);
        }
        self.text_location.preferred_grapheme_idx = self.text_location.grapheme_idx;
        self.scroll_text_location_into_view();
    }

    pub fn concat_lines(&mut self) {
        let mut to = self.text_location;
        to.line_idx += 1;
        self.buffer.concat_range(self.text_location, to);
        self.set_needs_redraw(true);
    }

    pub fn toggle_syntax(&mut self) {
        self.syntax_enabled = !self.syntax_enabled;
        self.set_needs_redraw(true);
    }

    pub fn is_file_loaded(&self) -> bool {
        self.buffer.is_file_loaded()
    }

    fn update_syntax_highlighter(&mut self) {
        if let Some(highlighter) = &mut self.syntax_highlighter {
            highlighter.update(&self.buffer.as_string());
        }
    }

    // region: search
    pub fn enter_search(&mut self) {
        self.search_info = Some(SearchInfo {
            prev_location: self.text_location,
            prev_scroll_offset: self.scroll_offset,
            query: None,
        });
        self.set_needs_redraw(true);
    }
    pub fn exit_search(&mut self) {
        if let Some(search_info) = &self.search_info {
            if let Some(query) = &search_info.query {
                self.last_search_query = Some(query.clone());
            }
        }
        self.search_info = None;
        self.set_needs_redraw(true);
    }
    pub fn clear_search_query(&mut self) {
        self.last_search_query = None;
        self.set_needs_redraw(true);
    }
    pub fn dismiss_search(&mut self) {
        if let Some(search_info) = &mut self.search_info {
            self.text_location = search_info.prev_location;
            self.scroll_offset = search_info.prev_scroll_offset;
            search_info.query = None;
            self.scroll_text_location_into_view(); // ensure the previous location is still visible even if the terminal has been resized during search.
        }
        self.exit_search();
    }

    pub fn search(&mut self, query: &str) {
        if let Some(search_info) = &mut self.search_info {
            search_info.query = Some(Line::from(query));
        }
        self.search_in_direction(self.text_location, SearchDirection::default());
    }

    // Attempts to get the current search query - for scenarios where the search query absolutely must be there.
    // Panics if not present in debug, or if search info is not present in debug
    // Returns None on release.
    pub fn get_search_query(&self) -> Option<&Line> {
        self.search_info
            .as_ref()
            .and_then(|search_info| search_info.query.as_ref())
            .or(self.last_search_query.as_ref())
    }

    fn search_in_direction(&mut self, from: Location, direction: SearchDirection) -> bool {
        if let Some(location) = self.get_search_query().and_then(|query| {
            if query.is_empty() {
                None
            } else if direction == SearchDirection::Forward {
                self.buffer.search_forward(query, from)
            } else {
                self.buffer.search_backward(query, from)
            }
        }) {
            self.text_location = location;
            self.center_text_location();
            self.set_needs_redraw(true);
            return true;
        }
        self.set_needs_redraw(true);
        false
    }
    pub fn search_next(&mut self) -> bool {
        let step_right = self
            .get_search_query()
            .map_or(1, |query| min(query.grapheme_count(), 1));

        let location = Location {
            line_idx: self.text_location.line_idx,
            grapheme_idx: self.text_location.grapheme_idx.saturating_add(step_right), //Start the new search behind the current match
            preferred_grapheme_idx: self.text_location.grapheme_idx.saturating_add(step_right),
        };
        self.search_in_direction(location, SearchDirection::Forward)
    }
    pub fn search_prev(&mut self) -> bool {
        self.search_in_direction(self.text_location, SearchDirection::Backward)
    }
    // endregion

    // region: file i/o
    pub fn load(&mut self, file_name: &str) -> Result<(), Error> {
        let buffer = Buffer::load(file_name)?;
        self.buffer = buffer;
        self.syntax_highlighter =
            create_syntax_highlighter(self.buffer.get_file_info().get_file_type());
        self.update_syntax_highlighter();
        self.set_needs_redraw(true);
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), Error> {
        self.buffer.save()?;
        self.set_needs_redraw(true);
        Ok(())
    }
    pub fn save_as(&mut self, file_name: &str) -> Result<(), Error> {
        self.buffer.save_as(file_name)?;
        self.syntax_highlighter =
            create_syntax_highlighter(self.buffer.get_file_info().get_file_type());
        self.update_syntax_highlighter();
        self.set_needs_redraw(true);
        Ok(())
    }

    // endregion

    // region: command handling
    pub fn handle_edit_command(&mut self, command: Edit) {
        match command {
            Edit::Insert(character) => self.insert_char(character),
            Edit::Complete => self.insert_char('\t'),
            Edit::Replace(character) => self.replace_char(character),
            Edit::Delete => self.delete(),
            Edit::DeleteBackward => self.delete_backward(),
            Edit::InsertNewline => self.insert_newline(),
            Edit::Undo => {
                if let Some(location) = self.buffer.undo(self.text_location) {
                    self.text_location = location;
                    self.snap_to_valid_line();
                    self.snap_to_valid_grapheme();
                    self.scroll_text_location_into_view();
                    self.set_needs_redraw(true);
                }
            }
            Edit::Redo => {
                if let Some(location) = self.buffer.redo(self.text_location) {
                    self.text_location = location;
                    self.snap_to_valid_line();
                    self.snap_to_valid_grapheme();
                    self.scroll_text_location_into_view();
                    self.set_needs_redraw(true);
                }
            }
        }
        self.update_syntax_highlighter();
    }
    pub fn handle_move_command(&mut self, command: Move) {
        let Size { height, .. } = self.size;
        let old_location = self.text_location;
        // This match moves the positon, but does not check for all boundaries.
        // The final boundarline checking happens after the match statement.
        match command {
            Move::Up(step) => self.move_up(step),
            Move::Down(step) => self.move_down(step),
            Move::Left(step) => self.move_left(step),
            Move::Right(step) => self.move_right(step),
            Move::HalfPageUp => self.move_up(height.saturating_div(2)),
            Move::HalfPageDown => self.move_down(height.saturating_div(2)),
            Move::PageUp => self.move_up(height.saturating_sub(1)),
            Move::PageDown => self.move_down(height.saturating_sub(1)),
            Move::ViewTop => self.move_to_view_top(),
            Move::ViewBottom => self.move_to_view_bottom(),
            Move::ViewCenter => self.move_to_view_center(),
            Move::StartOfLine => self.move_to_start_of_line(),
            Move::FirstNonWhitespace => self.move_to_first_non_whitespace(),
            Move::EndOfLine => self.move_to_end_of_line(),
            Move::AfterEndOfLine => self.move_to_after_end_of_line(),
            Move::WordForward => self.move_word_forward(),
            Move::WordBackward => self.move_word_backward(),
            Move::WordEnd => self.move_word_end(),
            Move::BufferStart => self.move_to_buffer_start(),
            Move::BufferEnd => self.move_to_buffer_end(),
            Move::GoToLine(line_idx) => self.move_to_line(line_idx),
            Move::JumpBackward | Move::JumpForward => {}
        }
        if old_location.line_idx != self.text_location.line_idx
            || (self.selection_start.is_some()
                && old_location.grapheme_idx != self.text_location.grapheme_idx)
        {
            self.set_needs_redraw(true);
        }
        self.scroll_text_location_into_view();
    }

    pub fn text_location(&self) -> Location {
        self.text_location
    }

    pub fn set_text_location(&mut self, location: Location) {
        self.text_location = location;
        self.snap_to_valid_line();
        self.snap_to_valid_grapheme();
        self.scroll_text_location_into_view();
        self.set_needs_redraw(true);
    }

    pub fn set_lsp_location(&mut self, pos: lsp_types::Position) {
        self.text_location.line_idx = pos.line as usize;
        self.snap_to_valid_line();
        if let Some(line) = self.buffer.get_line(self.text_location.line_idx) {
            self.text_location.grapheme_idx = line.utf16_code_unit_to_grapheme_idx(pos.character as usize);
        } else {
            self.text_location.grapheme_idx = 0;
        }
        self.text_location.preferred_grapheme_idx = self.text_location.grapheme_idx;
        self.scroll_text_location_into_view();
        self.set_needs_redraw(true);
    }

    pub fn move_to_position(&mut self, position: Position) {
        let gutter_width = self.gutter_width();
        let col = position
            .col
            .saturating_sub(gutter_width)
            .saturating_add(self.scroll_offset.col);
        let row = position.row.saturating_add(self.scroll_offset.row);
        let old_location = self.text_location;
        self.text_location.line_idx = row;
        self.snap_to_valid_line();
        self.text_location.grapheme_idx = self
            .buffer
            .grapheme_at_width(self.text_location.line_idx, col);
        self.text_location.preferred_grapheme_idx = self.text_location.grapheme_idx;
        self.snap_to_valid_grapheme();
        if old_location.line_idx != self.text_location.line_idx
            || (self.selection_start.is_some()
                && old_location.grapheme_idx != self.text_location.grapheme_idx)
        {
            self.set_needs_redraw(true);
        }
        self.scroll_text_location_into_view();
    }

    // endregion
    // region: Text editing
    fn insert_newline(&mut self) {
        let indent_len = self.buffer.insert_enter(self.text_location);
        self.text_location.line_idx = self.text_location.line_idx.saturating_add(1);
        self.text_location.grapheme_idx = indent_len;
        self.text_location.preferred_grapheme_idx = indent_len;
        self.scroll_text_location_into_view();
        self.set_needs_redraw(true);
    }
    fn delete_backward(&mut self) {
        if self.text_location.line_idx != 0 || self.text_location.grapheme_idx != 0 {
            self.handle_move_command(Move::Left(1));
            self.delete();
        }
    }
    fn delete(&mut self) {
        let char_at_cursor = self.get_current_character();
        let next_location = self.next_location(self.text_location);
        let char_after_cursor = self.buffer.get_range(next_location, next_location);

        match (char_at_cursor.as_str(), char_after_cursor.as_str()) {
            ("(", ")") | ("[", "]") | ("{", "}") | ("\"", "\"") | ("'", "'") | ("`", "`") => {
                let end = self.next_location(next_location);
                self.buffer.delete_range(self.text_location, end);
            }
            _ => self.buffer.delete(self.text_location),
        }
        self.set_needs_redraw(true);
    }
    fn replace_char(&mut self, character: char) {
        self.buffer.replace_char(character, self.text_location);
        self.set_needs_redraw(true);
    }
    pub fn handle_replace_mode_char(&mut self, character: char) {
        if self.text_location.grapheme_idx < self.buffer.grapheme_count(self.text_location.line_idx) {
            self.buffer.replace_char(character, self.text_location);
        } else {
            self.buffer.insert_char(character, self.text_location);
        }
        self.handle_move_command(Move::Right(1));
        self.set_needs_redraw(true);
    }
    fn insert_char(&mut self, character: char) {
        // If typing a closing character that is already there, just move past it
        if matches!(character, ')' | ']' | '}' | '"' | '\'' | '`') {
            let next_char = self.get_current_character();
            if next_char == character.to_string() {
                if character == '}' || character == ']' || character == ')' {
                    self.auto_deindent();
                }
                self.handle_move_command(Move::Right(1));
                return;
            }
        }
        let closing_char = match character {
            '(' => Some(')'),
            '[' => Some(']'),
            '{' => Some('}'),
            '"' => Some('"'),
            '\'' => Some('\''),
            '`' => Some('`'),
            _ => None,
        };
        if let Some(close) = closing_char {
            self.buffer.insert_char(character, self.text_location);
            self.handle_move_command(Move::Right(1));
            self.buffer.insert_char(close, self.text_location);
            self.set_needs_redraw(true);
            return;
        }

        if character == '}' || character == ']' || character == ')' {
            self.auto_deindent();
        }
        let old_len = self.buffer.grapheme_count(self.text_location.line_idx);
        self.buffer.insert_char(character, self.text_location);
        let new_len = self.buffer.grapheme_count(self.text_location.line_idx);
        let grapheme_delta = new_len.saturating_sub(old_len);
        if grapheme_delta > 0 {
            //move right for an added grapheme (should be the regular case)
            self.handle_move_command(Move::Right(1));
        }
        self.set_needs_redraw(true);
    }

    fn auto_deindent(&mut self) {
        let indent_size = self.buffer.indent_size();
        let non_whitespace = self
            .buffer
            .first_non_whitespace_grapheme(self.text_location.line_idx);
        if non_whitespace == self.text_location.grapheme_idx && non_whitespace >= indent_size {
            let start = Location {
                line_idx: self.text_location.line_idx,
                grapheme_idx: 0,
                preferred_grapheme_idx: 0,
            };
            let end = Location {
                line_idx: self.text_location.line_idx,
                grapheme_idx: indent_size,
                preferred_grapheme_idx: 0,
            };
            self.buffer.delete_range(start, end);
            self.text_location.grapheme_idx =
                self.text_location.grapheme_idx.saturating_sub(indent_size);
            self.text_location.preferred_grapheme_idx = self
                .text_location
                .preferred_grapheme_idx
                .saturating_sub(indent_size);
        }
    }
    // endregion

    // region: Rendering

    pub fn gutter_width(&self) -> usize {
        let total_lines = self.buffer.height();
        total_lines.to_string().len().max(3).saturating_add(1)
    }
    // endregion

    // region: Scrolling

    fn scroll_vertically(&mut self, to: RowIdx) {
        let Size { height, .. } = self.size;
        let offset_changed = if to < self.scroll_offset.row {
            self.scroll_offset.row = to;
            true
        } else if to >= self.scroll_offset.row.saturating_add(height) {
            self.scroll_offset.row = to.saturating_sub(height).saturating_add(1);
            true
        } else {
            false
        };
        if offset_changed {
            self.set_needs_redraw(true);
        }
    }
    fn scroll_horizontally(&mut self, to: ColIdx) {
        let gutter_width = self.gutter_width();
        let Size { width, .. } = self.size;
        let usable_width = width.saturating_sub(gutter_width);
        if usable_width == 0 {
            return;
        }
        let offset_changed = if to < self.scroll_offset.col {
            self.scroll_offset.col = to;
            true
        } else if to >= self.scroll_offset.col.saturating_add(usable_width) {
            self.scroll_offset.col = to.saturating_sub(usable_width).saturating_add(1);
            true
        } else {
            false
        };
        if offset_changed {
            self.set_needs_redraw(true);
        }
    }
    fn scroll_text_location_into_view(&mut self) {
        let Position { row, col } = self.text_location_to_position();
        self.scroll_vertically(row);
        self.scroll_horizontally(col);
    }
    fn center_text_location(&mut self) {
        let Size { height, width } = self.size;
        let gutter_width = self.gutter_width();
        let usable_width = width.saturating_sub(gutter_width);
        let Position { row, col } = self.text_location_to_position();
        let vertical_mid = height.div_ceil(2);
        let horizontal_mid = usable_width.div_ceil(2);
        self.scroll_offset.row = row.saturating_sub(vertical_mid);
        self.scroll_offset.col = col.saturating_sub(horizontal_mid);
        self.set_needs_redraw(true);
    }
    // endregion

    // region: Location and Position Handling

    pub fn caret_position(&self) -> Position {
        let mut position = self
            .text_location_to_position()
            .saturating_sub(self.scroll_offset);
        position.col = position.col.saturating_add(self.gutter_width());
        position
    }

    fn text_location_to_position(&self) -> Position {
        let row = self.text_location.line_idx;
        debug_assert!(row.saturating_sub(1) <= self.buffer.height());
        let col = self
            .buffer
            .width_until(row, self.text_location.grapheme_idx);
        Position { col, row }
    }

    fn next_location(&self, location: Location) -> Location {
        let mut next = location;
        let grapheme_count = self.buffer.grapheme_count(location.line_idx);
        if location.grapheme_idx < grapheme_count {
            next.grapheme_idx += 1;
        } else if location.line_idx.saturating_add(1) < self.buffer.height() {
            next.line_idx += 1;
            next.grapheme_idx = 0;
        }
        next
    }

    fn prev_location(&self, location: Location) -> Location {
        let mut prev = location;
        if location.grapheme_idx > 0 {
            prev.grapheme_idx -= 1;
        } else if location.line_idx > 0 {
            prev.line_idx -= 1;
            prev.grapheme_idx = self.buffer.grapheme_count(prev.line_idx);
        }
        prev
    }

    // endregion

    // region: text location movement

    fn move_up(&mut self, step: usize) {
        self.text_location.line_idx = self.text_location.line_idx.saturating_sub(step);
        self.snap_to_valid_line();
        self.snap_to_valid_grapheme();
    }
    fn move_down(&mut self, step: usize) {
        self.text_location.line_idx = self.text_location.line_idx.saturating_add(step);
        self.snap_to_valid_line();
        self.snap_to_valid_grapheme();
    }
    // clippy::arithmetic_side_effects: This function performs arithmetic calculations
    // after explicitly checking that the target value will be within bounds.
    #[allow(clippy::arithmetic_side_effects)]
    fn move_right(&mut self, step: usize) {
        for _ in 0..step {
            let grapheme_count = self.buffer.grapheme_count(self.text_location.line_idx);
            if self.text_location.grapheme_idx < grapheme_count {
                self.text_location.grapheme_idx += 1;
            } else {
                self.move_to_start_of_line();
                self.move_down(1);
            }
        }
        self.text_location.preferred_grapheme_idx = self.text_location.grapheme_idx;
    }
    // clippy::arithmetic_side_effects: This function performs arithmetic calculations
    // after explicitly checking that the target value will be within bounds.
    #[allow(clippy::arithmetic_side_effects)]
    fn move_left(&mut self, step: usize) {
        for _ in 0..step {
            if self.text_location.grapheme_idx > 0 {
                self.text_location.grapheme_idx -= 1;
            } else if self.text_location.line_idx > 0 {
                self.move_up(1);
                self.move_to_after_end_of_line();
            }
        }
        self.text_location.preferred_grapheme_idx = self.text_location.grapheme_idx;
    }
    fn move_to_view_top(&mut self) {
        self.text_location.line_idx = self.scroll_offset.row;
        self.snap_to_valid_line();
        self.snap_to_valid_grapheme();
    }
    fn move_to_view_bottom(&mut self) {
        self.text_location.line_idx = self
            .scroll_offset
            .row
            .saturating_add(self.size.height)
            .saturating_sub(1);
        self.snap_to_valid_line();
        self.snap_to_valid_grapheme();
    }
    fn move_to_view_center(&mut self) {
        self.text_location.line_idx = self
            .scroll_offset
            .row
            .saturating_add(self.size.height.saturating_div(2));
        self.snap_to_valid_line();
        self.snap_to_valid_grapheme();
    }
    fn move_to_start_of_line(&mut self) {
        self.text_location.grapheme_idx = 0;
        self.text_location.preferred_grapheme_idx = 0;
    }
    fn move_to_first_non_whitespace(&mut self) {
        self.text_location.grapheme_idx = self
            .buffer
            .first_non_whitespace_grapheme(self.text_location.line_idx);
        self.text_location.preferred_grapheme_idx = self.text_location.grapheme_idx;
    }
    fn move_to_end_of_line(&mut self) {
        self.text_location.grapheme_idx = self
            .buffer
            .grapheme_count(self.text_location.line_idx)
            .saturating_sub(1);
        self.text_location.preferred_grapheme_idx = self.text_location.grapheme_idx;
    }
    fn move_to_after_end_of_line(&mut self) {
        self.text_location.grapheme_idx = self.buffer.grapheme_count(self.text_location.line_idx);
        self.text_location.preferred_grapheme_idx = self.text_location.grapheme_idx;
    }
    fn move_to_buffer_start(&mut self) {
        self.text_location.line_idx = 0;
        self.text_location.grapheme_idx = 0;
        self.text_location.preferred_grapheme_idx = 0;
    }
    fn move_to_buffer_end(&mut self) {
        self.text_location.line_idx = self.buffer.height().saturating_sub(1);
        self.snap_to_valid_line();
        self.move_to_end_of_line();
    }
    fn move_to_line(&mut self, line_idx: usize) {
        self.text_location.line_idx = line_idx.saturating_sub(1);
        self.snap_to_valid_line();
        self.move_to_start_of_line();
    }

    fn move_word_forward(&mut self) {
        let mut loc = self.text_location;

        // Move at least once to avoid getting stuck if already at a target position
        let next = self.next_location(loc);
        if next == loc {
            return;
        }
        loc = next;

        loop {
            let next = self.next_location(loc);
            if next == loc {
                // End of buffer
                self.text_location = loc;
                return;
            }

            let next_is_word = self.is_word_char(next);
            let next_is_punc = self.is_punc_char(next);

            if next_is_word || next_is_punc {
                let curr_is_word = self.is_word_char(loc);
                let curr_is_punc = self.is_punc_char(loc);

                // If next is the start of a new group (word or punctuation),
                // then current (loc) is the character before it.
                if next_is_word && !curr_is_word {
                    self.text_location = loc;
                    return;
                }
                if next_is_punc && !curr_is_punc {
                    self.text_location = loc;
                    return;
                }
            }
            loc = next;
        }
    }

    fn move_word_backward(&mut self) {
        let mut loc = self.text_location;

        // Move at least once to avoid getting stuck
        let prev = self.prev_location(loc);
        if prev == loc {
            return;
        }
        loc = prev;

        loop {
            let prev = self.prev_location(loc);
            if prev == loc {
                // Start of buffer
                self.text_location = loc;
                return;
            }

            let curr_is_word = self.is_word_char(loc);
            let curr_is_punc = self.is_punc_char(loc);

            if curr_is_word || curr_is_punc {
                let prev_is_word = self.is_word_char(prev);
                let prev_is_punc = self.is_punc_char(prev);

                // If curr is the start of a group (word or punctuation) going backward,
                // we land ON it.
                if curr_is_word && !prev_is_word {
                    self.text_location = loc;
                    return;
                }
                if curr_is_punc && !prev_is_punc {
                    self.text_location = loc;
                    return;
                }
            }
            loc = prev;
        }
    }

    pub fn is_word_start(&self, loc: Location) -> bool {
        self.is_word_char(loc) && !self.is_word_char(self.prev_location(loc))
    }
    pub fn is_word_end(&self, loc: Location) -> bool {
        self.is_word_char(loc) && !self.is_word_char(self.next_location(loc))
    }
    pub fn is_punc_start(&self, loc: Location) -> bool {
        self.is_punc_char(loc) && !self.is_punc_char(self.prev_location(loc))
    }
    pub fn is_punc_end(&self, loc: Location) -> bool {
        self.is_punc_char(loc) && !self.is_punc_char(self.next_location(loc))
    }

    fn move_word_end(&mut self) {
        loop {
            let line = if let Some(line) = self.buffer.get_line(self.text_location.line_idx) {
                line
            } else {
                return;
            };

            let grapheme_count = line.grapheme_count();
            let start_idx = if self.text_location.grapheme_idx < grapheme_count {
                self.text_location.grapheme_idx.saturating_add(1)
            } else {
                grapheme_count
            };

            for i in start_idx..grapheme_count {
                let loc = Location {
                    line_idx: self.text_location.line_idx,
                    grapheme_idx: i,
                    preferred_grapheme_idx: i,
                };
                if self.is_word_char(loc) {
                    // Found a word char, now find its end
                    for j in i..grapheme_count {
                        let end_loc = Location {
                            line_idx: self.text_location.line_idx,
                            grapheme_idx: j,
                            preferred_grapheme_idx: j,
                        };
                        let next_loc = Location {
                            line_idx: self.text_location.line_idx,
                            grapheme_idx: j.saturating_add(1),
                            preferred_grapheme_idx: j.saturating_add(1),
                        };
                        if !self.is_word_char(next_loc) {
                            self.text_location = end_loc;
                            return;
                        }
                    }
                }
            }

            if self.text_location.line_idx.saturating_add(1) < self.buffer.height() {
                self.text_location.line_idx += 1;
                self.text_location.grapheme_idx = 0;
            } else {
                self.move_to_end_of_line();
                return;
            }
        }
    }

    fn is_word_char(&self, loc: Location) -> bool {
        if let Some(line) = self.buffer.get_line(loc.line_idx) {
            if let Some(g) = line.get_substring(loc.grapheme_idx..loc.grapheme_idx.saturating_add(1)).chars().next() {
                return g.is_alphanumeric() || g == '_';
            }
        }
        false
    }

    fn is_punc_char(&self, loc: Location) -> bool {
        if let Some(line) = self.buffer.get_line(loc.line_idx) {
            if let Some(g) = line.get_substring(loc.grapheme_idx..loc.grapheme_idx.saturating_add(1)).chars().next() {
                return !g.is_alphanumeric() && !g.is_whitespace() && g != '_';
            }
        }
        false
    }

    // Ensures self.location.grapheme_idx points to a valid grapheme index by snapping it to the left most grapheme if appropriate.
    // Doesn't trigger scrolling.
    fn snap_to_valid_grapheme(&mut self) {
        self.text_location.grapheme_idx = min(
            self.text_location.preferred_grapheme_idx,
            self.buffer.grapheme_count(self.text_location.line_idx),
        );
    }
    // Ensures self.location.line_idx points to a valid line index by snapping it to the bottom most line if appropriate.
    // Doesn't trigger scrolling.
    fn snap_to_valid_line(&mut self) {
        self.text_location.line_idx = min(
            self.text_location.line_idx,
            self.buffer.height().saturating_sub(1),
        );
    }

    // endregion
}

impl UIComponent for View {
    fn set_needs_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }
    fn set_size(&mut self, size: Size) {
        self.size = size;
        self.scroll_text_location_into_view();
    }

    fn draw(&mut self, origin_row: RowIdx) -> Result<(), Error> {
        let Size { height, width } = self.size;
        let gutter_width = self.gutter_width();
        let usable_width = width.saturating_sub(gutter_width);
        let end_y = origin_row.saturating_add(height);
        let scroll_top = self.scroll_offset.row;
        let query = self.get_search_query().cloned();
        let query_deref = query.as_ref().map(|line| line.deref());
        let selected_match = query.is_some().then_some(self.text_location);
        let selection = self
            .selection_start
            .map(|start| (start, self.text_location));
        let syntax_highlighter = if self.syntax_enabled {
            self.syntax_highlighter.as_deref_mut()
        } else {
            None
        };
        let mut highlighter = Highlighter::new(
            query_deref,
            selected_match,
            selection,
            Some(self.diagnostics.clone()),
            syntax_highlighter,
        );

        for current_row in origin_row..end_y {
            // to get the correct line index, we have to take current_row (the absolute row on screen),
            // subtract origin_row to get the current row relative to the view (ranging from 0 to self.size.height)
            // and add the scroll offset.
            let line_idx = current_row
                .saturating_sub(origin_row)
                .saturating_add(scroll_top);

            Terminal::move_caret_to(Position {
                row: current_row,
                col: 0,
            })?;
            if line_idx < self.buffer.height() {
                self.buffer.highlight(line_idx, &mut highlighter);
                let is_current = line_idx == self.text_location.line_idx;
                let label = if is_current {
                    format!(
                        "{:>width$}",
                        line_idx.saturating_add(1),
                        width = gutter_width.saturating_sub(1)
                    )
                } else {
                    format!(
                        "{:>width$}",
                        line_idx.abs_diff(self.text_location.line_idx),
                        width = gutter_width.saturating_sub(1)
                    )
                };
                if is_current {
                    Terminal::set_foreground_color(crossterm::style::Color::Yellow)?;
                } else {
                    Terminal::set_foreground_color(crossterm::style::Color::Grey)?;
                }
                Terminal::print(&label)?;
                Terminal::reset_attributes()?;
                Terminal::print(" ")?;
            } else {
                Terminal::print(&format!(
                    "{:>width$}",
                    "~",
                    width = gutter_width.saturating_sub(1)
                ))?;
                Terminal::print(" ")?;
            }

            let left = self.scroll_offset.col;
            let right = self.scroll_offset.col.saturating_add(usable_width);
            if let Some(annotated_string) =
                self.buffer
                    .get_highlighted_substring(line_idx, left..right, &highlighter)
            {
                let diagnostic = self
                    .diagnostics
                    .iter()
                    .find(|d| d.range.start.line as usize == line_idx)
                    .map(|d| {
                        let msg = d.message.lines().next().unwrap_or("");
                        let annotation_type = match d.severity {
                            Some(lsp_types::DiagnosticSeverity::ERROR) => AnnotationType::Error,
                            Some(lsp_types::DiagnosticSeverity::WARNING) => {
                                AnnotationType::Warning
                            }
                            Some(lsp_types::DiagnosticSeverity::INFORMATION) => {
                                AnnotationType::Information
                            }
                            Some(lsp_types::DiagnosticSeverity::HINT) => AnnotationType::Hint,
                            _ => AnnotationType::Error,
                        };
                        (msg, annotation_type)
                    });
                Terminal::print_annotated_row_at(
                    current_row,
                    gutter_width,
                    &annotated_string,
                    diagnostic,
                )?;
            } else {
                Terminal::print_row_at(current_row, gutter_width, "")?;
            }
        }
        Ok(())
    }
}
