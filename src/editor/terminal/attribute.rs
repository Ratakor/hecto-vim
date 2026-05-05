use crossterm::style::Color;

use super::super::AnnotationType;

pub struct Attribute {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
}

impl From<AnnotationType> for Attribute {
    #[allow(clippy::too_many_lines)]
    fn from(annotation_type: AnnotationType) -> Self {
        match annotation_type {
            AnnotationType::Match => Self {
                foreground: Some(Color::Rgb {
                    r: 251,
                    g: 241,
                    b: 199,
                }),
                background: Some(Color::Rgb {
                    r: 146,
                    g: 131,
                    b: 116,
                }),
            },
            AnnotationType::SelectedMatch => Self {
                foreground: Some(Color::Rgb {
                    r: 40,
                    g: 40,
                    b: 40,
                }),
                background: Some(Color::Rgb {
                    r: 250,
                    g: 189,
                    b: 47,
                }),
            },
            AnnotationType::Number => Self {
                foreground: Some(Color::Rgb {
                    r: 211,
                    g: 134,
                    b: 155,
                }),
                background: None,
            },
            AnnotationType::Keyword => Self {
                foreground: Some(Color::Rgb {
                    r: 251,
                    g: 73,
                    b: 52,
                }),
                background: None,
            },
            AnnotationType::Type => Self {
                foreground: Some(Color::Rgb {
                    r: 250,
                    g: 189,
                    b: 47,
                }),
                background: None,
            },
            AnnotationType::KnownValue => Self {
                foreground: Some(Color::Rgb {
                    r: 254,
                    g: 128,
                    b: 25,
                }),
                background: None,
            },
            AnnotationType::Char => Self {
                foreground: Some(Color::Rgb {
                    r: 184,
                    g: 187,
                    b: 38,
                }),
                background: None,
            },
            AnnotationType::LifetimeSpecifier => Self {
                foreground: Some(Color::Rgb {
                    r: 254,
                    g: 128,
                    b: 25,
                }),
                background: None,
            },
            AnnotationType::Comment => Self {
                foreground: Some(Color::Rgb {
                    r: 146,
                    g: 131,
                    b: 116,
                }),
                background: None,
            },
            AnnotationType::String => Self {
                foreground: Some(Color::Rgb {
                    r: 184,
                    g: 187,
                    b: 38,
                }),
                background: None,
            },
            AnnotationType::Variable => Self {
                foreground: Some(Color::Rgb {
                    r: 251,
                    g: 241,
                    b: 199,
                }),
                background: None,
            },
            AnnotationType::Function => Self {
                foreground: Some(Color::Rgb {
                    r: 184,
                    g: 187,
                    b: 38,
                }),
                background: None,
            },
            AnnotationType::Method => Self {
                foreground: Some(Color::Rgb {
                    r: 184,
                    g: 187,
                    b: 38,
                }),
                background: None,
            },
            AnnotationType::Operator => Self {
                foreground: Some(Color::Rgb {
                    r: 251,
                    g: 241,
                    b: 199,
                }),
                background: None,
            },
            AnnotationType::Punctuation => Self {
                foreground: Some(Color::Rgb {
                    r: 251,
                    g: 241,
                    b: 199,
                }),
                background: None,
            },
            AnnotationType::Property => Self {
                foreground: Some(Color::Rgb {
                    r: 131,
                    g: 165,
                    b: 152,
                }),
                background: None,
            },
            AnnotationType::Constant => Self {
                foreground: Some(Color::Rgb {
                    r: 211,
                    g: 134,
                    b: 155,
                }),
                background: None,
            },
            AnnotationType::Boolean => Self {
                foreground: Some(Color::Rgb {
                    r: 211,
                    g: 134,
                    b: 155,
                }),
                background: None,
            },
            AnnotationType::Macro => Self {
                foreground: Some(Color::Rgb {
                    r: 142,
                    g: 192,
                    b: 124,
                }),
                background: None,
            },
            AnnotationType::Attribute => Self {
                foreground: Some(Color::Rgb {
                    r: 142,
                    g: 192,
                    b: 124,
                }),
                background: None,
            },
            AnnotationType::Selection => Self {
                foreground: Some(Color::Rgb {
                    r: 40,
                    g: 40,
                    b: 40,
                }),
                background: Some(Color::Rgb {
                    r: 213,
                    g: 196,
                    b: 161,
                }),
            },
        }
    }
}
