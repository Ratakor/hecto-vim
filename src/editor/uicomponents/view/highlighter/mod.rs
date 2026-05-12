use super::super::super::{Annotation, AnnotationType, FileType, Line};
use crate::prelude::*;
mod diagnostichighlighter;
mod selectionhighlighter;
mod syntaxhighlighter;
use diagnostichighlighter::DiagnosticHighlighter;
use searchresulthighlighter::SearchResultHighlighter;
use selectionhighlighter::SelectionHighlighter;
pub use syntaxhighlighter::SyntaxHighlighter;
mod searchresulthighlighter;
mod treesitterhighlighter;
use treesitterhighlighter::TreeSitterHighlighter;

pub fn create_syntax_highlighter(file_type: FileType) -> Option<Box<dyn SyntaxHighlighter>> {
    match file_type {
        FileType::Rust | FileType::JavaScript | FileType::Zig | FileType::Nix => {
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
    diagnostic_highlighter: Option<DiagnosticHighlighter>,
}

impl<'a> Highlighter<'a> {
    pub fn new(
        matched_word: Option<&'a str>,
        selected_match: Option<Location>,
        selection: Option<(Location, Location)>,
        diagnostics: Option<Vec<lsp_types::Diagnostic>>,
        syntax_highlighter: Option<&'a mut (dyn SyntaxHighlighter + 'static)>,
    ) -> Self {
        let search_result_highlighter = matched_word
            .map(|matched_word| SearchResultHighlighter::new(matched_word, selected_match));
        let selection_highlighter =
            selection.map(|(start, end)| SelectionHighlighter::new(start, end));
        let diagnostic_highlighter = diagnostics.map(DiagnosticHighlighter::new);
        Self {
            syntax_highlighter,
            search_result_highlighter,
            selection_highlighter,
            diagnostic_highlighter,
        }
    }
    pub fn get_annotations(&self, idx: LineIdx, line: &Line) -> Vec<Annotation> {
        let mut result = Vec::new();

        if let Some(syntax_highlighter) = &self.syntax_highlighter
            && let Some(annotations) = syntax_highlighter.get_annotations(idx)
        {
            result.extend(annotations.iter().copied());
        }
        if let Some(search_result_highlighter) = &self.search_result_highlighter
            && let Some(annotations) = search_result_highlighter.get_annotations(idx)
        {
            result.extend(annotations.iter().copied());
        }
        if let Some(selection_highlighter) = &self.selection_highlighter
            && let Some(annotations) = selection_highlighter.get_annotations(idx, line)
        {
            result.extend(annotations.iter().copied());
        }
        if let Some(diagnostic_highlighter) = &self.diagnostic_highlighter
            && let Some(annotations) = diagnostic_highlighter.get_annotations(idx, line)
        {
            result.extend(annotations.iter().copied());
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
