use crossterm::style::Color;

use super::super::AnnotationType;

pub struct Attribute {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
}

impl From<AnnotationType> for Attribute {
    fn from(annotation_type: AnnotationType) -> Self {
        match annotation_type {
            AnnotationType::Match => Self {
                foreground: Some(Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                }),
                background: Some(Color::Rgb {
                    r: 100,
                    g: 100,
                    b: 100,
                }),
            },
            AnnotationType::SelectedMatch => Self {
                foreground: Some(Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                }),
                background: Some(Color::Rgb {
                    r: 153,
                    g: 153,
                    b: 0,
                }),
            },
            AnnotationType::Number => Self {
                foreground: Some(Color::Rgb {
                    r: 255,
                    g: 165,
                    b: 0,
                }),
                background: None,
            },
            AnnotationType::Keyword => Self {
                foreground: Some(Color::Rgb {
                    r: 135,
                    g: 206,
                    b: 250,
                }),
                background: None,
            },
            AnnotationType::Type => Self {
                foreground: Some(Color::Rgb {
                    r: 144,
                    g: 238,
                    b: 144,
                }),
                background: None,
            },
            AnnotationType::KnownValue => Self {
                foreground: Some(Color::Rgb {
                    r: 216,
                    g: 191,
                    b: 216,
                }),
                background: None,
            },
            AnnotationType::Char => Self {
                foreground: Some(Color::Rgb {
                    r: 255,
                    g: 215,
                    b: 0,
                }),
                background: None,
            },
            AnnotationType::LifetimeSpecifier => Self {
                foreground: Some(Color::Rgb {
                    r: 250,
                    g: 128,
                    b: 114,
                }),
                background: None,
            },
            AnnotationType::Comment => Self {
                foreground: Some(Color::Rgb {
                    r: 105,
                    g: 105,
                    b: 105,
                }),
                background: None,
            },
            AnnotationType::String => Self {
                foreground: Some(Color::Rgb {
                    r: 244,
                    g: 164,
                    b: 96,
                }),
                background: None,
            },
            AnnotationType::Variable => Self {
                foreground: Some(Color::Rgb {
                    r: 240,
                    g: 230,
                    b: 140,
                }),
                background: None,
            },
            AnnotationType::Function => Self {
                foreground: Some(Color::Rgb {
                    r: 173,
                    g: 216,
                    b: 230,
                }),
                background: None,
            },
            AnnotationType::Method => Self {
                foreground: Some(Color::Rgb {
                    r: 176,
                    g: 224,
                    b: 230,
                }),
                background: None,
            },
            AnnotationType::Operator => Self {
                foreground: Some(Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 224,
                }),
                background: None,
            },
            AnnotationType::Punctuation => Self {
                foreground: Some(Color::Rgb {
                    r: 211,
                    g: 211,
                    b: 211,
                }),
                background: None,
            },
            AnnotationType::Property => Self {
                foreground: Some(Color::Rgb {
                    r: 238,
                    g: 232,
                    b: 170,
                }),
                background: None,
            },
            AnnotationType::Constant => Self {
                foreground: Some(Color::Rgb {
                    r: 255,
                    g: 140,
                    b: 0,
                }),
                background: None,
            },
            AnnotationType::Boolean => Self {
                foreground: Some(Color::Rgb {
                    r: 255,
                    g: 99,
                    b: 71,
                }),
                background: None,
            },
            AnnotationType::Macro => Self {
                foreground: Some(Color::Rgb {
                    r: 255,
                    g: 105,
                    b: 180,
                }),
                background: None,
            },
            AnnotationType::Attribute => Self {
                foreground: Some(Color::Rgb {
                    r: 221,
                    g: 160,
                    b: 221,
                }),
                background: None,
            },
        }
    }
}
