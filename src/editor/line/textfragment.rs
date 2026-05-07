use crate::prelude::*;

use super::super::AnnotationType;
use super::GraphemeWidth;

#[derive(Clone, Debug)]
pub struct TextFragment {
    pub grapheme: String,
    pub rendered_width: GraphemeWidth,
    pub replacement: Option<String>,
    pub replacement_annotations: Vec<(AnnotationType, usize, usize)>,
    pub start: ByteIdx,
}
