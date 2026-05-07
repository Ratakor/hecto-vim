use crate::editor::{Annotation, AnnotationType, Line};
use crate::prelude::*;

pub struct SelectionHighlighter {
    start: Location,
    end: Location,
}

impl SelectionHighlighter {
    pub fn new(start: Location, end: Location) -> Self {
        if start <= end {
            Self { start, end }
        } else {
            Self {
                start: end,
                end: start,
            }
        }
    }

    pub fn get_annotations(&self, idx: LineIdx, line: &Line) -> Option<Vec<Annotation>> {
        if idx < self.start.line_idx || idx > self.end.line_idx {
            return None;
        }

        let start_grapheme = if idx == self.start.line_idx {
            self.start.grapheme_idx
        } else {
            0
        };

        let end_grapheme = if idx == self.end.line_idx {
            self.end.grapheme_idx
        } else {
            line.grapheme_count()
        };

        let start_byte = line.grapheme_idx_to_byte_idx(start_grapheme);
        let end_byte = if end_grapheme >= line.grapheme_count() {
            line.len().saturating_add(1)
        } else {
            line.grapheme_idx_to_byte_idx(end_grapheme)
        };


        if start_byte >= end_byte {
            return None;
        }

        Some(vec![Annotation {
            annotation_type: AnnotationType::Selection,
            start: start_byte,
            end: end_byte,
        }])
    }
}
