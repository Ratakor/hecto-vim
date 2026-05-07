use crate::prelude::*;
use arboard::Clipboard;
use crossterm::{
    cursor::SetCursorStyle,
    event::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind, poll, read,
    },
};
use std::{
    env,
    io::Error,
    panic::{set_hook, take_hook},
    time::Duration,
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
use uicomponents::{
    CommandBar, ContextMenu, ContextMenuAction, MessageBar, StatusBar, UIComponent, View,
};

use self::command::{Command, Edit, Move, System};

const QUIT_TIMES: u8 = 3;

#[derive(Eq, PartialEq, Default, Clone, Copy, Debug)]
pub enum EditorMode {
    #[default]
    Normal,
    Insert,
    Visual,
    Help,
}

impl std::fmt::Display for EditorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "NORMAL"),
            Self::Insert => write!(f, "INSERT"),
            Self::Visual => write!(f, "VISUAL"),
            Self::Help => write!(f, "HELP"),
        }
    }
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
    move_left_on_escape: bool,
    clipboard: String,
    system_clipboard: Clipboard,
    command_buffer: Vec<KeyEvent>,
    count: Option<usize>,
    context_menu: Option<ContextMenu>,
}

impl Editor {
    // region: struct lifecycle

    pub fn new() -> Result<Self, Error> {
        let current_hook = take_hook();
        set_hook(Box::new(move |panic_info| {
            #[cfg(not(debug_assertions))]
            {
                let _ = Terminal::terminate();
            }
            current_hook(panic_info);
        }));
        Terminal::initialize()?;

        let size = Terminal::size().unwrap_or_default();
        let mut editor = Self {
            should_quit: false,
            view: View::default(),
            status_bar: StatusBar::default(),
            message_bar: MessageBar::default(),
            command_bar: CommandBar::default(),
            prompt_type: PromptType::default(),
            mode: EditorMode::default(),
            terminal_size: size,
            title: String::new(),
            quit_times: 0,
            move_left_on_escape: false,
            clipboard: String::new(),
            system_clipboard: Clipboard::new()
                .map_err(|e| Error::new(std::io::ErrorKind::Other, e))?,
            command_buffer: Vec::new(),
            count: None,
            context_menu: None,
        };
        editor.handle_resize_command(size);

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
            match poll(Duration::from_millis(100)) {
                Ok(true) => match read() {
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
                },
                Ok(false) => {}
                Err(err) => {
                    #[cfg(debug_assertions)]
                    {
                        panic!("Could not poll event: {err:?}");
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
        if self.mode == EditorMode::Help {
            let _ = self.draw_help();
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
            if self.context_menu.is_some() {
                self.view.set_needs_redraw(true);
            }
            self.view.render(0);
        }
        if let Some(menu) = &mut self.context_menu {
            menu.set_needs_redraw(true);
            menu.render(0);
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
        let mut command_buffer = self
            .count
            .map(|c| c.to_string())
            .unwrap_or_else(String::new);
        command_buffer.push_str(
            &self
                .command_buffer
                .iter()
                .map(|key| self.key_event_to_string(*key))
                .collect::<String>(),
        );
        self.message_bar.update_command_buffer(&command_buffer);

        let status = self.view.get_status(self.mode.to_string());
        let title = format!("{} - {NAME}", status.file_name);
        self.status_bar.update_status(status);
        if title != self.title && matches!(Terminal::set_title(&title), Ok(())) {
            self.title = title;
        }
    }

    fn key_event_to_string(&self, key: KeyEvent) -> String {
        match key.code {
            KeyCode::Char(' ') => "<space>".to_string(),
            KeyCode::Char(c) => c.to_string(),
            _ => String::new(),
        }
    }

    fn draw_help(&mut self) -> Result<(), Error> {
        let _ = Terminal::clear_screen();
        let _ = Terminal::move_caret_to(Position::default());
        let help_text = vec![
            "HELP - ALL COMMANDS (Press any key to exit)",
            "This may be out of date",
            "",
            "[Movement]",
            "  h, j, k, l : Left, Down, Up, Right",
            "  C-u, C-d   : Half page Up/Down",
            "  gg / XXXg  : Buffer Start / Go to line XXX",
            "  g          : Enter Goto mode",
            "  g/e        : Buffer Start/End",
            "  h/l        : Line Start/End",
            "  s          : First non-whitespace",
            "  t/b/c      : View Top/Bottom/Center",
            "",
            "[Editing]",
            "  i          : Insert mode",
            "  a          : Append (Right + Insert)",
            "  o, O       : Open line below/above + Insert",
            "  r          : Replace character",
            "  u, U       : Undo, Redo",
            "  p          : Paste clipboard",
            "  SPC p/P    : Paste from system clipboard",
            "  d          : Delete selection & copy to clipboard",
            "  SPC d      : Delete selection & copy to system clipboard",
            "",
            "[Selection & Visual]",
            "  v          : Toggle Visual mode",
            "  x          : Select whole line",
            "  y          : Copy (yank) selection",
            "  SPC y      : Copy (yank) selection to system clipboard",
            "",
            "[Search & Commands]",
            "  /          : Search",
            "  :          : Command mode",
            "  :w [path]  : Save",
            "  :q, :q!    : Quit, Force quit",
            "  :wq, :x    : Save and quit",
            "  :syntax    : Toggle syntax highlighting",
            "  ?          : Show this help",
        ];

        for (i, line) in help_text.iter().enumerate() {
            if i < self.terminal_size.height {
                Terminal::print_row_at(i, 0, line)?;
            }
        }
        Terminal::execute()
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
                    EditorMode::Normal | EditorMode::Visual => {
                        self.command_buffer.push(key_event);
                        if self.handle_key_sequence() {
                            self.command_buffer.clear();
                        }
                        return;
                    }
                    EditorMode::Insert => {
                        if key_event.code == KeyCode::Esc {
                            self.mode = EditorMode::Normal;
                            let _ = Terminal::set_cursor_style(SetCursorStyle::SteadyBlock);
                            if self.move_left_on_escape {
                                self.process_command(Command::Move(Move::Left(1)));
                                self.move_left_on_escape = false;
                            }
                            self.update_message("");
                            return;
                        }
                    }
                    EditorMode::Help => {
                        self.mode = EditorMode::Normal;
                        self.view.set_needs_redraw(true);
                        self.status_bar.set_needs_redraw(true);
                        self.message_bar.set_needs_redraw(true);
                        return;
                    }
                }
            }
            if let Ok(command) = Command::try_from(event) {
                self.process_command(command);
            }
        }
    }

    fn handle_key_sequence(&mut self) -> bool {
        if self.command_buffer.is_empty() {
            return true;
        }

        let first_key = self.command_buffer[0];
        let first_code = first_key.code;
        let first_mod = first_key.modifiers;

        if self.command_buffer.len() == 1 {
            if let (KeyCode::Char(c), KeyModifiers::NONE) = (first_code, first_mod) {
                if c.is_ascii_digit() {
                    let digit = c.to_digit(10).unwrap_or(0) as usize;
                    self.count = Some(
                        self.count
                            .unwrap_or(0)
                            .saturating_mul(10)
                            .saturating_add(digit),
                    );
                    return true;
                }
            }

            match (first_code, first_mod) {
                (KeyCode::Char('g'), KeyModifiers::NONE) => {
                    if let Some(count) = self.count {
                        self.process_command(Command::Move(Move::GoToLine(count)));
                        self.count = None;
                        return true;
                    }
                    return false;
                }
                (KeyCode::Char(' '), KeyModifiers::NONE) => {
                    return false;
                }
                (KeyCode::Char('r'), KeyModifiers::NONE) => {
                    return false;
                }
                (KeyCode::Char('i'), KeyModifiers::NONE) => {
                    self.enter_insert_mode(false);
                }
                (KeyCode::Char('a'), KeyModifiers::NONE) => {
                    self.process_command(Command::Move(Move::Right(1)));
                    self.enter_insert_mode(true);
                }
                (KeyCode::Char('o'), KeyModifiers::NONE) => {
                    self.process_command(Command::Move(Move::AfterEndOfLine));
                    self.process_command(Command::Edit(Edit::InsertNewline));
                    self.enter_insert_mode(false);
                }
                (KeyCode::Char('O'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                    self.process_command(Command::Move(Move::StartOfLine));
                    self.process_command(Command::Edit(Edit::InsertNewline));
                    self.process_command(Command::Move(Move::Up(1)));
                    self.enter_insert_mode(false);
                }
                (KeyCode::Char(':'), KeyModifiers::NONE) => {
                    self.set_prompt(PromptType::Command);
                }
                (KeyCode::Char('/'), KeyModifiers::NONE) => {
                    self.set_prompt(PromptType::Search);
                }
                (KeyCode::Char('h'), KeyModifiers::NONE) => {
                    self.process_command(Command::Move(Move::Left(self.count.unwrap_or(1))));
                }
                (KeyCode::Char('j'), KeyModifiers::NONE) => {
                    self.process_command(Command::Move(Move::Down(self.count.unwrap_or(1))));
                }
                (KeyCode::Char('k'), KeyModifiers::NONE) => {
                    self.process_command(Command::Move(Move::Up(self.count.unwrap_or(1))));
                }
                (KeyCode::Char('l'), KeyModifiers::NONE) => {
                    self.process_command(Command::Move(Move::Right(self.count.unwrap_or(1))));
                }
                (KeyCode::Char('y'), KeyModifiers::NONE) => {
                    if let Some(text) = self.view.get_selected_text() {
                        self.clipboard = text;
                        self.update_message("Text copied to clipboard.");
                    } else {
                        self.clipboard = self.view.get_current_character();
                        self.update_message("Character copied to clipboard.");
                    }
                }
                (KeyCode::Char('v'), KeyModifiers::NONE) => {
                    if self.mode == EditorMode::Visual {
                        self.mode = EditorMode::Normal;
                        self.view.clear_selection();
                    } else {
                        self.mode = EditorMode::Visual;
                        if self.view.get_selection().is_none() {
                            self.view.start_selection();
                        }
                    }
                }
                (KeyCode::Char('x'), KeyModifiers::NONE) => {
                    self.view.select_line_down();
                }
                (KeyCode::Char('X'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                    self.view.select_line_up();
                }
                (KeyCode::Char('d'), KeyModifiers::NONE) => {
                    if let Some(text) = self.view.get_selected_text() {
                        self.clipboard = text;
                        self.view.delete_selection();
                        self.mode = EditorMode::Normal;
                    } else {
                        self.process_command(Command::Edit(Edit::Delete));
                    }
                }
                (KeyCode::Char('p'), KeyModifiers::NONE) => {
                    self.view.paste(&self.clipboard);
                    self.mode = EditorMode::Normal;
                }
                (KeyCode::Char('P'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                    self.view.paste_backward(&self.clipboard);
                    self.mode = EditorMode::Normal;
                }
                (KeyCode::Char('%'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                    self.view.select_all();
                }
                (KeyCode::Char('J'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                    self.view.concat_lines();
                }
                (KeyCode::Char('u'), KeyModifiers::NONE) => {
                    self.process_command(Command::Edit(Edit::Undo));
                }
                (KeyCode::Char('U'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                    self.process_command(Command::Edit(Edit::Redo));
                }
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                    self.process_command(Command::Move(Move::HalfPageUp));
                }
                (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                    self.process_command(Command::Move(Move::HalfPageDown));
                }
                (KeyCode::Char('?'), KeyModifiers::NONE) => {
                    self.mode = EditorMode::Help;
                }
                (KeyCode::Esc, KeyModifiers::NONE) => {
                    self.view.clear_selection();
                    if self.mode == EditorMode::Visual {
                        self.mode = EditorMode::Normal;
                    }
                    self.update_message("");
                }
                (KeyCode::Enter, KeyModifiers::NONE) => {
                    // Do nothing in Normal/Visual mode for Enter
                }
                _ => {}
            }
            self.count = None;
            return true;
        }

        if self.command_buffer.len() == 2 {
            self.update_message("");
            let second_key = self.command_buffer[1];
            let second_code = second_key.code;
            let second_mod = second_key.modifiers;

            if second_code == KeyCode::Esc {
                return true;
            }

            match (first_code, first_mod) {
                (KeyCode::Char('r'), KeyModifiers::NONE) => {
                    if let KeyCode::Char(c) = second_code {
                        self.process_command(Command::Edit(Edit::Replace(c)));
                    }
                    self.count = None;
                }
                (KeyCode::Char('g'), KeyModifiers::NONE) => match (second_code, second_mod) {
                    (KeyCode::Char('g'), KeyModifiers::NONE) => {
                        if let Some(count) = self.count {
                            self.process_command(Command::Move(Move::GoToLine(count)));
                        } else {
                            self.process_command(Command::Move(Move::BufferStart));
                        }
                        self.count = None;
                    }
                    (KeyCode::Char('e'), KeyModifiers::NONE) => {
                        self.process_command(Command::Move(Move::BufferEnd));
                        self.count = None;
                    }
                    (KeyCode::Char('h'), KeyModifiers::NONE) => {
                        self.process_command(Command::Move(Move::StartOfLine));
                        self.count = None;
                    }
                    (KeyCode::Char('l'), KeyModifiers::NONE) => {
                        self.process_command(Command::Move(Move::EndOfLine));
                        self.count = None;
                    }
                    (KeyCode::Char('s'), KeyModifiers::NONE) => {
                        self.process_command(Command::Move(Move::FirstNonWhitespace));
                        self.count = None;
                    }
                    (KeyCode::Char('t'), KeyModifiers::NONE) => {
                        self.process_command(Command::Move(Move::ViewTop));
                        self.count = None;
                    }
                    (KeyCode::Char('b'), KeyModifiers::NONE) => {
                        self.process_command(Command::Move(Move::ViewBottom));
                        self.count = None;
                    }
                    (KeyCode::Char('c'), KeyModifiers::NONE) => {
                        self.process_command(Command::Move(Move::ViewCenter));
                        self.count = None;
                    }
                    _ => {
                        // If unknown g- command, do nothing and clear buffer
                        self.count = None;
                    }
                },
                (KeyCode::Char(' '), KeyModifiers::NONE) => match (second_code, second_mod) {
                    (KeyCode::Char('y'), KeyModifiers::NONE) => {
                        let text = if self.view.get_selection().is_some() {
                            self.view.get_selected_text()
                        } else {
                            Some(self.view.get_current_character())
                        };

                        if let Some(text) = text {
                            if let Err(e) = self.system_clipboard.set_text(text) {
                                self.update_message(&format!("ERR: Clipboard error: {e}"));
                            } else {
                                if self.view.get_selection().is_some() {
                                    self.update_message("Text copied to system clipboard.");
                                } else {
                                    self.update_message("Character copied to system clipboard.");
                                }
                            }
                        }
                    }
                    (KeyCode::Char('p'), KeyModifiers::NONE) => {
                        if let Ok(text) = self.system_clipboard.get_text() {
                            self.view.paste(&text);
                        }
                        self.mode = EditorMode::Normal;
                    }
                    (KeyCode::Char('P'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                        if let Ok(text) = self.system_clipboard.get_text() {
                            self.view.paste_backward(&text);
                        }
                        self.mode = EditorMode::Normal;
                    }
                    (KeyCode::Char('d'), KeyModifiers::NONE) => {
                        if let Some(text) = self.view.get_selected_text() {
                            if let Err(e) = self.system_clipboard.set_text(text) {
                                self.update_message(&format!("ERR: Clipboard error: {e}"));
                            } else {
                                self.view.delete_selection();
                                self.mode = EditorMode::Normal;
                            }
                        } else {
                            self.process_command(Command::Edit(Edit::Delete));
                        }
                    }
                    _ => {
                        // If unknown SPC- command, do nothing and clear buffer
                    }
                },
                _ => {}
            }
            self.count = None;
            return true;
        }

        true
    }

    fn enter_insert_mode(&mut self, move_left_on_escape: bool) {
        self.mode = EditorMode::Insert;
        let _ = Terminal::set_cursor_style(SetCursorStyle::SteadyBar);
        self.move_left_on_escape = move_left_on_escape;
    }

    fn handle_mouse_event(&mut self, event: MouseEvent) {
        let MouseEvent {
            kind, column, row, ..
        } = event;
        let mouse_pos = Position {
            col: column as usize,
            row: row as usize,
        };

        if let Some(menu) = &mut self.context_menu {
            match kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(action) = menu.handle_click(mouse_pos) {
                        match action {
                            ContextMenuAction::Copy => {
                                if let Some(text) = self.view.get_selected_text() {
                                    self.clipboard = text.clone();
                                    let _ = self.system_clipboard.set_text(text);
                                    self.update_message("Text copied to clipboard.");
                                } else {
                                    let text = self.view.get_current_character();
                                    self.clipboard = text.clone();
                                    let _ = self.system_clipboard.set_text(text);
                                    self.update_message("Character copied to clipboard.");
                                }
                            }
                            ContextMenuAction::Delete => {
                                if let Some(text) = self.view.get_selected_text() {
                                    self.clipboard = text;
                                    self.view.delete_selection();
                                    self.mode = EditorMode::Normal;
                                    self.update_message("Selection deleted.");
                                }
                            }
                            ContextMenuAction::Paste => {
                                self.view.move_to_position(menu.position());
                                if let Ok(text) = self.system_clipboard.get_text() {
                                    self.view.paste(&text);
                                } else {
                                    self.view.paste(&self.clipboard);
                                }
                            }
                            ContextMenuAction::Undo => {
                                self.process_command(Command::Edit(Edit::Undo));
                            }
                            ContextMenuAction::Redo => {
                                self.process_command(Command::Edit(Edit::Redo));
                            }
                            ContextMenuAction::SelectAll => {
                                // I know this is different from %, it's on purpose
                                self.mode = EditorMode::Visual;
                                self.view.select_all();
                            }
                        }
                    }
                    self.context_menu = None;
                    self.view.set_needs_redraw(true);
                    return;
                }
                MouseEventKind::Moved => {
                    menu.handle_mouse_move(mouse_pos);
                    return;
                }
                _ => {}
            }
        }

        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if mouse_pos.row < self.terminal_size.height.saturating_sub(2) {
                    self.view.move_to_position(mouse_pos);
                    self.view.clear_selection();
                    self.mode = EditorMode::Normal;
                    let _ = Terminal::set_cursor_style(SetCursorStyle::SteadyBlock);
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if self.context_menu.is_none() {
                    self.context_menu = Some(ContextMenu::new(
                        mouse_pos,
                        self.terminal_size,
                        self.view.can_undo(),
                        self.view.can_redo(),
                        self.view.has_selection(),
                        self.view
                            .can_paste(&self.clipboard, &mut self.system_clipboard),
                    ));
                } else {
                    self.context_menu = None;
                    self.view.set_needs_redraw(true);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if mouse_pos.row < self.terminal_size.height.saturating_sub(2) {
                    if self.mode != EditorMode::Visual {
                        self.mode = EditorMode::Visual;
                        self.view.start_selection();
                    }
                    self.view.move_to_position(mouse_pos);
                }
            }
            MouseEventKind::ScrollUp => {
                self.view.handle_move_command(Move::Up(1));
            }
            MouseEventKind::ScrollDown => {
                self.view.handle_move_command(Move::Down(1));
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
        self.reset_quit_times(); // Reset quit times for all other commands

        match command {
            Command::System(System::Resize(_)) => {}
            Command::System(System::Dismiss) => {
                self.view.clear_selection();
                self.update_message("");
            }
            Command::Edit(edit_command) => {
                self.view.clear_selection();
                self.view.handle_edit_command(edit_command);
            }
            Command::Move(move_command) => {
                if self.mode == EditorMode::Normal {
                    self.view.clear_selection();
                }
                self.view.handle_move_command(move_command);
            }
        }
    }

    fn process_command_during_command(&mut self, command: Command) {
        match command {
            Command::System(System::Dismiss) => {
                self.set_prompt(PromptType::None);
            }
            Command::Edit(Edit::InsertNewline) => {
                let command_str = self.command_bar.value();
                self.command_bar.add_to_history(command_str.clone());
                self.handle_vim_command(&command_str);
                if !self.should_quit {
                    self.set_prompt(PromptType::None);
                }
            }
            Command::Edit(edit_command) => self.command_bar.handle_edit_command(edit_command),
            Command::Move(Move::Up(_)) => self.command_bar.navigate_history_up(),
            Command::Move(Move::Down(_)) => self.command_bar.navigate_history_down(),
            Command::Move(Move::Left(_)) => self.command_bar.move_caret_left(),
            Command::Move(Move::Right(_)) => self.command_bar.move_caret_right(),
            _ => {}
        }
    }

    fn handle_vim_command(&mut self, command: &str) {
        let parts: Vec<&str> = command.split_whitespace().collect();
        let cmd = parts.get(0).copied().unwrap_or("");
        let arg = parts.get(1).copied();

        match cmd {
            "q" => self.handle_quit_command(),
            "q!" => self.should_quit = true,
            "w" => {
                if let Some(path) = arg {
                    self.save(Some(path));
                } else {
                    self.handle_save_command();
                }
            }
            "syntax" => self.view.toggle_syntax(),
            "wq" | "x" => {
                if let Some(path) = arg {
                    self.save(Some(path));
                } else {
                    self.save(None);
                }
                if !self.view.get_status(self.mode.to_string()).is_modified {
                    self.should_quit = true;
                }
            }
            _ => self.update_message(&format!("ERR: Unknown command: {cmd}")),
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
        if !self.view.get_status(self.mode.to_string()).is_modified
            || self.quit_times + 1 == QUIT_TIMES
        {
            self.should_quit = true;
        } else if self.view.get_status(self.mode.to_string()).is_modified {
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
            Command::System(System::Resize(_)) | Command::Move(_) => {} // Not applicable during save, Resize already handled at this stage
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
            Command::Move(Move::Right(_) | Move::Down(_)) => {
                self.view.search_next();
            }
            Command::Move(Move::Up(_) | Move::Left(_)) => {
                self.view.search_prev();
            }
            Command::System(System::Resize(_)) | Command::Move(_) => {} // Not applicable during search, Resize already handled at this stage
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
    }
}
