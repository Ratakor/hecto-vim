use super::super::super::AnnotatedString;
use super::FileInfo;
use super::Highlighter;
use super::Line;
use crate::prelude::*;
use std::fs::{File, read_to_string};
use std::io::Error;
use std::io::Write;
use std::ops::Range;

pub struct Buffer {
    lines: Vec<Line>,
    file_info: FileInfo,
    undo_stack: Vec<Vec<Line>>,
    redo_stack: Vec<Vec<Line>>,
}

impl Default for Buffer {
    fn default() -> Self {
        Self {
            lines: vec![Line::default()],
            file_info: FileInfo::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}

impl Buffer {
    pub fn is_dirty(&self) -> bool {
        !self.undo_stack.is_empty()
    }
    pub const fn get_file_info(&self) -> &FileInfo {
        &self.file_info
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(self.lines.clone());
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(lines) = self.undo_stack.pop() {
            self.redo_stack.push(self.lines.clone());
            self.lines = lines;
            return true;
        }
        false
    }

    pub fn redo(&mut self) -> bool {
        if let Some(lines) = self.redo_stack.pop() {
            self.undo_stack.push(self.lines.clone());
            self.lines = lines;
            return true;
        }
        false
    }

    pub fn grapheme_count(&self, idx: LineIdx) -> GraphemeIdx {
        self.lines.get(idx).map_or(0, Line::grapheme_count)
    }
    pub fn first_non_whitespace_grapheme(&self, idx: LineIdx) -> GraphemeIdx {
        self.lines
            .get(idx)
            .map_or(0, Line::first_non_whitespace_grapheme)
    }
    pub fn width_until(&self, idx: LineIdx, until: GraphemeIdx) -> GraphemeIdx {
        self.lines
            .get(idx)
            .map_or(0, |line| line.width_until(until))
    }
    pub fn grapheme_at_width(&self, idx: LineIdx, width: ColIdx) -> GraphemeIdx {
        self.lines
            .get(idx)
            .map_or(0, |line| line.grapheme_at_width(width))
    }

    pub fn get_highlighted_substring(
        &self,
        line_idx: LineIdx,
        range: Range<ColIdx>,
        highlighter: &Highlighter,
    ) -> Option<AnnotatedString> {
        self.lines.get(line_idx).map(|line| {
            line.get_annotated_visible_substr(
                range,
                Some(&highlighter.get_annotations(line_idx, line)),
            )
        })
    }

    pub fn highlight(&self, idx: LineIdx, highlighter: &mut Highlighter) {
        if let Some(line) = self.lines.get(idx) {
            highlighter.highlight(idx, line);
        }
    }

    pub fn load(file_name: &str) -> Result<Self, Error> {
        let contents = read_to_string(file_name)?;
        let lines = contents.lines().map(Line::from).collect();
        Ok(Self {
            lines,
            file_info: FileInfo::from(file_name),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn search_forward(&self, query: &str, from: Location) -> Option<Location> {
        if query.is_empty() {
            return None;
        }
        let mut is_first = true;
        for (line_idx, line) in self
            .lines
            .iter()
            .enumerate()
            .cycle()
            .skip(from.line_idx)
            .take(self.lines.len().saturating_add(1))
        //taking one more, to search the current line twice (once from the middle, once from the start)
        {
            let from_grapheme_idx = if is_first {
                is_first = false;
                from.grapheme_idx
            } else {
                0
            };
            if let Some(grapheme_idx) = line.search_forward(query, from_grapheme_idx) {
                return Some(Location {
                    grapheme_idx,
                    line_idx,
                    preferred_grapheme_idx: grapheme_idx,
                });
            }
        }
        None
    }
    pub fn search_backward(&self, query: &str, from: Location) -> Option<Location> {
        if query.is_empty() {
            return None;
        }
        let mut is_first = true;
        for (line_idx, line) in self
            .lines
            .iter()
            .enumerate()
            .rev()
            .cycle()
            .skip(
                self.lines
                    .len()
                    .saturating_sub(from.line_idx)
                    .saturating_sub(1),
            )
            .take(self.lines.len().saturating_add(1))
        {
            let from_grapheme_idx = if is_first {
                is_first = false;
                from.grapheme_idx
            } else {
                line.grapheme_count()
            };
            if let Some(grapheme_idx) = line.search_backward(query, from_grapheme_idx) {
                return Some(Location {
                    grapheme_idx,
                    line_idx,
                    preferred_grapheme_idx: grapheme_idx,
                });
            }
        }
        None
    }

    fn save_to_file(&self, file_info: &FileInfo) -> Result<(), Error> {
        if let Some(file_path) = &file_info.get_path() {
            let mut file = File::create(file_path)?;
            for line in &self.lines {
                writeln!(file, "{line}")?;
            }
        } else {
            #[cfg(debug_assertions)]
            {
                panic!("Attempting to save with no file path present");
            }
        }
        Ok(())
    }
    pub fn save_as(&mut self, file_name: &str) -> Result<(), Error> {
        let file_info = FileInfo::from(file_name);
        self.save_to_file(&file_info)?;
        self.file_info = file_info;
        self.undo_stack.clear();
        self.redo_stack.clear();
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), Error> {
        self.save_to_file(&self.file_info)?;
        self.undo_stack.clear();
        self.redo_stack.clear();
        Ok(())
    }

    pub fn as_string(&self) -> String {
        self.lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>()
            .join("\n")
    }
    pub const fn is_file_loaded(&self) -> bool {
        self.file_info.has_path()
    }
    pub fn height(&self) -> LineIdx {
        self.lines.len()
    }
    pub fn insert_char(&mut self, character: char, at: Location) {
        self.push_undo();
        self.insert_char_no_undo(character, at);
    }
    fn insert_char_no_undo(&mut self, character: char, at: Location) {
        debug_assert!(at.line_idx <= self.height());
        if at.line_idx == self.height() {
            self.lines.push(Line::from(&character.to_string()));
        } else if let Some(line) = self.lines.get_mut(at.line_idx) {
            line.insert_char(character, at.grapheme_idx);
        }
    }
    pub fn delete(&mut self, at: Location) {
        self.push_undo();
        if let Some(line) = self.lines.get(at.line_idx) {
            if at.grapheme_idx >= line.grapheme_count()
                && self.height() > at.line_idx.saturating_add(1)
            {
                let next_line = self.lines.remove(at.line_idx.saturating_add(1));
                // clippy::indexing_slicing: We checked for existence of this line in the surrounding if statment
                #[allow(clippy::indexing_slicing)]
                self.lines[at.line_idx].append(&next_line);
            } else if at.grapheme_idx < line.grapheme_count() {
                // clippy::indexing_slicing: We checked for existence of this line in the surrounding if statment
                #[allow(clippy::indexing_slicing)]
                self.lines[at.line_idx].delete(at.grapheme_idx);
            }
        } else {
            // Undo was pushed but no line was found. We should pop it back to keep redo stack clean if we want but for now just leave it.
            // Actually, if we pushed undo and nothing happened, we might want to pop it.
            self.undo_stack.pop();
        }
    }
    pub fn insert_enter(&mut self, at: Location) -> usize {
        self.push_undo();
        let indent_string = if let Some(line) = self.lines.get(at.line_idx) {
            let non_whitespace_idx = line.first_non_whitespace_grapheme();
            let indent_end = std::cmp::min(non_whitespace_idx, at.grapheme_idx);
            line.get_substring(0..indent_end)
        } else {
            String::new()
        };
        self.insert_newline_no_undo(at);
        let mut current_at = at;
        current_at.line_idx = current_at.line_idx.saturating_add(1);
        current_at.grapheme_idx = 0;
        let mut indent_count: usize = 0;
        for character in indent_string.chars() {
            self.insert_char_no_undo(character, current_at);
            current_at.grapheme_idx = current_at.grapheme_idx.saturating_add(1);
            indent_count = indent_count.saturating_add(1);
        }
        indent_count
    }
    fn insert_newline_no_undo(&mut self, at: Location) {
        if at.line_idx == self.height() {
            self.lines.push(Line::default());
        } else if let Some(line) = self.lines.get_mut(at.line_idx) {
            let new = line.split(at.grapheme_idx);
            self.lines.insert(at.line_idx.saturating_add(1), new);
        }
    }

    pub fn concat_range(&mut self, start: Location, end: Location) {
        self.push_undo();
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        if self.lines.get(start.line_idx).is_none() {
            return;
        };
        for line_idx in start.line_idx + 1..=end.line_idx {
            if line_idx >= self.lines.len() {
                break;
            }
            let next = self.lines.remove(line_idx);
            self.lines[start.line_idx].append_char(' ');
            self.lines[start.line_idx].append(&next);
        }
    }

    pub fn get_range(&self, start: Location, end: Location) -> String {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let mut result = Vec::new();
        for line_idx in start.line_idx..=end.line_idx {
            if let Some(line) = self.lines.get(line_idx) {
                let start_g = if line_idx == start.line_idx {
                    start.grapheme_idx
                } else {
                    0
                };
                let end_g = if line_idx == end.line_idx {
                    end.grapheme_idx.saturating_add(1)
                } else {
                    line.grapheme_count()
                };
                result.push(line.get_substring(start_g..end_g));
                if line_idx == end.line_idx && end.grapheme_idx >= line.grapheme_count() {
                    result.push(String::new());
                }
            } else if line_idx == end.line_idx && end.grapheme_idx == 0 && line_idx == self.height()
            {
                result.push(String::new());
            }
        }
        result.join("\n")
    }

    pub fn delete_range(&mut self, start: Location, end: Location) {
        self.push_undo();
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };

        let mut lines_to_remove_until = end.line_idx;
        let mut suffix = String::new();

        if let Some(last_line) = self.lines.get(end.line_idx) {
            if end.grapheme_idx >= last_line.grapheme_count() {
                if end.line_idx < self.height().saturating_sub(1) {
                    lines_to_remove_until = end.line_idx.saturating_add(1);
                    // clippy::indexing_slicing: We checked that lines_to_remove_until is within bounds
                    #[allow(clippy::indexing_slicing)]
                    suffix.push_str(&self.lines[lines_to_remove_until]);
                }
            } else {
                suffix = last_line
                    .get_substring(end.grapheme_idx.saturating_add(1)..last_line.grapheme_count());
            }
        }

        if let Some(first_line) = self.lines.get_mut(start.line_idx) {
            while first_line.grapheme_count() > start.grapheme_idx {
                first_line.delete(start.grapheme_idx);
            }
            first_line.append_char(' '); // Temporarily add a char to avoid being empty if needed? No, rebuild_fragments handles empty.
            first_line.delete_last(); // Remove the temp char.
            // Actually, just push_str and rebuild.
            first_line.append(&Line::from(&suffix)); // Use append for convenience
        }

        for _ in start.line_idx..lines_to_remove_until {
            if start.line_idx.saturating_add(1) < self.lines.len() {
                self.lines.remove(start.line_idx.saturating_add(1));
            }
        }
    }

    pub fn insert_string(&mut self, string: &str, at: Location) {
        self.push_undo();
        let mut current_at = at;
        for (i, line_str) in string.split('\n').enumerate() {
            if i > 0 {
                self.insert_newline_no_undo(current_at);
                current_at.line_idx = current_at.line_idx.saturating_add(1);
                current_at.grapheme_idx = 0;
            }
            for character in line_str.chars() {
                self.insert_char_no_undo(character, current_at);
                current_at.grapheme_idx = current_at.grapheme_idx.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_enter_indentation() {
        let mut buffer = Buffer {
            lines: vec![Line::from("    hello")],
            file_info: FileInfo::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };
        let at = Location {
            line_idx: 0,
            grapheme_idx: 9, // end of "    hello"
            preferred_grapheme_idx: 9,
        };
        buffer.insert_enter(at);
        assert_eq!(buffer.lines.len(), 2);
        assert_eq!(buffer.lines[0].to_string(), "    hello");
        assert_eq!(buffer.lines[1].to_string(), "    ");
    }

    #[test]
    fn test_insert_enter_mid_indentation() {
        let mut buffer = Buffer {
            lines: vec![Line::from("    hello")],
            file_info: FileInfo::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };
        let at = Location {
            line_idx: 0,
            grapheme_idx: 2, // middle of "    "
            preferred_grapheme_idx: 2,
        };
        buffer.insert_enter(at);
        assert_eq!(buffer.lines.len(), 2);
        assert_eq!(buffer.lines[0].to_string(), "  ");
        assert_eq!(buffer.lines[1].to_string(), "    hello");
    }
}
