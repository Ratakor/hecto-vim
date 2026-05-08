use crate::editor::{Annotation, AnnotationType, Line};
use crate::prelude::*;
use lsp_types::{Diagnostic, DiagnosticSeverity};

pub struct DiagnosticHighlighter {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticHighlighter {
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        Self { diagnostics }
    }

    pub fn get_annotations(&self, idx: LineIdx, line: &Line) -> Option<Vec<Annotation>> {
        let mut result = Vec::new();

        for diagnostic in &self.diagnostics {
            let range = diagnostic.range;
            let start_line = range.start.line as usize;
            let end_line = range.end.line as usize;

            if idx >= start_line && idx <= end_line {
                let start_utf16 = if idx == start_line {
                    range.start.character as usize
                } else {
                    0
                };
                let end_utf16 = if idx == end_line {
                    range.end.character as usize
                } else {
                    // This is still a bit tricky if it spans multiple lines.
                    // For now, assume it ends at the end of the current line if it's not the last line.
                    10000 // Large number to mean "end of line"
                };

                let start_byte = line.utf16_code_unit_to_byte_idx(start_utf16);
                let end_byte = if end_utf16 >= 10000 {
                    line.len()
                } else {
                    line.utf16_code_unit_to_byte_idx(end_utf16)
                };

                if start_byte < end_byte {
                    let annotation_type = match diagnostic.severity {
                        Some(DiagnosticSeverity::ERROR) => AnnotationType::Error,
                        Some(DiagnosticSeverity::WARNING) => AnnotationType::Warning,
                        Some(DiagnosticSeverity::INFORMATION) => AnnotationType::Information,
                        Some(DiagnosticSeverity::HINT) => AnnotationType::Hint,
                        _ => AnnotationType::Error,
                    };

                    result.push(Annotation {
                        annotation_type,
                        start: start_byte,
                        end: end_byte,
                    });
                }
            }
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}
