use crate::prelude::*;

use super::FileType;

#[derive(Default, Eq, PartialEq, Debug)]
pub struct DocumentStatus {
    pub total_lines: usize,
    pub current_line_idx: LineIdx,
    pub current_col_idx: ColIdx,
    pub is_modified: bool,
    pub file_name: String,
    pub file_type: FileType,
    pub mode: String,
}

impl DocumentStatus {
    pub fn modified_indicator_to_string(&self) -> String {
        if self.is_modified {
            " [+]".to_string()
        } else {
            String::new()
        }
    }
    pub fn position_indicator_to_string(&self) -> String {
        format!(
            "{}:{}",
            self.current_line_idx.saturating_add(1),
            self.current_col_idx.saturating_add(1)
        )
    }
    pub fn file_type_to_string(&self) -> String {
        format!("{}", self.file_type)
    }
}
