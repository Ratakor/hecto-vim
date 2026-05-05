use super::{Annotation, AnnotationType, Line, SyntaxHighlighter};
use crate::prelude::*;
use tree_sitter::{Parser, Query, QueryCursor, Tree, StreamingIterator};

pub struct TreeSitterHighlighter {
    parser: Parser,
    tree: Option<Tree>,
    query: Query,
    annotations: Vec<Vec<Annotation>>,
}

impl TreeSitterHighlighter {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        let language = tree_sitter_rust::LANGUAGE.into();
        parser.set_language(&language).expect("Error loading Rust grammar");
        let query = Query::new(&language, tree_sitter_rust::HIGHLIGHTS_QUERY)
            .expect("Error loading Rust highlight query");

        Self {
            parser,
            tree: None,
            query,
            annotations: Vec::new(),
        }
    }

    fn map_capture_to_annotation_type(capture: &str) -> Option<AnnotationType> {
        if capture.starts_with("comment") {
            return Some(AnnotationType::Comment);
        }
        if capture.starts_with("string") {
            return Some(AnnotationType::String);
        }
        if capture.starts_with("keyword") {
            return Some(AnnotationType::Keyword);
        }
        if capture.starts_with("type") || capture.contains("primitive") {
            return Some(AnnotationType::Type);
        }
        if capture.contains("number") || capture.contains("integer") || capture.contains("float") {
            return Some(AnnotationType::Number);
        }
        if capture.starts_with("constant") {
            return Some(AnnotationType::Constant);
        }
        if capture.starts_with("variable") || capture.contains("parameter") {
            if capture.contains("builtin") {
                return Some(AnnotationType::KnownValue);
            }
            return Some(AnnotationType::Variable);
        }
        if capture.starts_with("function") || capture.contains("method") {
            return Some(AnnotationType::Function);
        }
        if capture.starts_with("punctuation") {
            return Some(AnnotationType::Punctuation);
        }
        if capture.starts_with("operator") {
            return Some(AnnotationType::Operator);
        }
        if capture.starts_with("property") {
            return Some(AnnotationType::Property);
        }
        if capture.starts_with("boolean") {
            return Some(AnnotationType::Boolean);
        }
        if capture.contains("char") || capture.contains("character") {
            return Some(AnnotationType::Char);
        }
        if capture.starts_with("attribute") {
            return Some(AnnotationType::Attribute);
        }
        if capture.contains("macro") {
            return Some(AnnotationType::Macro);
        }
        if capture.contains("lifetime") || capture.contains("label") {
            return Some(AnnotationType::LifetimeSpecifier);
        }

        None
    }

    fn update_annotations(&mut self, source_code: &str) {
        let mut line_starts = Vec::new();
        let mut current_pos = 0;
        for line in source_code.split_inclusive('\n') {
            line_starts.push(current_pos);
            current_pos += line.len();
        }

        let mut annotations_per_line: Vec<Vec<Annotation>> = vec![Vec::new(); line_starts.len()];

        if let Some(tree) = &self.tree {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&self.query, tree.root_node(), source_code.as_bytes());
            
            let capture_names = self.query.capture_names();

            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let capture_name = &capture_names[capture.index as usize];
                    if let Some(annotation_type) = Self::map_capture_to_annotation_type(capture_name) {
                        let range = capture.node.range();
                        let start_byte = range.start_byte;
                        let end_byte = range.end_byte;
                        let start_row = range.start_point.row;
                        let end_row = range.end_point.row;

                        for row in start_row..=end_row {
                            if row >= line_starts.len() {
                                break;
                            }
                            
                            let line_start = line_starts[row];
                            let line_end = if row + 1 < line_starts.len() {
                                line_starts[row + 1]
                            } else {
                                source_code.len()
                            };

                            let start = if row == start_row {
                                start_byte.saturating_sub(line_start)
                            } else {
                                0
                            };

                            let end = if row == end_row {
                                end_byte.saturating_sub(line_start)
                            } else {
                                line_end.saturating_sub(line_start)
                            };

                            if start < end {
                                let line_annotations: &mut Vec<Annotation> = &mut annotations_per_line[row];
                                line_annotations.push(Annotation {
                                    annotation_type,
                                    start,
                                    end,
                                });
                            }
                        }
                    }
                }
            }
        }
        self.annotations = annotations_per_line;
    }
}

impl SyntaxHighlighter for TreeSitterHighlighter {
    fn highlight(&mut self, _idx: LineIdx, _line: &Line) {}

    fn get_annotations(&self, idx: LineIdx) -> Option<&Vec<Annotation>> {
        self.annotations.get(idx)
    }

    fn update(&mut self, source_code: &str) {
        self.tree = self.parser.parse(source_code, self.tree.as_ref());
        self.update_annotations(source_code);
    }
}

impl Default for TreeSitterHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_highlighting() {
        let mut highlighter = TreeSitterHighlighter::new();
        let source = "fn main() {\n    let x = 123;\n}";
        highlighter.update(source);

        let annotations_0 = highlighter.get_annotations(0).unwrap();
        assert!(annotations_0.iter().any(|a| a.annotation_type == AnnotationType::Keyword));
        assert!(annotations_0.iter().any(|a| a.annotation_type == AnnotationType::Function));

        let annotations_1 = highlighter.get_annotations(1).unwrap();
        assert!(annotations_1.iter().any(|a| a.annotation_type == AnnotationType::Keyword));
        assert!(annotations_1.iter().any(|a| a.annotation_type == AnnotationType::Constant));
    }
}
