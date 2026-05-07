use super::UIComponent;
use crate::editor::Terminal;
use crate::prelude::*;
use crossterm::style::Color;
use std::io::Error;

#[derive(Clone, Copy, PartialEq)]
pub enum ContextMenuAction {
    Copy,
    Delete,
    Paste,
    Undo,
    Redo,
    SelectAll,
}

pub struct ContextMenu {
    position: Position,
    needs_redraw: bool,
    size: Size,
    hovered_action: Option<ContextMenuAction>,
    available_actions: Vec<(ContextMenuAction, String)>,
}

impl ContextMenu {
    pub fn new(
        mut position: Position,
        terminal_size: Size,
        can_undo: bool,
        can_redo: bool,
        has_selection: bool,
        can_paste: bool,
    ) -> Self {
        let mut raw_actions = Vec::new();
        if has_selection {
            raw_actions.push((ContextMenuAction::Copy, "Copy"));
        }
        if can_paste {
            raw_actions.push((ContextMenuAction::Paste, "Paste"));
        }
        if has_selection {
            raw_actions.push((ContextMenuAction::Delete, "Delete"));
        }
        raw_actions.push((ContextMenuAction::SelectAll, "Select All"));

        if can_undo {
            raw_actions.push((ContextMenuAction::Undo, "Undo"));
        }
        if can_redo {
            raw_actions.push((ContextMenuAction::Redo, "Redo"));
        }

        let max_label_width = raw_actions
            .iter()
            .map(|(_, label)| label.len())
            .max()
            .unwrap_or(0);

        // Add padding: 1 left, 1 right
        let width = max_label_width.saturating_add(2);

        let available_actions: Vec<(ContextMenuAction, String)> = raw_actions
            .into_iter()
            .map(|(action, label)| {
                (
                    action,
                    format!(" {:<width$} ", label, width = max_label_width),
                )
            })
            .collect();

        let size = Size {
            width,
            height: available_actions.len(),
        };

        // Adjust position if it would go out of bounds
        if position.col + size.width > terminal_size.width {
            position.col = terminal_size.width.saturating_sub(size.width);
        }
        if position.row + size.height > terminal_size.height {
            position.row = terminal_size.height.saturating_sub(size.height);
        }

        Self {
            position,
            needs_redraw: true,
            size,
            hovered_action: None,
            available_actions,
        }
    }

    pub fn handle_mouse_move(&mut self, mouse_pos: Position) {
        let new_hover = self.action_at(mouse_pos);
        if new_hover != self.hovered_action {
            self.hovered_action = new_hover;
            self.needs_redraw = true;
        }
    }

    pub fn handle_click(&self, mouse_pos: Position) -> Option<ContextMenuAction> {
        self.action_at(mouse_pos)
    }

    pub fn position(&self) -> Position {
        self.position
    }

    fn action_at(&self, mouse_pos: Position) -> Option<ContextMenuAction> {
        if mouse_pos.col >= self.position.col
            && mouse_pos.col < self.position.col + self.size.width
            && mouse_pos.row >= self.position.row
            && mouse_pos.row < self.position.row + self.size.height
        {
            let relative_row = mouse_pos.row.saturating_sub(self.position.row);
            if relative_row < self.available_actions.len() {
                return Some(self.available_actions[relative_row].0);
            }
        }
        None
    }
}

impl UIComponent for ContextMenu {
    fn set_needs_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    fn draw(&mut self, _origin_row: RowIdx) -> Result<(), Error> {
        let col = self.position.col;
        let row = self.position.row;

        for (i, (action, label)) in self.available_actions.iter().enumerate() {
            if self.hovered_action == Some(*action) {
                Terminal::set_background_color(Color::Grey)?;
            } else {
                Terminal::set_background_color(Color::DarkGrey)?;
            }
            Terminal::print_row_at_no_clear(row + i, col, label)?;
            Terminal::reset_attributes()?;
        }

        Ok(())
    }
}
