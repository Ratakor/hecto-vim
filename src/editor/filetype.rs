use std::fmt::{Display, Formatter, Result};

#[derive(Default, Eq, PartialEq, Hash, Debug, Copy, Clone)]
pub enum FileType {
    Rust,
    JavaScript,
    Zig,
    #[default]
    Text,
}

impl FileType {
    pub fn language_id(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::JavaScript => "javascript",
            Self::Zig => "zig",
            Self::Text => "text",
        }
    }

    pub fn lsp_server(&self) -> Option<(&'static str, Vec<&'static str>)> {
        match self {
            Self::Rust => Some(("rust-analyzer", vec![])),
            Self::JavaScript => Some(("typescript-language-server", vec!["--stdio"])),
            Self::Zig => Some(("zls", vec![])),
            Self::Text => None,
        }
    }

    pub fn from_extension(extension: &str) -> Self {
        if extension.eq_ignore_ascii_case("rs") {
            Self::Rust
        } else if extension.eq_ignore_ascii_case("js") {
            Self::JavaScript
        } else if extension.eq_ignore_ascii_case("zig") {
            Self::Zig
        } else {
            Self::Text
        }
    }
}

impl Display for FileType {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        match self {
            Self::Rust => write!(formatter, "Rust"),
            Self::JavaScript => write!(formatter, "JavaScript"),
            Self::Zig => write!(formatter, "Zig"),
            Self::Text => write!(formatter, "Text"),
        }
    }
}
