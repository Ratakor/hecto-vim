use std::collections::HashMap;

use super::{syntaxhighlighter::SyntaxHighlighter, Annotation, AnnotationType, Line};
use crate::prelude::*;

#[derive(Default)]
pub struct SearchResultHighlighter<'a> {
    matched_word: &'a str,
    selected_match: Option<Location>,
    highlights: HashMap<LineIdx, Vec<Annotation>>,
}

impl<'a> SearchResultHighlighter<'a> {
    pub fn new(matched_word: &'a str, selected_match: Option<Location>) -> Self {
        Self {
            matched_word,
            selected_match,
            highlights: HashMap::new(),
        }
    }

    fn highlight_matched_words(
        &self,
        line_idx: LineIdx,
        line: &Line,
        result: &mut Vec<Annotation>,
    ) {
        if self.matched_word.is_empty() {
            return;
        }
        line.find_all(self.matched_word, 0..line.len())
            .iter()
            .for_each(|(byte_idx, grapheme_idx)| {
                let is_selected = self.selected_match.is_some_and(|loc| {
                    loc.line_idx == line_idx && loc.grapheme_idx == *grapheme_idx
                });
                let annotation_type = if is_selected {
                    AnnotationType::SelectedMatch
                } else {
                    AnnotationType::Match
                };

                result.push(Annotation {
                    annotation_type,
                    start: *byte_idx,
                    end: byte_idx.saturating_add(self.matched_word.len()),
                });
            });
    }
}

impl SyntaxHighlighter for SearchResultHighlighter<'_> {
    fn highlight(&mut self, idx: LineIdx, line: &Line) {
        let mut result = Vec::new();
        self.highlight_matched_words(idx, line, &mut result);
        self.highlights.insert(idx, result);
    }
    fn get_annotations(&self, idx: LineIdx) -> Option<&Vec<Annotation>> {
        self.highlights.get(&idx)
    }
}
