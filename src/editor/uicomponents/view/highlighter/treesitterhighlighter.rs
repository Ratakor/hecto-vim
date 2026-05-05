use super::{Annotation, AnnotationType, Line, SyntaxHighlighter};
use crate::editor::FileType;
use crate::prelude::*;
use std::cmp::min;
use tree_sitter::{Parser, Query, QueryCursor, Tree, StreamingIterator};

pub struct TreeSitterHighlighter {
    parser: Parser,
    tree: Option<Tree>,
    query: Query,
    annotations: Vec<Vec<Annotation>>,
}

impl TreeSitterHighlighter {
    pub fn new(file_type: FileType) -> Self {
        let mut parser = Parser::new();
        let (language, query_str) = match file_type {
            FileType::Rust => (
                tree_sitter_rust::LANGUAGE.into(),
                tree_sitter_rust::HIGHLIGHTS_QUERY,
            ),
            FileType::JavaScript => (
                tree_sitter_javascript::LANGUAGE.into(),
                tree_sitter_javascript::HIGHLIGHT_QUERY,
            ),
            FileType::Zig => (
                tree_sitter_zig::LANGUAGE.into(),
                tree_sitter_zig::HIGHLIGHTS_QUERY,
            ),
            FileType::Text => panic!("Cannot create TreeSitterHighlighter for Text"),
        };
        parser.set_language(&language).expect("Error loading grammar");
        let query = Query::new(&language, query_str).expect("Error loading highlight query");

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
        let mut line_info = Vec::new();
        let mut current_pos = 0;
        let lines: Vec<&str> = source_code.lines().collect();
        for line in &lines {
            line_info.push((current_pos, line.len()));
            current_pos += line.len() + 1; // +1 for the \n
        }

        let mut annotations_per_line: Vec<Vec<Annotation>> = vec![Vec::new(); line_info.len()];

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
                            if row >= line_info.len() {
                                break;
                            }
                            
                            let (line_start, line_len) = line_info[row];

                            let start = if row == start_row {
                                start_byte.saturating_sub(line_start)
                            } else {
                                0
                            };

                            let end = if row == end_row {
                                end_byte.saturating_sub(line_start)
                            } else {
                                line_len
                            };

                            let start = min(start, line_len);
                            let end = min(end, line_len);

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
        self.tree = self.parser.parse(source_code, None);
        self.update_annotations(source_code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_highlighting() {
        let mut highlighter = TreeSitterHighlighter::new(FileType::Rust);
        let source = "fn main() {\n    let x = 123;\n}";
        highlighter.update(source);

        let annotations_0 = highlighter.get_annotations(0).unwrap();
        assert!(annotations_0.iter().any(|a| a.annotation_type == AnnotationType::Keyword));
        assert!(annotations_0.iter().any(|a| a.annotation_type == AnnotationType::Function));

        let annotations_1 = highlighter.get_annotations(1).unwrap();
        assert!(annotations_1.iter().any(|a| a.annotation_type == AnnotationType::Keyword));
        assert!(annotations_1.iter().any(|a| a.annotation_type == AnnotationType::Constant));
    }

    #[test]
    fn test_javascript_highlighting() {
        let mut highlighter = TreeSitterHighlighter::new(FileType::JavaScript);
        let source = "function main() {\n    const x = 123;\n}";
        highlighter.update(source);

        let annotations_0 = highlighter.get_annotations(0).unwrap();
        assert!(annotations_0.iter().any(|a| a.annotation_type == AnnotationType::Keyword));
        assert!(annotations_0.iter().any(|a| a.annotation_type == AnnotationType::Function));

        let annotations_1 = highlighter.get_annotations(1).unwrap();
        assert!(annotations_1.iter().any(|a| a.annotation_type == AnnotationType::Keyword));
        assert!(annotations_1.iter().any(|a| a.annotation_type == AnnotationType::Number));
    }

    #[test]
    fn test_zig_highlighting() {
        let mut highlighter = TreeSitterHighlighter::new(FileType::Zig);
        let source = "fn main() void {\n    const x = 123;\n}";
        highlighter.update(source);

        let annotations_0 = highlighter.get_annotations(0).unwrap();
        assert!(annotations_0.iter().any(|a| a.annotation_type == AnnotationType::Keyword));
        assert!(annotations_0.iter().any(|a| a.annotation_type == AnnotationType::Function));

        let annotations_1 = highlighter.get_annotations(1).unwrap();
        assert!(annotations_1.iter().any(|a| a.annotation_type == AnnotationType::Keyword));
        assert!(annotations_1.iter().any(|a| a.annotation_type == AnnotationType::Number));
    }
}
