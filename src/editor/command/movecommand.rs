use crossterm::event::{
    KeyCode::{Down, End, Home, Left, PageDown, PageUp, Right, Up},
    KeyEvent, KeyModifiers,
};

#[derive(Clone, Copy)]
pub enum Move {
    HalfPageUp,
    HalfPageDown,
    PageUp,
    PageDown,
    ViewTop,
    ViewBottom,
    ViewCenter,
    StartOfLine,
    FirstNonWhitespace,
    EndOfLine,
    AfterEndOfLine,
    BufferStart,
    BufferEnd,
    GoToLine(usize),
    Up(usize),
    Left(usize),
    Right(usize),
    Down(usize),
}
impl TryFrom<KeyEvent> for Move {
    type Error = String;
    fn try_from(event: KeyEvent) -> Result<Self, Self::Error> {
        let KeyEvent {
            code, modifiers, ..
        } = event;

        if modifiers == KeyModifiers::NONE {
            match code {
                Up => Ok(Self::Up(1)),
                Down => Ok(Self::Down(1)),
                Left => Ok(Self::Left(1)),
                Right => Ok(Self::Right(1)),
                PageDown => Ok(Self::PageDown),
                PageUp => Ok(Self::PageUp),
                Home => Ok(Self::StartOfLine),
                End => Ok(Self::EndOfLine),
                _ => Err(format!("Unsupported code: {code:?}")),
            }
        } else {
            Err(format!(
                "Unsupported key code {code:?} or modifier {modifiers:?}"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_arrows() {
        let event = KeyEvent::new(Up, KeyModifiers::NONE);
        let mv = Move::try_from(event).unwrap();
        assert!(matches!(mv, Move::Up(1)));

        let event = KeyEvent::new(Down, KeyModifiers::NONE);
        let mv = Move::try_from(event).unwrap();
        assert!(matches!(mv, Move::Down(1)));

        let event = KeyEvent::new(Left, KeyModifiers::NONE);
        let mv = Move::try_from(event).unwrap();
        assert!(matches!(mv, Move::Left(1)));

        let event = KeyEvent::new(Right, KeyModifiers::NONE);
        let mv = Move::try_from(event).unwrap();
        assert!(matches!(mv, Move::Right(1)));
    }

    #[test]
    fn test_parse_home_end() {
        let event = KeyEvent::new(Home, KeyModifiers::NONE);
        let mv = Move::try_from(event).unwrap();
        assert!(matches!(mv, Move::StartOfLine));

        let event = KeyEvent::new(End, KeyModifiers::NONE);
        let mv = Move::try_from(event).unwrap();
        assert!(matches!(mv, Move::EndOfLine));
    }
}
