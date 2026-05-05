use std::{cmp::min, io::Error};

use crate::editor::RowIdx;
use crate::prelude::*;

use crate::editor::{DocumentStatus, Line, Terminal};
use crate::editor::command::{Edit, Move};
use super::UIComponent;
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

#[derive(Default)]
pub struct View {
    buffer: Buffer,
    needs_redraw: bool,
    size: Size,
    text_location: Location,
    scroll_offset: Position,
    search_info: Option<SearchInfo>,
    syntax_highlighter: Option<Box<dyn SyntaxHighlighter>>,
}

impl View {
    pub fn get_status(&self, mode: String) -> DocumentStatus {
        let file_info = self.buffer.get_file_info();
        DocumentStatus {
            total_lines: self.buffer.height(),
            current_line_idx: self.text_location.line_idx,
            file_name: format!("{file_info}"),
            is_modified: self.buffer.is_dirty(),
            file_type: file_info.get_file_type(),
            mode,
        }
    }

    pub const fn is_file_loaded(&self) -> bool {
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
    }
    pub fn exit_search(&mut self) {
        self.search_info = None;
        self.set_needs_redraw(true);
    }
    pub fn dismiss_search(&mut self) {
        if let Some(search_info) = &self.search_info {
            self.text_location = search_info.prev_location;
            self.scroll_offset = search_info.prev_scroll_offset;
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
    fn get_search_query(&self) -> Option<&Line> {
        let query = self
            .search_info
            .as_ref()
            .and_then(|search_info| search_info.query.as_ref());

        debug_assert!(
            query.is_some(),
            "Attempting to search with malformed searchinfo present"
        );
        query
    }

    fn search_in_direction(&mut self, from: Location, direction: SearchDirection) {
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
        }
        self.set_needs_redraw(true);
    }
    pub fn search_next(&mut self) {
        let step_right = self
            .get_search_query()
            .map_or(1, |query| min(query.grapheme_count(), 1));

        let location = Location {
            line_idx: self.text_location.line_idx,
            grapheme_idx: self.text_location.grapheme_idx.saturating_add(step_right), //Start the new search behind the current match
        };
        self.search_in_direction(location, SearchDirection::Forward);
    }
    pub fn search_prev(&mut self) {
        self.search_in_direction(self.text_location, SearchDirection::Backward);
    }
    // endregion

    // region: file i/o
    pub fn load(&mut self, file_name: &str) -> Result<(), Error> {
        let buffer = Buffer::load(file_name)?;
        self.buffer = buffer;
        self.syntax_highlighter = create_syntax_highlighter(self.buffer.get_file_info().get_file_type());
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
        self.syntax_highlighter = create_syntax_highlighter(self.buffer.get_file_info().get_file_type());
        self.update_syntax_highlighter();
        self.set_needs_redraw(true);
        Ok(())
    }

    // endregion

    // region: command handling
    pub fn handle_edit_command(&mut self, command: Edit) {
        match command {
            Edit::Insert(character) => self.insert_char(character),
            Edit::Delete => self.delete(),
            Edit::DeleteBackward => self.delete_backward(),
            Edit::InsertNewline => self.insert_newline(),
        }
        self.update_syntax_highlighter();
    }
    pub fn handle_move_command(&mut self, command: Move) {
        let Size { height, .. } = self.size;
        let old_line_idx = self.text_location.line_idx;
        // This match moves the positon, but does not check for all boundaries.
        // The final boundarline checking happens after the match statement.
        match command {
            Move::Up => self.move_up(1),
            Move::Down => self.move_down(1),
            Move::Left => self.move_left(),
            Move::Right => self.move_right(),
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
            Move::BufferStart => self.move_to_buffer_start(),
            Move::BufferEnd => self.move_to_buffer_end(),
        }
        if old_line_idx != self.text_location.line_idx {
            self.set_needs_redraw(true);
        }
        self.scroll_text_location_into_view();
    }

    pub fn move_to_position(&mut self, position: Position) {
        let gutter_width = self.gutter_width();
        let col = position
            .col
            .saturating_sub(gutter_width)
            .saturating_add(self.scroll_offset.col);
        let row = position.row.saturating_add(self.scroll_offset.row);
        let old_line_idx = self.text_location.line_idx;
        self.text_location.line_idx = row;
        self.snap_to_valid_line();
        if old_line_idx != self.text_location.line_idx {
            self.set_needs_redraw(true);
        }
        self.text_location.grapheme_idx = self.buffer.grapheme_at_width(self.text_location.line_idx, col);
        self.snap_to_valid_grapheme();
        self.scroll_text_location_into_view();
    }

    // endregion
    // region: Text editing
    fn insert_newline(&mut self) {
        self.buffer.insert_newline(self.text_location);
        self.handle_move_command(Move::Right);
        self.set_needs_redraw(true);
    }
    fn delete_backward(&mut self) {
        if self.text_location.line_idx != 0 || self.text_location.grapheme_idx != 0 {
            self.handle_move_command(Move::Left);
            self.delete();
        }
    }
    fn delete(&mut self) {
        self.buffer.delete(self.text_location);
        self.set_needs_redraw(true);
    }
    fn insert_char(&mut self, character: char) {
        let old_len = self.buffer.grapheme_count(self.text_location.line_idx);
        self.buffer.insert_char(character, self.text_location);
        let new_len = self.buffer.grapheme_count(self.text_location.line_idx);
        let grapheme_delta = new_len.saturating_sub(old_len);
        if grapheme_delta > 0 {
            //move right for an added grapheme (should be the regular case)
            self.handle_move_command(Move::Right);
        }
        self.set_needs_redraw(true);
    }
    // endregion

    // region: Rendering

    pub fn gutter_width(&self) -> usize {
        let total_lines = self.buffer.height();
        if total_lines == 0 {
            0
        } else {
            total_lines.to_string().len().max(3).saturating_add(1)
        }
    }

    fn build_welcome_message(width: usize) -> String {
        if width == 0 {
            return String::new();
        }
        let welcome_message = format!("{NAME} editor -- version {VERSION}");
        let len = welcome_message.len();
        let remaining_width = width.saturating_sub(1);
        // hide the welcome message if it doesn't fit entirely.
        if remaining_width < len {
            return "~".to_string();
        }
        format!("{:<1}{:^remaining_width$}", "~", welcome_message)
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

    // endregion

    // region: text location movement

    fn move_up(&mut self, step: usize) {
        self.text_location.line_idx = self.text_location.line_idx.saturating_sub(step);
        self.snap_to_valid_grapheme();
    }
    fn move_down(&mut self, step: usize) {
        self.text_location.line_idx = self.text_location.line_idx.saturating_add(step);
        self.snap_to_valid_grapheme();
        self.snap_to_valid_line();
    }
    // clippy::arithmetic_side_effects: This function performs arithmetic calculations
    // after explicitly checking that the target value will be within bounds.
    #[allow(clippy::arithmetic_side_effects)]
    fn move_right(&mut self) {
        let grapheme_count = self.buffer.grapheme_count(self.text_location.line_idx);
        if self.text_location.grapheme_idx < grapheme_count {
            self.text_location.grapheme_idx += 1;
        } else {
            self.move_to_start_of_line();
            self.move_down(1);
        }
    }
    // clippy::arithmetic_side_effects: This function performs arithmetic calculations
    // after explicitly checking that the target value will be within bounds.
    #[allow(clippy::arithmetic_side_effects)]
    fn move_left(&mut self) {
        if self.text_location.grapheme_idx > 0 {
            self.text_location.grapheme_idx -= 1;
        } else if self.text_location.line_idx > 0 {
            self.move_up(1);
            self.move_to_end_of_line();
        }
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
    }
    fn move_to_first_non_whitespace(&mut self) {
        self.text_location.grapheme_idx = self.buffer.first_non_whitespace_grapheme(self.text_location.line_idx);
    }
    fn move_to_end_of_line(&mut self) {
        self.text_location.grapheme_idx = self.buffer.grapheme_count(self.text_location.line_idx);
    }
    fn move_to_buffer_start(&mut self) {
        self.text_location.line_idx = 0;
        self.text_location.grapheme_idx = 0;
    }
    fn move_to_buffer_end(&mut self) {
        self.text_location.line_idx = self.buffer.height().saturating_sub(1);
        self.snap_to_valid_line();
        self.move_to_end_of_line();
    }

    // Ensures self.location.grapheme_idx points to a valid grapheme index by snapping it to the left most grapheme if appropriate.
    // Doesn't trigger scrolling.
    fn snap_to_valid_grapheme(&mut self) {
        self.text_location.grapheme_idx = min(
            self.text_location.grapheme_idx,
            self.buffer.grapheme_count(self.text_location.line_idx),
        );
    }
    // Ensures self.location.line_idx points to a valid line index by snapping it to the bottom most line if appropriate.
    // Doesn't trigger scrolling.
    fn snap_to_valid_line(&mut self) {
        self.text_location.line_idx = min(self.text_location.line_idx, self.buffer.height());
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
        let top_third = height.div_ceil(3);
        let scroll_top = self.scroll_offset.row;

        let query = self
            .search_info
            .as_ref()
            .and_then(|search_info| search_info.query.as_deref());
        let selected_match = query.is_some().then_some(self.text_location);
        let mut highlighter = Highlighter::new(
            query,
            selected_match,
            self.syntax_highlighter.as_deref_mut(),
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
                Terminal::print_annotated_row_at(current_row, gutter_width, &annotated_string)?;
            } else if current_row == top_third && self.buffer.is_empty() {
                Terminal::print_row_at(
                    current_row,
                    gutter_width,
                    &Self::build_welcome_message(usable_width),
                )?;
            } else {
                Terminal::print_row_at(current_row, gutter_width, "")?;
            }
        }
        Ok(())
    }
}
