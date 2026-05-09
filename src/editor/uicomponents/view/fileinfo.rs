use super::super::super::FileType;
use std::{
    fmt::{self, Display},
    path::{Path, PathBuf},
};

#[derive(Default, Debug)]
pub struct FileInfo {
    path: Option<PathBuf>,
    display_name: Option<String>,
    file_type: FileType,
}

impl FileInfo {
    pub fn new_internal(name: &str) -> Self {
        Self {
            path: None,
            display_name: Some(name.to_string()),
            file_type: FileType::Text,
        }
    }
    pub fn from(file_name: &str) -> Self {
        let path = PathBuf::from(file_name);
        let file_type = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(FileType::from_extension)
            .unwrap_or_default();
        Self {
            path: Some(path),
            display_name: None,
            file_type,
        }
    }
    pub fn get_path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub const fn has_path(&self) -> bool {
        self.path.is_some()
    }
    pub const fn get_file_type(&self) -> FileType {
        self.file_type
    }
}

impl Display for FileInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = &self.display_name {
            return write!(formatter, "{name}");
        }
        let name = self
            .get_path()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("[No Name]");
        write!(formatter, "{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_info_rust() {
        let info = FileInfo::from("test.rs");
        assert_eq!(info.get_file_type(), FileType::Rust);
        assert_eq!(format!("{info}"), "test.rs");
    }

    #[test]
    fn test_file_info_javascript() {
        let info = FileInfo::from("test.js");
        assert_eq!(info.get_file_type(), FileType::JavaScript);
    }

    #[test]
    fn test_file_info_zig() {
        let info = FileInfo::from("test.zig");
        assert_eq!(info.get_file_type(), FileType::Zig);
    }

    #[test]
    fn test_file_info_nix() {
        let info = FileInfo::from("test.nix");
        assert_eq!(info.get_file_type(), FileType::Nix);
    }

    #[test]
    fn test_file_info_text() {
        let info = FileInfo::from("test.txt");
        assert_eq!(info.get_file_type(), FileType::Text);
    }

    #[test]
    fn test_file_info_no_extension() {
        let info = FileInfo::from("README");
        assert_eq!(info.get_file_type(), FileType::Text);
    }
}
