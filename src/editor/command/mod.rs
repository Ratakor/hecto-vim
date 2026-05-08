use crossterm::event::KeyEvent;
use std::convert::TryFrom;
mod movecommand;
pub use movecommand::Move;
mod system;
pub use system::System;
mod edit;
pub use edit::Edit;

#[derive(Clone, Copy)]
pub enum Command {
    Move(Move),
    Edit(Edit),
    System(System),
}

impl TryFrom<KeyEvent> for Command {
    type Error = String;
    fn try_from(key_event: KeyEvent) -> Result<Self, Self::Error> {
        Edit::try_from(key_event)
            .map(Command::Edit)
            .or_else(|_| Move::try_from(key_event).map(Command::Move))
            .or_else(|_| System::try_from(key_event).map(Command::System))
            .map_err(|_err| format!("Key event not supported: {key_event:?}"))
    }
}
