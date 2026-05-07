use crate::prelude::*;
use std::{
    cmp::min,
    fmt::{self, Display},
    ops::{Deref, Range},
};
mod graphemewidth;
mod textfragment;
use graphemewidth::GraphemeWidth;
use textfragment::TextFragment;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::AnnotatedString;
use super::Annotation;

#[derive(Default, Clone)]
pub struct Line {
    fragments: Vec<TextFragment>,
    string: String,
}

impl Line {
    pub fn from(line_str: &str) -> Self {
        debug_assert!(line_str.is_empty() || line_str.lines().count() == 1);
        let fragments = Self::str_to_fragments(line_str);
        Self {
            fragments,
            string: String::from(line_str),
        }
    }

    fn str_to_fragments(line_str: &str) -> Vec<TextFragment> {
        line_str
            .grapheme_indices(true)
            .map(|(byte_idx, grapheme)| {
                let (replacement, rendered_width) = Self::get_replacement_character(grapheme)
                    .map_or_else(
                        || {
                            let unicode_width = grapheme.width();
                            let rendered_width = match unicode_width {
                                0 | 1 => GraphemeWidth::Half,
                                _ => GraphemeWidth::Full,
                            };
                            (None, rendered_width)
                        },
                        |replacement| (Some(replacement), GraphemeWidth::Half),
                    );

                TextFragment {
                    grapheme: grapheme.to_string(),
                    rendered_width,
                    replacement,
                    start: byte_idx,
                }
            })
            .collect()
    }

    fn rebuild_fragments(&mut self) {
        self.fragments = Self::str_to_fragments(&self.string);
    }

    fn get_replacement_character(for_str: &str) -> Option<char> {
        let width = for_str.width();
        match for_str {
            " " => None,
            "\t" => Some(' '),
            _ if width > 0 && for_str.trim().is_empty() => Some('␣'),
            _ => {
                let mut chars = for_str.chars();
                if let Some(ch) = chars.next() {
                    if ch.is_control() && chars.next().is_none() {
                        return Some('▯');
                    }
                }
                None
            } // _ if width == 0 => {
              //     let mut chars = for_str.chars();
              //     if let Some(ch) = chars.next() {
              //         if ch.is_control() && chars.next().is_none() {
              //             return Some('▯');
              //         }
              //     }
              //     Some('·')
              // }
              // _ => None,
        }
    }
    // Gets the visible graphemes in the given column index.
    // Note that the column index is not the same as the grapheme index:
    // A grapheme can have a width of 2 columns.
    pub fn get_visible_graphemes(&self, range: Range<ColIdx>) -> String {
        self.get_annotated_visible_substr(range, None).to_string()
    }

    // Gets the annotated string in the given column index.
    // Note that the column index is not the same as the grapheme index:
    // A grapheme can have a width of 2 columns.
    // Parameters:
    // - range: The range of columns to get the annotated string from.
    // - query: The query to highlight in the annotated string.
    // - selected_match: The selected match to highlight in the annotated string. This is only applied if the query is not empty.
    pub fn get_annotated_visible_substr(
        &self,
        range: Range<ColIdx>,
        annotations: Option<&Vec<Annotation>>,
    ) -> AnnotatedString {
        if range.start >= range.end {
            return AnnotatedString::default();
        }
        // Create a new annotated string with a virtual space for the newline
        let mut result = AnnotatedString::from(&(self.string.clone() + " "));

        // Apply annotations for this string
        if let Some(annotations) = annotations {
            for annotation in annotations {
                result.add_annotation(annotation.annotation_type, annotation.start, annotation.end);
            }
        }

        // Insert replacement characters, and truncate if needed.
        // We do this backwards, otherwise the byte indices would be off in case a replacement character has a different width than the original character.

        // Handle virtual newline space clipping
        if self.width() >= range.end {
            result.truncate_right_from(self.string.len());
        } else if self.width().saturating_add(1) <= range.start {
            result.truncate_left_until(self.string.len() + 1);
        }

        let mut fragment_start = self.width();
        for fragment in self.fragments.iter().rev() {
            let fragment_end = fragment_start;
            fragment_start = fragment_start.saturating_sub(fragment.rendered_width.into());

            if fragment_start > range.end {
                continue; // No  processing needed if we haven't reached the visible range yet.
            }

            // clip right if the fragment is partially visible
            if fragment_start < range.end && fragment_end > range.end {
                result.replace(fragment.start, self.string.len(), "⋯");
                continue;
            } else if fragment_start == range.end {
                // Truncate right if we've reached the end of the visible range
                result.truncate_right_from(fragment.start);
                continue;
            }

            // Fragment ends at the start of the range: Remove the entire left side of the string (if not already at start of string)
            if fragment_end <= range.start {
                result.truncate_left_until(fragment.start.saturating_add(fragment.grapheme.len()));
                break; //End processing since all remaining fragments will be invisible.
            } else if fragment_start < range.start && fragment_end > range.start {
                // Fragment overlaps with the start of range: Remove the left side of the string and add an ellipsis
                result.replace(
                    0,
                    fragment.start.saturating_add(fragment.grapheme.len()),
                    "⋯",
                );
                break; //End processing since all remaining fragments will be invisible.
            }

            // Fragment is fully within range: Apply replacement characters if appropriate
            if fragment_start >= range.start && fragment_end <= range.end {
                if let Some(replacement) = fragment.replacement {
                    let start = fragment.start;
                    let end = start.saturating_add(fragment.grapheme.len());
                    result.replace(start, end, &replacement.to_string());
                }
            }
        }

        result
    }

    pub fn get_substring(&self, range: Range<GraphemeIdx>) -> String {
        let start = min(range.start, self.grapheme_count());
        let end = min(range.end, self.grapheme_count());
        if start >= end {
            return String::new();
        }
        self.fragments[start..end]
            .iter()
            .map(|f| f.grapheme.as_str())
            .collect()
    }

    pub fn grapheme_count(&self) -> GraphemeIdx {
        self.fragments.len()
    }
    pub fn first_non_whitespace_grapheme(&self) -> GraphemeIdx {
        self.fragments
            .iter()
            .position(|fragment| !fragment.grapheme.trim().is_empty())
            .unwrap_or(0)
    }
    pub fn width_until(&self, grapheme_idx: GraphemeIdx) -> ColIdx {
        self.fragments
            .iter()
            .take(grapheme_idx)
            .map(|fragment| match fragment.rendered_width {
                GraphemeWidth::Half => 1,
                GraphemeWidth::Full => 2,
            })
            .sum()
    }
    pub fn width(&self) -> ColIdx {
        self.width_until(self.grapheme_count())
    }

    pub fn grapheme_at_width(&self, width: ColIdx) -> GraphemeIdx {
        let mut current_width: ColIdx = 0;
        for (idx, fragment) in self.fragments.iter().enumerate() {
            let fragment_width: usize = fragment.rendered_width.into();
            if current_width.saturating_add(fragment_width) > width {
                return idx;
            }
            current_width = current_width.saturating_add(fragment_width);
        }
        self.grapheme_count()
    }
    // Inserts a character into the line, or appends it at the end if at == grapheme_count + 1
    pub fn insert_char(&mut self, character: char, at: GraphemeIdx) {
        debug_assert!(at.saturating_sub(1) <= self.grapheme_count());
        if let Some(fragment) = self.fragments.get(at) {
            self.string.insert(fragment.start, character);
        } else {
            self.string.push(character);
        }
        self.rebuild_fragments();
    }
    pub fn append_char(&mut self, character: char) {
        self.insert_char(character, self.grapheme_count());
    }
    pub fn delete(&mut self, at: GraphemeIdx) {
        debug_assert!(at <= self.grapheme_count());
        if let Some(fragment) = self.fragments.get(at) {
            let start = fragment.start;
            let end = fragment.start.saturating_add(fragment.grapheme.len());
            self.string.drain(start..end);
            self.rebuild_fragments();
        }
    }

    pub fn delete_last(&mut self) {
        self.delete(self.grapheme_count().saturating_sub(1));
    }

    pub fn append(&mut self, other: &Self) {
        self.string.push_str(&other.string);
        self.rebuild_fragments();
    }

    pub fn split(&mut self, at: GraphemeIdx) -> Self {
        if let Some(fragment) = self.fragments.get(at) {
            let remainder = self.string.split_off(fragment.start);
            self.rebuild_fragments();
            Self::from(&remainder)
        } else {
            Self::default()
        }
    }
    fn byte_idx_to_grapheme_idx(&self, byte_idx: ByteIdx) -> Option<GraphemeIdx> {
        if byte_idx > self.string.len() {
            return None;
        }
        self.fragments
            .iter()
            .position(|fragment| fragment.start >= byte_idx)
    }
    pub fn grapheme_idx_to_byte_idx(&self, grapheme_idx: GraphemeIdx) -> ByteIdx {
        if grapheme_idx >= self.grapheme_count() {
            return self.string.len();
        }
        self.fragments
            .get(grapheme_idx)
            .map_or(0, |fragment| fragment.start)
    }
    pub fn search_forward(
        &self,
        query: &str,
        from_grapheme_idx: GraphemeIdx,
    ) -> Option<GraphemeIdx> {
        debug_assert!(from_grapheme_idx <= self.grapheme_count());
        if from_grapheme_idx == self.grapheme_count() {
            return None;
        }
        let start = self.grapheme_idx_to_byte_idx(from_grapheme_idx);
        self.find_all(query, start..self.string.len())
            .first()
            .map(|(_, grapheme_idx)| *grapheme_idx)
    }
    pub fn search_backward(
        &self,
        query: &str,
        from_grapheme_idx: GraphemeIdx,
    ) -> Option<GraphemeIdx> {
        debug_assert!(from_grapheme_idx <= self.grapheme_count());

        if from_grapheme_idx == 0 {
            return None;
        }
        let end_byte_index = if from_grapheme_idx == self.grapheme_count() {
            self.string.len()
        } else {
            self.grapheme_idx_to_byte_idx(from_grapheme_idx)
        };
        self.find_all(query, 0..end_byte_index)
            .last()
            .map(|(_, grapheme_idx)| *grapheme_idx)
    }
    pub fn find_all(&self, query: &str, range: Range<ByteIdx>) -> Vec<(ByteIdx, GraphemeIdx)> {
        let end = min(range.end, self.string.len());
        let start = range.start;
        debug_assert!(start <= end);
        debug_assert!(start <= self.string.len());
        self.string.get(start..end).map_or_else(Vec::new, |substr| {
            let potential_matches: Vec<ByteIdx> = substr
                .match_indices(query) // find _potential_ matches within the substring
                .map(|(relative_start_idx, _)| {
                    relative_start_idx.saturating_add(start) //convert their relative indices to absolute indices
                })
                .collect();
            self.match_grapheme_clusters(&potential_matches, query) //convert the potential matches into matches which align with the grapheme boundaries.
        })
    }

    // Finds all matches which align with grapheme boundaries.
    // Parameters:
    // - query: The query to search for.
    // - matches: A vector of byte indices of potential matches, which might or might not align with the grapheme clusters.
    // Returns:
    // A Vec of (byte_index, grapheme_idx) pairs for each match that alignes with the grapheme clusters, where byte_index is the byte index of the match, and grapheme_idx is the grapheme index of the match.
    fn match_grapheme_clusters(
        &self,
        matches: &[ByteIdx],
        query: &str,
    ) -> Vec<(ByteIdx, GraphemeIdx)> {
        let grapheme_count = query.graphemes(true).count();
        matches
            .iter()
            .filter_map(|&start| {
                self.byte_idx_to_grapheme_idx(start)
                    .and_then(|grapheme_idx| {
                        self.fragments
                            .get(grapheme_idx..grapheme_idx.saturating_add(grapheme_count)) // get all fragments that should be part of the match
                            .and_then(|fragments| {
                                let substring = fragments
                                    .iter()
                                    .map(|fragment| fragment.grapheme.as_str())
                                    .collect::<String>(); //combine the fragments into a single string.
                                (substring == query).then_some((start, grapheme_idx))
                                // if the combined string matches the query, we have an actual match.
                            })
                    })
            })
            .collect()
    }
}

impl Display for Line {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}", self.string)
    }
}

impl Deref for Line {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::AnnotationType;

    #[test]
    fn test_line_from() {
        let line = Line::from("hello");
        assert_eq!(line.string, "hello");
        assert_eq!(line.grapheme_count(), 5);
    }

    #[test]
    fn test_insert_char() {
        let mut line = Line::from("hello");
        line.insert_char('!', 5);
        assert_eq!(line.string, "hello!");
        line.insert_char(' ', 0);
        assert_eq!(line.string, " hello!");
    }

    #[test]
    fn test_delete() {
        let mut line = Line::from("hello");
        line.delete(0);
        assert_eq!(line.string, "ello");
        line.delete(3);
        assert_eq!(line.string, "ell");
    }

    #[test]
    fn test_append() {
        let mut line1 = Line::from("hello ");
        let line2 = Line::from("world");
        line1.append(&line2);
        assert_eq!(line1.string, "hello world");
    }

    #[test]
    fn test_split() {
        let mut line = Line::from("hello world");
        let line2 = line.split(6);
        assert_eq!(line.string, "hello ");
        assert_eq!(line2.string, "world");
    }

    #[test]
    fn test_search_forward() {
        let line = Line::from("hello world hello");
        assert_eq!(line.search_forward("hello", 0), Some(0));
        assert_eq!(line.search_forward("hello", 1), Some(12));
        assert_eq!(line.search_forward("world", 0), Some(6));
        assert_eq!(line.search_forward("xyz", 0), None);
    }

    #[test]
    fn test_search_backward() {
        let line = Line::from("hello world hello");
        assert_eq!(line.search_backward("hello", 17), Some(12));
        assert_eq!(line.search_backward("hello", 11), Some(0));
        assert_eq!(line.search_backward("world", 17), Some(6));
    }

    #[test]
    fn test_grapheme_width() {
        // "a" is width 1, "👍" is width 2
        let line = Line::from("a👍b");
        assert_eq!(line.width_until(0), 0);
        assert_eq!(line.width_until(1), 1); // "a"
        assert_eq!(line.width_until(2), 3); // "a👍"
        assert_eq!(line.width_until(3), 4); // "a👍b"
    }

    #[test]
    fn test_grapheme_at_width() {
        let line = Line::from("a👍b");
        assert_eq!(line.grapheme_at_width(0), 0);
        assert_eq!(line.grapheme_at_width(1), 1);
        assert_eq!(line.grapheme_at_width(2), 1); // inside "👍"
        assert_eq!(line.grapheme_at_width(3), 2);
        assert_eq!(line.grapheme_at_width(4), 3);
    }

    #[test]
    fn test_get_annotated_visible_substr() {
        let line = Line::from("hello");
        let annotations = vec![Annotation {
            annotation_type: AnnotationType::Selection,
            start: 0,
            end: 6, // includes virtual space
        }];

        // Full visible
        let res = line.get_annotated_visible_substr(0..10, Some(&annotations));
        assert_eq!(res.to_string(), "hello ");

        // Clipped right (virtual space removed)
        let res = line.get_annotated_visible_substr(0..5, Some(&annotations));
        assert_eq!(res.to_string(), "hello");

        // Clipped right (in text)
        let res = line.get_annotated_visible_substr(0..3, Some(&annotations));
        assert_eq!(res.to_string(), "hel");

        // Clipped left
        let res = line.get_annotated_visible_substr(2..10, Some(&annotations));
        assert_eq!(res.to_string(), "llo ");
    }
}
