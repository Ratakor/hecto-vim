use std::{
    io::Error,
    time::{Duration, Instant},
};

use crate::prelude::*;

use super::super::Terminal;
use super::UIComponent;

const DEFAULT_DURATION: Duration = Duration::new(5, 0);

struct Message {
    text: String,
    time: Instant,
}

impl Message {
    fn is_expired(&self) -> bool {
        self.time.elapsed() > DEFAULT_DURATION
    }
}

#[derive(Default)]
pub struct MessageBar {
    current_message: Option<Message>,
    command_buffer: String,
    needs_redraw: bool,
    cleared_after_expiry: bool,
    size: Size,
}

impl MessageBar {
    pub fn update_message(&mut self, text: &str) {
        self.current_message = Some(Message {
            text: text.to_string(),
            time: Instant::now(),
        });
        self.cleared_after_expiry = false;
        self.set_needs_redraw(true);
    }

    pub fn update_command_buffer(&mut self, new_buffer: &str) {
        if new_buffer != self.command_buffer {
            self.command_buffer = new_buffer.to_string();
            self.set_needs_redraw(true);
        }
    }
}

impl UIComponent for MessageBar {
    fn set_needs_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
            || self
                .current_message
                .as_ref()
                .map_or(false, |m| !self.cleared_after_expiry && m.is_expired())
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    fn draw(&mut self, origin: RowIdx) -> Result<(), Error> {
        let message_text = match &self.current_message {
            Some(m) if m.is_expired() => {
                self.cleared_after_expiry = true;
                ""
            }
            Some(m) => &m.text,
            None => "",
        };

        let target_pos = self.size.width.saturating_sub(15);
        let mut row = format!("{:<width$}", message_text, width = target_pos);
        if row.len() > target_pos {
            row.truncate(target_pos.saturating_sub(1));
            row.push(' ');
        }
        row.push_str(&self.command_buffer);

        Terminal::print_row(origin, &row)
    }
}
