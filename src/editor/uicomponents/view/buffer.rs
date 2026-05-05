use super::super::super::AnnotatedString;
use super::FileInfo;
use super::Highlighter;
use super::Line;
use crate::prelude::*;
use std::fs::{read_to_string, File};
use std::io::Error;
use std::io::Write;
use std::ops::Range;
use std::cmp::min;

pub struct Buffer {
    lines: Vec<Line>,
    file_info: FileInfo,
    dirty: bool,
    undo_stack: Vec<Vec<Line>>,
    redo_stack: Vec<Vec<Line>>,
}

impl Default for Buffer {
    fn default() -> Self {
        Self {
            lines: vec![Line::default()],
            file_info: FileInfo::default(),
            dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}

impl Buffer {
    pub const fn is_dirty(&self) -> bool {
        self.dirty
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
            self.dirty = true;
            return true;
        }
        false
    }

    pub fn redo(&mut self) -> bool {
        if let Some(lines) = self.redo_stack.pop() {
            self.undo_stack.push(self.lines.clone());
            self.lines = lines;
            self.dirty = true;
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
        range: Range<GraphemeIdx>,
        highlighter: &Highlighter,
    ) -> Option<AnnotatedString> {
        self.lines.get(line_idx).map(|line| {
            line.get_annotated_visible_substr(range, Some(&highlighter.get_annotations(line_idx, line)))
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
            dirty: false,
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
        self.dirty = false;
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), Error> {
        self.save_to_file(&self.file_info)?;
        self.dirty = false;
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
            self.dirty = true;
        } else if let Some(line) = self.lines.get_mut(at.line_idx) {
            line.insert_char(character, at.grapheme_idx);
            self.dirty = true;
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
                self.dirty = true;
            } else if at.grapheme_idx < line.grapheme_count() {
                // clippy::indexing_slicing: We checked for existence of this line in the surrounding if statment
                #[allow(clippy::indexing_slicing)]
                self.lines[at.line_idx].delete(at.grapheme_idx);
                self.dirty = true;
            }
        } else {
            // Undo was pushed but no line was found. We should pop it back to keep redo stack clean if we want but for now just leave it.
            // Actually, if we pushed undo and nothing happened, we might want to pop it.
            self.undo_stack.pop();
        }
    }
    pub fn insert_newline(&mut self, at: Location) {
        self.push_undo();
        self.insert_newline_no_undo(at);
    }
    fn insert_newline_no_undo(&mut self, at: Location) {
        if at.line_idx == self.height() {
            self.lines.push(Line::default());
            self.dirty = true;
        } else if let Some(line) = self.lines.get_mut(at.line_idx) {
            let new = line.split(at.grapheme_idx);
            self.lines.insert(at.line_idx.saturating_add(1), new);
            self.dirty = true;
        }
    }

    pub fn get_range(&self, start: Location, end: Location) -> String {
        let (start, end) = if start <= end { (start, end) } else { (end, start) };
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
            }
        }
        result.join("\n")
    }

    pub fn delete_range(&mut self, start: Location, end: Location) {
        self.push_undo();
        let (start, end) = if start <= end { (start, end) } else { (end, start) };

        if start.line_idx == end.line_idx {
            if let Some(line) = self.lines.get_mut(start.line_idx) {
                let count = end.grapheme_idx.saturating_add(1).saturating_sub(start.grapheme_idx);
                for _ in 0..count {
                    line.delete(start.grapheme_idx);
                }
                self.dirty = true;
            }
            return;
        }

        // Multiple lines
        if let Some(line) = self.lines.get_mut(start.line_idx) {
            while line.grapheme_count() > start.grapheme_idx {
                line.delete(start.grapheme_idx);
            }
        }

        if let Some(last_line) = self.lines.get(end.line_idx) {
            let mut remaining = last_line.clone();
            for _ in 0..end.grapheme_idx.saturating_add(1) {
                remaining.delete(0);
            }
            if let Some(first_line) = self.lines.get_mut(start.line_idx) {
                first_line.append(&remaining);
            }
        }

        let height = self.height();
        let end_to_remove = min(end.line_idx, height.saturating_sub(1));
        for _ in start.line_idx..end_to_remove {
            self.lines.remove(start.line_idx.saturating_add(1));
        }
        self.dirty = true;
    }

    pub fn insert_string(&mut self, string: &str, at: Location) {
        self.push_undo();
        let mut current_at = at;
        for (i, line_str) in string.lines().enumerate() {
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
        self.dirty = true;
    }
}
