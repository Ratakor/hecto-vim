use crossterm::event::{
    KeyCode::{Backspace, Char, Delete, Enter, Tab},
    KeyEvent, KeyModifiers,
};
#[derive(Clone, Copy)]
pub enum Edit {
    Insert(char),
    InsertNewline,
    Delete,
    DeleteBackward,
    Undo,
    Redo,
}
impl TryFrom<KeyEvent> for Edit {
    type Error = String;

    fn try_from(event: KeyEvent) -> Result<Self, Self::Error> {
        match (event.code, event.modifiers) {
            (Char(character), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                Ok(Self::Insert(character))
            }
            (Tab, KeyModifiers::NONE) => Ok(Self::Insert('\t')),
            (Enter, KeyModifiers::NONE) => Ok(Self::InsertNewline),
            (Backspace, KeyModifiers::NONE) => Ok(Self::DeleteBackward),
            (Delete, KeyModifiers::NONE) => Ok(Self::Delete),
            _ => Err(format!(
                "Unsupported key code {:?} with modifiers {:?}",
                event.code, event.modifiers
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_char() {
        let event = KeyEvent::new(Char('a'), KeyModifiers::NONE);
        let edit = Edit::try_from(event).unwrap();
        if let Edit::Insert(c) = edit {
            assert_eq!(c, 'a');
        } else {
            panic!("Expected Insert");
        }
    }

    #[test]
    fn test_parse_backspace() {
        let event = KeyEvent::new(Backspace, KeyModifiers::NONE);
        let edit = Edit::try_from(event).unwrap();
        assert!(matches!(edit, Edit::DeleteBackward));
    }

    #[test]
    fn test_parse_unsupported() {
        let event = KeyEvent::new(Char('a'), KeyModifiers::CONTROL);
        assert!(Edit::try_from(event).is_err());
    }
}
