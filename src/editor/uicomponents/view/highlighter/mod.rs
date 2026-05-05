use super::super::super::{Annotation, AnnotationType, FileType, Line};
use crate::prelude::*;
mod syntaxhighlighter;
mod selectionhighlighter;
use selectionhighlighter::SelectionHighlighter;
use searchresulthighlighter::SearchResultHighlighter;
pub use syntaxhighlighter::SyntaxHighlighter;
mod searchresulthighlighter;
mod treesitterhighlighter;
use treesitterhighlighter::TreeSitterHighlighter;

pub fn create_syntax_highlighter(file_type: FileType) -> Option<Box<dyn SyntaxHighlighter>> {
    match file_type {
        FileType::Rust | FileType::JavaScript | FileType::Zig => {
            Some(Box::new(TreeSitterHighlighter::new(file_type)))
        }
        FileType::Text => None,
    }
}

#[derive(Default)]
pub struct Highlighter<'a> {
    syntax_highlighter: Option<&'a mut (dyn SyntaxHighlighter + 'static)>,
    search_result_highlighter: Option<SearchResultHighlighter<'a>>,
    selection_highlighter: Option<SelectionHighlighter>,
}

impl<'a> Highlighter<'a> {
    pub fn new(
        matched_word: Option<&'a str>,
        selected_match: Option<Location>,
        selection: Option<(Location, Location)>,
        syntax_highlighter: Option<&'a mut (dyn SyntaxHighlighter + 'static)>,
    ) -> Self {
        let search_result_highlighter = matched_word
            .map(|matched_word| SearchResultHighlighter::new(matched_word, selected_match));
        let selection_highlighter = selection.map(|(start, end)| SelectionHighlighter::new(start, end));
        Self {
            syntax_highlighter,
            search_result_highlighter,
            selection_highlighter,
        }
    }
    pub fn get_annotations(&self, idx: LineIdx, line: &Line) -> Vec<Annotation> {
        let mut result = Vec::new();

        if let Some(syntax_highlighter) = &self.syntax_highlighter {
            if let Some(annotations) = syntax_highlighter.get_annotations(idx) {
                result.extend(annotations.iter().copied());
            }
        }
        if let Some(search_result_highlighter) = &self.search_result_highlighter {
            if let Some(annotations) = search_result_highlighter.get_annotations(idx) {
                result.extend(annotations.iter().copied());
            }
        }
        if let Some(selection_highlighter) = &self.selection_highlighter {
            if let Some(annotations) = selection_highlighter.get_annotations(idx, line) {
                result.extend(annotations.iter().copied());
            }
        }
        result
    }
    pub fn highlight(&mut self, idx: LineIdx, line: &Line) {
        if let Some(syntax_highlighter) = &mut self.syntax_highlighter {
            syntax_highlighter.highlight(idx, line);
        }
        if let Some(search_result_highlighter) = &mut self.search_result_highlighter {
            search_result_highlighter.highlight(idx, line);
        }
    }
}
