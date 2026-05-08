use std::fmt::{Display, Formatter, Result};

#[derive(Default, Eq, PartialEq, Hash, Debug, Copy, Clone)]
pub enum FileType {
    Rust,
    JavaScript,
    Zig,
    #[default]
    Text,
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
