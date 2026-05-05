use super::{GraphemeIdx, LineIdx};
use std::cmp::Ordering;

#[derive(Copy, Clone, Default, Eq, PartialEq, Debug)]
pub struct Location {
    pub line_idx: LineIdx,
    pub grapheme_idx: GraphemeIdx,
    pub preferred_grapheme_idx: GraphemeIdx,
}

impl PartialOrd for Location {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Location {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.line_idx.cmp(&other.line_idx) {
            Ordering::Equal => self.grapheme_idx.cmp(&other.grapheme_idx),
            ord => ord,
        }
    }
}
