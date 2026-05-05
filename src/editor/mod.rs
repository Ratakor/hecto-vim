use crate::prelude::*;
use crossterm::event::{
    read, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use std::{
    env,
    io::Error,
    panic::{set_hook, take_hook},
};
mod annotatedstring;
pub mod annotationtype;
mod command;
mod documentstatus;
mod line;
mod terminal;
mod uicomponents;
pub use annotationtype::AnnotationType;
mod annotation;
use annotation::Annotation;
mod filetype;
use annotatedstring::AnnotatedString;
use documentstatus::DocumentStatus;
use filetype::FileType;
use line::Line;
use terminal::Terminal;
use uicomponents::{CommandBar, MessageBar, StatusBar, UIComponent, View};

use self::command::{
    Command,
    Edit,
    Move,
    System,
};

const QUIT_TIMES: u8 = 3;

#[derive(Eq, PartialEq, Default, Clone, Copy)]
pub enum EditorMode {
    #[default]
    Normal,
    Insert,
}

#[derive(Eq, PartialEq, Default)]
enum PromptType {
    Search,
    Save,
    Command,
    #[default]
    None,
}

impl PromptType {
    fn is_none(&self) -> bool {
        *self == Self::None
    }
}

#[derive(Default)]
pub struct Editor {
    should_quit: bool,
    view: View,
    status_bar: StatusBar,
    message_bar: MessageBar,
    command_bar: CommandBar,
    prompt_type: PromptType,
    mode: EditorMode,
    terminal_size: Size,
    title: String,
    quit_times: u8,
}

impl Editor {
    // region: struct lifecycle

    pub fn new() -> Result<Self, Error> {
        let current_hook = take_hook();
        set_hook(Box::new(move |panic_info| {
            let _ = Terminal::terminate();
            current_hook(panic_info);
        }));
        Terminal::initialize()?;

        let mut editor = Self::default();
        let size = Terminal::size().unwrap_or_default();
        editor.handle_resize_command(size);
        editor.update_message("HELP: i = insert | :w = save | :q = quit | / = search");

        let args: Vec<String> = env::args().collect();
        if let Some(file_name) = args.get(1) {
            debug_assert!(!file_name.is_empty());
            if editor.view.load(file_name).is_err() {
                editor.update_message(&format!("ERR: Could not open file: {file_name}"));
            }
        }
        editor.refresh_status();
        Ok(editor)
    }

    // endregion

    // region: Event Loop
    pub fn run(&mut self) {
        loop {
            self.refresh_screen();
            if self.should_quit {
                break;
            }
            match read() {
                Ok(event) => self.evaluate_event(event),
                Err(err) => {
                    #[cfg(debug_assertions)]
                    {
                        panic!("Could not read event: {err:?}");
                    }
                    #[cfg(not(debug_assertions))]
                    {
                        let _ = err;
                    }
                }
            }
            self.refresh_status();
        }
    }

    fn refresh_screen(&mut self) {
        if self.terminal_size.height == 0 || self.terminal_size.width == 0 {
            return;
        }
        let bottom_bar_row = self.terminal_size.height.saturating_sub(1);
        let _ = Terminal::hide_caret();
        if self.in_prompt() {
            self.command_bar.render(bottom_bar_row);
        } else {
            self.message_bar.render(bottom_bar_row);
        }
        if self.terminal_size.height > 1 {
            self.status_bar
                .render(self.terminal_size.height.saturating_sub(2));
        }
        if self.terminal_size.height > 2 {
            self.view.render(0);
        }
        let new_caret_pos = if self.in_prompt() {
            Position {
                row: bottom_bar_row,
                col: self.command_bar.caret_position_col(),
            }
        } else {
            self.view.caret_position()
        };
        debug_assert!(new_caret_pos.col <= self.terminal_size.width);
        debug_assert!(new_caret_pos.row <= self.terminal_size.height);

        let _ = Terminal::move_caret_to(new_caret_pos);
        let _ = Terminal::show_caret();
        let _ = Terminal::execute();
    }

    fn refresh_status(&mut self) {
        let status = self.view.get_status();
        let title = format!("{} - {NAME}", status.file_name);
        self.status_bar.update_status(status);
        if title != self.title && matches!(Terminal::set_title(&title), Ok(())) {
            self.title = title;
        }
    }

    fn evaluate_event(&mut self, event: Event) {
        let should_process = match &event {
            Event::Key(KeyEvent { kind, .. }) => kind == &KeyEventKind::Press,
            Event::Resize(_, _) => true,
            Event::Mouse(_) => true,
            _ => false,
        };

        if !should_process {
            return;
        }

        if let Event::Resize(width_u16, height_u16) = event {
            self.process_command(Command::System(System::Resize(Size {
                height: height_u16 as usize,
                width: width_u16 as usize,
            })));
            return;
        }

        if let Event::Mouse(mouse_event) = event {
            self.handle_mouse_event(mouse_event);
            return;
        }

        if let Event::Key(key_event) = event {
            if !self.in_prompt() {
                match self.mode {
                    EditorMode::Normal => {
                        match (key_event.code, key_event.modifiers) {
                            (KeyCode::Char('i'), KeyModifiers::NONE) => {
                                self.mode = EditorMode::Insert;
                                self.update_message("-- INSERT --");
                            }
                            (KeyCode::Char(':'), KeyModifiers::NONE) => {
                                self.set_prompt(PromptType::Command);
                            }
                            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                                self.set_prompt(PromptType::Search);
                            }
                            (KeyCode::Char('h'), KeyModifiers::NONE) => {
                                self.process_command(Command::Move(Move::Left));
                            }
                            (KeyCode::Char('j'), KeyModifiers::NONE) => {
                                self.process_command(Command::Move(Move::Down));
                            }
                            (KeyCode::Char('k'), KeyModifiers::NONE) => {
                                self.process_command(Command::Move(Move::Up));
                            }
                            (KeyCode::Char('l'), KeyModifiers::NONE) => {
                                self.process_command(Command::Move(Move::Right));
                            }
                            _ => {
                                if let Ok(command) = Command::try_from(event) {
                                    if !matches!(command, Command::Edit(_)) {
                                        self.process_command(command);
                                    }
                                }
                            }
                        }
                        return;
                    }
                    EditorMode::Insert => {
                        if key_event.code == KeyCode::Esc {
                            self.mode = EditorMode::Normal;
                            self.update_message("");
                            return;
                        }
                    }
                }
            }
            if let Ok(command) = Command::try_from(event) {
                self.process_command(command);
            }
        }
    }

    fn handle_mouse_event(&mut self, event: MouseEvent) {
        let MouseEvent {
            kind, column, row, ..
        } = event;
        let mouse_pos = Position {
            col: column as usize,
            row: row as usize,
        };
        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if mouse_pos.row < self.terminal_size.height.saturating_sub(2) {
                    self.view.move_to_position(mouse_pos);
                }
            }
            MouseEventKind::ScrollUp => {
                self.view.handle_move_command(Move::Up);
            }
            MouseEventKind::ScrollDown => {
                self.view.handle_move_command(Move::Down);
            }
            _ => {}
        }
    }
    // endregion

    // region command handling

    fn process_command(&mut self, command: Command) {
        if let Command::System(System::Resize(size)) = command {
            self.handle_resize_command(size);
            return;
        }
        match self.prompt_type {
            PromptType::Search => self.process_command_during_search(command),
            PromptType::Save => self.process_command_during_save(command),
            PromptType::Command => self.process_command_during_command(command),
            PromptType::None => self.process_command_no_prompt(command),
        }
    }

    fn process_command_no_prompt(&mut self, command: Command) {
        if matches!(command, Command::System(System::Quit)) {
            self.handle_quit_command();
            return;
        }
        self.reset_quit_times(); // Reset quit times for all other commands

        match command {
            Command::System(System::Quit | System::Resize(_)) => {} // Quit and Resize already handled above, others not applicable
            Command::System(System::Dismiss) => self.update_message(""),
            Command::System(System::Search) => self.set_prompt(PromptType::Search),
            Command::System(System::Save) => self.handle_save_command(),
            Command::Edit(edit_command) => self.view.handle_edit_command(edit_command),
            Command::Move(move_command) => self.view.handle_move_command(move_command),
        }
    }

    fn process_command_during_command(&mut self, command: Command) {
        match command {
            Command::System(System::Dismiss) => {
                self.set_prompt(PromptType::None);
            }
            Command::Edit(Edit::InsertNewline) => {
                let command_str = self.command_bar.value();
                self.handle_vim_command(&command_str);
                if !self.should_quit {
                    self.set_prompt(PromptType::None);
                }
            }
            Command::Edit(edit_command) => self.command_bar.handle_edit_command(edit_command),
            _ => {}
        }
    }

    fn handle_vim_command(&mut self, command: &str) {
        match command {
            "q" => self.handle_quit_command(),
            "q!" => self.should_quit = true,
            "w" => self.handle_save_command(),
            "wq" | "x" => {
                self.save(None);
                if !self.view.get_status().is_modified {
                    self.should_quit = true;
                }
            }
            _ => self.update_message(&format!("ERR: Unknown command: {command}")),
        }
    }

    // region resize command handling

    fn handle_resize_command(&mut self, size: Size) {
        self.terminal_size = size;
        self.view.resize(Size {
            height: size.height.saturating_sub(2),
            width: size.width,
        });
        let bar_size = Size {
            height: 1,
            width: size.width,
        };
        self.message_bar.resize(bar_size);
        self.status_bar.resize(bar_size);
        self.command_bar.resize(bar_size);
    }

    // endregion

    // region quit command handling

    // clippy::arithmetic_side_effects: quit_times is guaranteed to be between 0 and QUIT_TIMES
    #[allow(clippy::arithmetic_side_effects)]
    fn handle_quit_command(&mut self) {
        if !self.view.get_status().is_modified || self.quit_times + 1 == QUIT_TIMES {
            self.should_quit = true;
        } else if self.view.get_status().is_modified {
            self.update_message(&format!(
                "WARNING! File has unsaved changes. Type :q {} more times to quit.",
                QUIT_TIMES - self.quit_times - 1
            ));

            self.quit_times += 1;
        }
    }
    fn reset_quit_times(&mut self) {
        if self.quit_times > 0 {
            self.quit_times = 0;
            self.update_message("");
        }
    }
    // end region

    // region save command & prompt handling

    fn handle_save_command(&mut self) {
        if self.view.is_file_loaded() {
            self.save(None);
        } else {
            self.set_prompt(PromptType::Save);
        }
    }
    fn process_command_during_save(&mut self, command: Command) {
        match command {
            Command::System(System::Quit | System::Resize(_) | System::Search | System::Save) | Command::Move(_) => {} // Not applicable during save, Resize already handled at this stage
            Command::System(System::Dismiss) => {
                self.set_prompt(PromptType::None);
                self.update_message("Save aborted.");
            }
            Command::Edit(Edit::InsertNewline) => {
                let file_name = self.command_bar.value();
                self.save(Some(&file_name));
                self.set_prompt(PromptType::None);
            }
            Command::Edit(edit_command) => self.command_bar.handle_edit_command(edit_command),
        }
    }
    fn save(&mut self, file_name: Option<&str>) {
        let result = if let Some(name) = file_name {
            self.view.save_as(name)
        } else {
            self.view.save()
        };
        if result.is_ok() {
            self.update_message("File saved successfully.");
        } else {
            self.update_message("Error writing file!");
        }
    }

    // endregion

    // region search command & prompt handling
    fn process_command_during_search(&mut self, command: Command) {
        match command {
            Command::System(System::Dismiss) => {
                self.set_prompt(PromptType::None);
                self.view.dismiss_search();
            }
            Command::Edit(Edit::InsertNewline) => {
                self.set_prompt(PromptType::None);
                self.view.exit_search();
            }
            Command::Edit(edit_command) => {
                self.command_bar.handle_edit_command(edit_command);
                let query = self.command_bar.value();
                self.view.search(&query);
            }
            Command::Move(Move::Right | Move::Down) => self.view.search_next(),
            Command::Move(Move::Up | Move::Left) => self.view.search_prev(),
            Command::System(System::Quit | System::Resize(_) | System::Search | System::Save) | Command::Move(_) => {} // Not applicable during save, Resize already handled at this stage
        }
    }
    // endregion

    // region message & command bar
    fn update_message(&mut self, new_message: &str) {
        self.message_bar.update_message(new_message);
    }
    // endregion

    //region prompt handling
    fn in_prompt(&self) -> bool {
        !self.prompt_type.is_none()
    }

    fn set_prompt(&mut self, prompt_type: PromptType) {
        match prompt_type {
            PromptType::None => self.message_bar.set_needs_redraw(true), //Ensures the message bar is properly painted during the next redraw cycle
            PromptType::Save => self.command_bar.set_prompt("Save as: "),
            PromptType::Command => self.command_bar.set_prompt(":"),
            PromptType::Search => {
                self.view.enter_search();
                self.command_bar
                    .set_prompt("Search (Esc to cancel, Arrows to navigate): ");
            }
        }
        self.command_bar.clear_value();
        self.prompt_type = prompt_type;
    }
    // end region
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = Terminal::terminate();
        if self.should_quit {
            let _ = Terminal::print("Goodbye.\r\n");
        }
    }
}
