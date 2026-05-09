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
mod lsp;
mod terminal;
mod uicomponents;

pub use annotationtype::AnnotationType;
use lsp::{LspManager, LspMessage};
use serde_json::json;
mod annotation;
use annotation::Annotation;
mod filetype;
use annotatedstring::AnnotatedString;
use documentstatus::DocumentStatus;
use filetype::FileType;
use line::Line;
use terminal::Terminal;
use uicomponents::{
    CommandBar, ContextMenu, ContextMenuAction, InfoPopup, MessageBar, StatusBar, UIComponent, View,
};

use self::command::{Command, Edit, Move, System};

const QUIT_TIMES: u8 = 3;
const HELP_TEXT: &str = "\
HELP - ALL COMMANDS

[Movement]
  h, j, k, l : Left, Down, Up, Right
  C-u, C-d   : Half page Up/Down
  C-o, C-i   : Jump backward/forward in history
  gg / XXXg  : Buffer Start / Go to line XXX
  ge         : Buffer End
  gh / gl    : Line Start / End
  gs         : First non-whitespace
  gt/gb/gc   : View Top/Bottom/Center

[Editing]
  i          : Insert mode
  a          : Append (Right + Insert)
  o, O       : Open line below/above + Insert
  rX         : Replace character under cursor with X
  R          : Enter Replace mode
  u, U       : Undo, Redo
  p          : Paste from internal clipboard
  P          : Paste before from internal clipboard
  SPC p/P    : Paste from system clipboard
  d          : Delete selection or current character
  SPC d      : Delete selection to system clipboard
  J          : Join current line with the next one

[Selection & Visual]
  v          : Toggle Visual mode
  x          : Select whole line down
  X          : Select whole line up
  y          : Copy (yank) selection
  SPC y      : Copy (yank) selection to system clipboard
  %          : Select all

[Search & Commands]
  /          : Search
  :          : Command mode
  :w [path]  : Save
  :q, :q!    : Quit, Force quit
  :wq, :x    : Save and quit
  :syntax    : Toggle syntax highlighting
  :next, :n  : Next buffer
  :prev, :p  : Previous buffer
  :o [path]  : Open file
  :help      : Show this help

[LSP]
  gd         : Go to Definition
  SPC k      : Hover (show documentation)
  :format    : Format current buffer
";

#[derive(Copy, Clone, Debug, PartialEq)]
struct JumpEntry {
    view_idx: usize,
    location: Location,
}

#[derive(Eq, PartialEq, Default, Clone, Copy, Debug)]
pub enum EditorMode {
    #[default]
    Normal,
    Insert,
    Visual,
    Replace,
}

impl std::fmt::Display for EditorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "NORMAL"),
            Self::Insert => write!(f, "INSERT"),
            Self::Visual => write!(f, "VISUAL"),
            Self::Replace => write!(f, "REPLACE"),
        }
    }
}

#[derive(Eq, PartialEq, Default)]
enum PromptType {
    Search,
    Command,
    #[default]
    None,
}

impl PromptType {
    fn is_none(&self) -> bool {
        *self == Self::None
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum LspRequestType {
    Hover,
    Definition,
    Formatting,
}

pub struct Editor {
    should_quit: bool,
    views: Vec<View>,
    current_view_idx: usize,
    jump_list: Vec<JumpEntry>,
    jump_index: usize,
    lsp_manager: LspManager,
    pending_requests: std::collections::HashMap<lsp::RequestId, (usize, LspRequestType)>,
    info_popup: Option<InfoPopup>,
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
            views: vec![View::default()],
            current_view_idx: 0,
            jump_list: vec![JumpEntry {
                view_idx: 0,
                location: Location::default(),
            }],
            jump_index: 0,
            lsp_manager: LspManager::new(),
            pending_requests: std::collections::HashMap::new(),
            info_popup: None,
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
        let mut first = true;
        for file_name in args.iter().skip(1) {
            debug_assert!(!file_name.is_empty());
            if first {
                if editor.views[0].load(file_name).is_err() {
                    editor.update_message(&format!("ERR: Could not open file: {file_name}"));
                } else {
                    editor.notify_lsp_did_open(0);
                    first = false;
                }
            } else {
                let mut new_view = View::default();
                new_view.resize(Size {
                    height: size.height.saturating_sub(2),
                    width: size.width,
                });
                if new_view.load(file_name).is_err() {
                    editor.update_message(&format!("ERR: Could not open file: {file_name}"));
                } else {
                    editor.views.push(new_view);
                    editor.notify_lsp_did_open(editor.views.len() - 1);
                }
            }
        }
        editor.refresh_status();
        Ok(editor)
    }

    // endregion

    // region: Event Loop
    pub fn run(&mut self) {
        self.refresh_status();
        self.refresh_screen();

        loop {
            if self.should_quit {
                break;
            }

            let mut event_processed = false;
            // Process all pending events
            while poll(Duration::from_millis(0)).unwrap_or(false) {
                if let Ok(event) = read() {
                    self.evaluate_event(event);
                    event_processed = true;
                }
                if self.should_quit {
                    break;
                }
            }

            // Wait for the next event if none were pending
            if !event_processed && poll(Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(event) = read() {
                    self.evaluate_event(event);
                    event_processed = true;
                }
            }

            if event_processed {
                self.refresh_status();
                self.refresh_screen();
            }

            if self.handle_lsp_messages() {
                self.refresh_status();
                self.refresh_screen();
            }
        }
    }

    fn handle_lsp_messages(&mut self) -> bool {
        let messages = self.lsp_manager.poll_messages();
        let handled = !messages.is_empty();
        for (file_type, msg) in messages {
            match msg {
                LspMessage::Notification(notification) => {
                    if notification.method == "textDocument/publishDiagnostics" {
                        self.handle_diagnostics(file_type, notification.params);
                    }
                }
                LspMessage::Response(response) => {
                    self.handle_lsp_response(file_type, response);
                }
            }
        }
        handled
    }

    fn handle_diagnostics(&mut self, _file_type: FileType, params: serde_json::Value) {
        if let Ok(params) = serde_json::from_value::<lsp_types::PublishDiagnosticsParams>(params) {
            let uri = params.uri.to_string();
            for view in &mut self.views {
                if view.get_uri() == uri {
                    view.update_diagnostics(params.diagnostics.clone());
                }
            }
        }
    }

    fn handle_lsp_response(&mut self, _file_type: FileType, response: lsp::JsonRpcResponse) {
        if let Some(id) = response.id {
            if let Some((view_idx, req_type)) = self.pending_requests.remove(&id) {
                if let Some(result) = response.result {
                    match req_type {
                        LspRequestType::Hover => {
                            if let Ok(hover) = serde_json::from_value::<lsp_types::Hover>(result) {
                                let text = match hover.contents {
                                    lsp_types::HoverContents::Scalar(marked_string) => {
                                        match marked_string {
                                            lsp_types::MarkedString::String(s) => s,
                                            lsp_types::MarkedString::LanguageString(ls) => ls.value,
                                        }
                                    }
                                    lsp_types::HoverContents::Array(vec) => vec
                                        .iter()
                                        .map(|ms| match ms {
                                            lsp_types::MarkedString::String(s) => s.clone(),
                                            lsp_types::MarkedString::LanguageString(ls) => {
                                                ls.value.clone()
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                    lsp_types::HoverContents::Markup(markup) => markup.value,
                                };
                                if !text.is_empty() {
                                    let pos = self.views[view_idx].caret_position();
                                    self.info_popup =
                                        Some(InfoPopup::new(pos, self.terminal_size, &text));
                                }
                            }
                        }
                        LspRequestType::Definition => {
                            if let Ok(goto) =
                                serde_json::from_value::<lsp_types::GotoDefinitionResponse>(result)
                            {
                                let target = match goto {
                                    lsp_types::GotoDefinitionResponse::Scalar(loc) => Some(loc),
                                    lsp_types::GotoDefinitionResponse::Array(vec) => {
                                        vec.get(0).cloned()
                                    }
                                    lsp_types::GotoDefinitionResponse::Link(vec) => {
                                        vec.get(0).map(|link| lsp_types::Location {
                                            uri: link.target_uri.clone(),
                                            range: link.target_range,
                                        })
                                    }
                                };

                                if let Some(loc) = target {
                                    self.open_file_from_uri(loc.uri.as_str(), loc.range.start);
                                }
                            }
                        }
                        LspRequestType::Formatting => {
                            if let Ok(Some(edits)) =
                                serde_json::from_value::<Option<Vec<lsp_types::TextEdit>>>(result)
                            {
                                self.views[view_idx].apply_lsp_edits(edits);
                                self.notify_lsp_did_change(view_idx);
                            }
                        }
                    }
                }
            }
        }
    }

    fn open_file_from_uri(&mut self, uri: &str, pos: lsp_types::Position) {
        if let Some(path) = uri.strip_prefix("file://") {
            let path_str = percent_encoding::percent_decode_str(path)
                .decode_utf8_lossy()
                .into_owned();
            // Check if already open
            let mut found_idx = None;
            for (i, view) in self.views.iter().enumerate() {
                if view.get_uri() == uri {
                    found_idx = Some(i);
                    break;
                }
            }

            if let Some(idx) = found_idx {
                self.current_view_idx = idx;
            } else {
                let mut new_view = View::default();
                new_view.resize(Size {
                    height: self.terminal_size.height.saturating_sub(2),
                    width: self.terminal_size.width,
                });
                if new_view.load(&path_str).is_ok() {
                    self.views.push(new_view);
                    self.current_view_idx = self.views.len() - 1;
                    self.notify_lsp_did_open(self.current_view_idx);
                } else {
                    return;
                }
            }

            self.views[self.current_view_idx].set_lsp_location(pos);
        }
    }

    fn notify_lsp_did_open(&mut self, view_idx: usize) {
        let view = &self.views[view_idx];
        let file_type = view.get_status(String::new()).file_type;
        let uri = view.get_uri();
        if uri.is_empty() {
            return;
        }

        if let Some(client) = self.lsp_manager.get_client(file_type) {
            let params = json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": match file_type {
                        FileType::Rust => "rust",
                        FileType::JavaScript => "javascript",
                        FileType::Zig => "zig",
                        FileType::Text => "text",
                    },
                    "version": 1,
                    "text": view.get_text()
                }
            });
            client.send_notification("textDocument/didOpen", params);
        }
    }

    fn notify_lsp_did_change(&mut self, view_idx: usize) {
        let view = &self.views[view_idx];
        let file_type = view.get_status(String::new()).file_type;
        let uri = view.get_uri();
        if uri.is_empty() {
            return;
        }

        if let Some(client) = self.lsp_manager.get_client(file_type) {
            let params = json!({
                "textDocument": {
                    "uri": uri,
                    "version": 2 // Simplification: we should track version per view
                },
                "contentChanges": [{
                    "text": view.get_text()
                }]
            });
            client.send_notification("textDocument/didChange", params);
        }
    }

    fn lsp_hover(&mut self) {
        let view = &self.views[self.current_view_idx];
        let file_type = view.get_status(String::new()).file_type;
        let uri = view.get_uri();
        if uri.is_empty() {
            return;
        }

        if let Some(client) = self.lsp_manager.get_client(file_type) {
            let params = json!({
                "textDocument": { "uri": uri },
                "position": view.get_lsp_position()
            });
            let id = client.send_request("textDocument/hover", params);
            self.pending_requests
                .insert(id, (self.current_view_idx, LspRequestType::Hover));
        }
    }

    fn lsp_goto_definition(&mut self) {
        self.record_jump();
        let view = &self.views[self.current_view_idx];
        let file_type = view.get_status(String::new()).file_type;
        let uri = view.get_uri();
        if uri.is_empty() {
            return;
        }

        if let Some(client) = self.lsp_manager.get_client(file_type) {
            let params = json!({
                "textDocument": { "uri": uri },
                "position": view.get_lsp_position()
            });
            let id = client.send_request("textDocument/definition", params);
            self.pending_requests
                .insert(id, (self.current_view_idx, LspRequestType::Definition));
        }
    }

    fn lsp_format(&mut self) {
        self.record_jump();
        let view = &self.views[self.current_view_idx];
        let file_type = view.get_status(String::new()).file_type;
        let uri = view.get_uri();
        if uri.is_empty() {
            return;
        }

        if let Some(client) = self.lsp_manager.get_client(file_type) {
            let params = json!({
                "textDocument": { "uri": uri },
                "options": {
                    "tabSize": 4,
                    "insertSpaces": true
                }
            });
            let id = client.send_request("textDocument/formatting", params);
            self.pending_requests
                .insert(id, (self.current_view_idx, LspRequestType::Formatting));
        }
    }

    fn refresh_screen(&mut self) {
        if self.terminal_size.height == 0 || self.terminal_size.width == 0 || self.views.is_empty()
        {
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
                self.views[self.current_view_idx].set_needs_redraw(true);
            }
            self.views[self.current_view_idx].render(0);
        }
        if let Some(menu) = &mut self.context_menu {
            menu.set_needs_redraw(true);
            menu.render(0);
        }
        if let Some(popup) = &mut self.info_popup {
            popup.set_needs_redraw(true);
            popup.render(0);
        }
        let new_caret_pos = if self.in_prompt() {
            Position {
                row: bottom_bar_row,
                col: self.command_bar.caret_position_col(),
            }
        } else {
            self.views[self.current_view_idx].caret_position()
        };
        debug_assert!(new_caret_pos.col <= self.terminal_size.width);
        debug_assert!(new_caret_pos.row <= self.terminal_size.height);

        let _ = Terminal::set_cursor_style(self.get_cursor_style());
        let _ = Terminal::move_caret_to(new_caret_pos);
        let _ = Terminal::show_caret();
        let _ = Terminal::execute();
    }

    fn get_cursor_style(&self) -> SetCursorStyle {
        match self.mode {
            EditorMode::Insert => SetCursorStyle::SteadyBar,
            EditorMode::Replace => SetCursorStyle::SteadyUnderScore,
            EditorMode::Normal | EditorMode::Visual => SetCursorStyle::SteadyBlock,
        }
    }

    fn refresh_status(&mut self) {
        if self.views.is_empty() {
            return;
        }
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

        let status = self.views[self.current_view_idx].get_status(self.mode.to_string());
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
            self.info_popup = None;
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
                            if self.move_left_on_escape {
                                self.process_command(Command::Move(Move::Left(1)));
                                self.move_left_on_escape = false;
                            }
                            return;
                        }
                    }
                    EditorMode::Replace => {
                        if key_event.code == KeyCode::Esc {
                            self.mode = EditorMode::Normal;
                            return;
                        }
                        if let KeyCode::Char(c) = key_event.code {
                            self.views[self.current_view_idx].handle_replace_mode_char(c);
                            return;
                        }
                    }
                }
            }
            if let Ok(command) = Command::try_from(key_event) {
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
                (KeyCode::Char('R'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                    self.mode = EditorMode::Replace;
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
                    if let Some(text) = self.views[self.current_view_idx].get_selected_text() {
                        self.clipboard = text;
                        self.update_message("Text copied to clipboard.");
                    } else {
                        self.clipboard = self.views[self.current_view_idx].get_current_character();
                        self.update_message("Character copied to clipboard.");
                    }
                }
                (KeyCode::Char('v'), KeyModifiers::NONE) => {
                    if self.mode == EditorMode::Visual {
                        self.mode = EditorMode::Normal;
                        self.views[self.current_view_idx].clear_selection();
                    } else {
                        self.mode = EditorMode::Visual;
                        if self.views[self.current_view_idx].get_selection().is_none() {
                            self.views[self.current_view_idx].start_selection();
                        }
                    }
                }
                (KeyCode::Char('x'), KeyModifiers::NONE) => {
                    self.views[self.current_view_idx].select_line_down();
                }
                (KeyCode::Char('X'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                    self.views[self.current_view_idx].select_line_up();
                }
                (KeyCode::Char('d'), KeyModifiers::NONE) => {
                    if let Some(text) = self.views[self.current_view_idx].get_selected_text() {
                        self.clipboard = text;
                        self.views[self.current_view_idx].delete_selection();
                        self.mode = EditorMode::Normal;
                    } else {
                        self.process_command(Command::Edit(Edit::Delete));
                    }
                }
                (KeyCode::Char('p'), KeyModifiers::NONE) => {
                    self.views[self.current_view_idx].paste(&self.clipboard);
                    self.mode = EditorMode::Normal;
                }
                (KeyCode::Char('P'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                    self.views[self.current_view_idx].paste_backward(&self.clipboard);
                    self.mode = EditorMode::Normal;
                }
                (KeyCode::Char('%'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                    self.views[self.current_view_idx].select_all();
                }
                (KeyCode::Char('J'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                    self.views[self.current_view_idx].concat_lines();
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
                (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
                    self.process_command(Command::Move(Move::JumpBackward));
                }
                (KeyCode::Char('i'), KeyModifiers::CONTROL) => {
                    self.process_command(Command::Move(Move::JumpForward));
                }
                (KeyCode::Esc, KeyModifiers::NONE) => {
                    self.views[self.current_view_idx].clear_selection();
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
                    (KeyCode::Char('d'), KeyModifiers::NONE) => {
                        self.lsp_goto_definition();
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
                        let text = if self.views[self.current_view_idx].get_selection().is_some() {
                            self.views[self.current_view_idx].get_selected_text()
                        } else {
                            Some(self.views[self.current_view_idx].get_current_character())
                        };

                        if let Some(text) = text {
                            if let Err(e) = self.system_clipboard.set_text(text) {
                                self.update_message(&format!("ERR: Clipboard error: {e}"));
                            } else {
                                if self.views[self.current_view_idx].get_selection().is_some() {
                                    self.update_message("Text copied to system clipboard.");
                                } else {
                                    self.update_message("Character copied to system clipboard.");
                                }
                            }
                        }
                    }
                    (KeyCode::Char('p'), KeyModifiers::NONE) => {
                        if let Ok(text) = self.system_clipboard.get_text() {
                            self.views[self.current_view_idx].paste(&text);
                        }
                        self.mode = EditorMode::Normal;
                    }
                    (KeyCode::Char('P'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                        if let Ok(text) = self.system_clipboard.get_text() {
                            self.views[self.current_view_idx].paste_backward(&text);
                        }
                        self.mode = EditorMode::Normal;
                    }
                    (KeyCode::Char('d'), KeyModifiers::NONE) => {
                        if let Some(text) = self.views[self.current_view_idx].get_selected_text() {
                            if let Err(e) = self.system_clipboard.set_text(text) {
                                self.update_message(&format!("ERR: Clipboard error: {e}"));
                            } else {
                                self.views[self.current_view_idx].delete_selection();
                                self.mode = EditorMode::Normal;
                            }
                        } else {
                            self.process_command(Command::Edit(Edit::Delete));
                        }
                    }
                    (KeyCode::Char('k'), KeyModifiers::NONE) => {
                        self.lsp_hover();
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
                                if let Some(text) =
                                    self.views[self.current_view_idx].get_selected_text()
                                {
                                    self.clipboard = text.clone();
                                    let _ = self.system_clipboard.set_text(text);
                                    self.update_message("Text copied to clipboard.");
                                } else {
                                    let text =
                                        self.views[self.current_view_idx].get_current_character();
                                    self.clipboard = text.clone();
                                    let _ = self.system_clipboard.set_text(text);
                                    self.update_message("Character copied to clipboard.");
                                }
                            }
                            ContextMenuAction::Delete => {
                                if let Some(text) =
                                    self.views[self.current_view_idx].get_selected_text()
                                {
                                    self.clipboard = text;
                                    self.views[self.current_view_idx].delete_selection();
                                    self.mode = EditorMode::Normal;
                                    self.update_message("Selection deleted.");
                                }
                            }
                            ContextMenuAction::Paste => {
                                self.views[self.current_view_idx].move_to_position(menu.position());
                                if let Ok(text) = self.system_clipboard.get_text() {
                                    self.views[self.current_view_idx].paste(&text);
                                } else {
                                    self.views[self.current_view_idx].paste(&self.clipboard);
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
                                self.views[self.current_view_idx].select_all();
                            }
                        }
                    }
                    self.context_menu = None;
                    self.views[self.current_view_idx].set_needs_redraw(true);
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
                    self.views[self.current_view_idx].move_to_position(mouse_pos);
                    self.views[self.current_view_idx].clear_selection();
                    self.mode = EditorMode::Normal;
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if self.context_menu.is_none() {
                    self.context_menu = Some(ContextMenu::new(
                        mouse_pos,
                        self.terminal_size,
                        self.views[self.current_view_idx].can_undo(),
                        self.views[self.current_view_idx].can_redo(),
                        self.views[self.current_view_idx].has_selection(),
                        self.views[self.current_view_idx]
                            .can_paste(&self.clipboard, &mut self.system_clipboard),
                    ));
                } else {
                    self.context_menu = None;
                    self.views[self.current_view_idx].set_needs_redraw(true);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if mouse_pos.row < self.terminal_size.height.saturating_sub(2) {
                    if self.mode != EditorMode::Visual {
                        self.mode = EditorMode::Visual;
                        self.views[self.current_view_idx].start_selection();
                    }
                    self.views[self.current_view_idx].move_to_position(mouse_pos);
                }
            }
            MouseEventKind::ScrollUp => {
                self.views[self.current_view_idx].handle_move_command(Move::Up(1));
            }
            MouseEventKind::ScrollDown => {
                self.views[self.current_view_idx].handle_move_command(Move::Down(1));
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
            PromptType::Command => self.process_command_during_command(command),
            PromptType::None => self.process_command_no_prompt(command),
        }
    }

    fn process_command_no_prompt(&mut self, command: Command) {
        self.reset_quit_times(); // Reset quit times for all other commands

        match command {
            Command::System(System::Resize(_)) => {}
            Command::System(System::Dismiss) => {
                self.views[self.current_view_idx].clear_selection();
                self.update_message("");
            }
            Command::Edit(edit_command) => {
                if self.mode == EditorMode::Normal
                    && matches!(
                        edit_command,
                        Edit::Undo | Edit::Redo | Edit::InsertNewline | Edit::Insert(_)
                    )
                {
                    self.record_jump();
                }
                self.views[self.current_view_idx].clear_selection();
                self.views[self.current_view_idx].handle_edit_command(edit_command);
                self.notify_lsp_did_change(self.current_view_idx);
            }
            Command::Move(move_command) => {
                if self.mode == EditorMode::Normal {
                    self.views[self.current_view_idx].clear_selection();
                }
                if matches!(
                    move_command,
                    Move::ViewTop
                        | Move::ViewBottom
                        | Move::ViewCenter
                        | Move::BufferStart
                        | Move::BufferEnd
                        | Move::GoToLine(_)
                ) {
                    self.record_jump();
                }
                match move_command {
                    Move::JumpBackward => self.move_to_jump_backward(),
                    Move::JumpForward => self.move_to_jump_forward(),
                    _ => self.views[self.current_view_idx].handle_move_command(move_command),
                }
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
            Command::Edit(Edit::Complete) => self.handle_complete_command(),
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
            "q" | "quit" => self.handle_quit_command(),
            "q!" | "quit!" => self.should_quit = true,
            "w" | "write" => {
                if let Some(path) = arg {
                    self.save(Some(path));
                } else {
                    self.handle_save_command();
                }
            }
            "syntax" => self.views[self.current_view_idx].toggle_syntax(),
            "fmt" | "format" => self.lsp_format(),
            "wq" | "x" => {
                if let Some(path) = arg {
                    self.save(Some(path));
                } else {
                    self.save(None);
                }
                if !self.views[self.current_view_idx]
                    .get_status(self.mode.to_string())
                    .is_modified
                {
                    self.should_quit = true;
                }
            }
            "p" | "prev" => {
                self.record_jump();
                self.current_view_idx =
                    (self.current_view_idx + self.views.len() - 1) % self.views.len();
                self.views[self.current_view_idx].set_needs_redraw(true);
                self.reset_quit_times();
            }
            "n" | "next" => {
                self.record_jump();
                self.current_view_idx = (self.current_view_idx + 1) % self.views.len();
                self.views[self.current_view_idx].set_needs_redraw(true);
                self.reset_quit_times();
            }
            "o" | "open" | "e" | "edit" => {
                if let Some(path) = arg {
                    self.record_jump();
                    let mut new_view = View::default();
                    new_view.resize(Size {
                        height: self.terminal_size.height.saturating_sub(2),
                        width: self.terminal_size.width,
                    });
                    if new_view.load(path).is_err() {
                        self.update_message(&format!("ERR: Could not open file: {path}"));
                    } else {
                        self.views.push(new_view);
                        self.current_view_idx = self.views.len() - 1;
                        self.notify_lsp_did_open(self.current_view_idx);
                        self.update_message(&format!("Opened file: {path}"));
                        self.reset_quit_times();
                    }
                } else {
                    self.update_message("ERR: No file name provided");
                }
            }
            "h" | "help" => {
                self.record_jump();
                let new_view = View::new_with_content(
                    HELP_TEXT,
                    "HELP",
                    Size {
                        height: self.terminal_size.height.saturating_sub(2),
                        width: self.terminal_size.width,
                    },
                );
                self.views.push(new_view);
                self.current_view_idx = self.views.len() - 1;
                self.update_message("Opened help");
                self.reset_quit_times();
            }
            _ => self.update_message(&format!("ERR: Unknown command: {cmd}")),
        }
    }

    // region jump list handling
    fn record_jump(&mut self) {
        let entry = JumpEntry {
            view_idx: self.current_view_idx,
            location: self.views[self.current_view_idx].text_location(),
        };
        if self.jump_list.get(self.jump_index) != Some(&entry) {
            self.jump_list.truncate(self.jump_index.saturating_add(1));
            self.jump_list.push(entry);
            self.jump_index = self.jump_list.len().saturating_sub(1);
        }
    }

    fn move_to_jump_backward(&mut self) {
        let entry = JumpEntry {
            view_idx: self.current_view_idx,
            location: self.views[self.current_view_idx].text_location(),
        };
        if self.jump_list.get(self.jump_index) != Some(&entry) {
            self.record_jump();
        }
        if self.jump_index > 0 {
            self.jump_index = self.jump_index.saturating_sub(1);
            let entry = self.jump_list[self.jump_index];
            if entry.view_idx < self.views.len() {
                self.current_view_idx = entry.view_idx;
                self.views[self.current_view_idx].set_text_location(entry.location);
            }
        }
    }

    fn move_to_jump_forward(&mut self) {
        if self.jump_index.saturating_add(1) < self.jump_list.len() {
            self.jump_index = self.jump_index.saturating_add(1);
            let entry = self.jump_list[self.jump_index];
            if entry.view_idx < self.views.len() {
                self.current_view_idx = entry.view_idx;
                self.views[self.current_view_idx].set_text_location(entry.location);
            }
        }
    }
    // endregion

    fn handle_resize_command(&mut self, size: Size) {
        self.terminal_size = size;
        for view in &mut self.views {
            view.resize(Size {
                height: size.height.saturating_sub(2),
                width: size.width,
            });
        }
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
        if self.quit_times + 1 == QUIT_TIMES {
            self.should_quit = true;
            return;
        }

        self.views
            .retain(|v| v.get_status(self.mode.to_string()).is_modified);

        self.jump_list = vec![JumpEntry {
            view_idx: 0,
            location: Location::default(),
        }];
        self.jump_index = 0;

        if self.views.is_empty() {
            self.should_quit = true;
        } else {
            if self.current_view_idx >= self.views.len() {
                self.current_view_idx = self.views.len() - 1;
            }
            let dirty_count = self.views.len();
            self.update_message(&format!(
                "WARNING! {dirty_count} buffer(s) have unsaved changes. Type :q {} more times to quit.",
                QUIT_TIMES - self.quit_times - 1
            ));
            self.quit_times += 1;
            self.views[self.current_view_idx].set_needs_redraw(true);
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
        if self.views[self.current_view_idx].is_file_loaded() {
            self.save(None);
        } else {
            self.update_message(&format!("ERR: Can't save without filename!"));
        }
    }
    fn save(&mut self, file_name: Option<&str>) {
        let result = if let Some(name) = file_name {
            self.views[self.current_view_idx].save_as(name)
        } else {
            self.views[self.current_view_idx].save()
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
                self.views[self.current_view_idx].dismiss_search();
            }
            Command::Edit(Edit::InsertNewline) => {
                self.set_prompt(PromptType::None);
                self.views[self.current_view_idx].exit_search();
            }
            Command::Edit(edit_command) => {
                self.command_bar.handle_edit_command(edit_command);
                let query = self.command_bar.value();
                self.views[self.current_view_idx].search(&query);
            }
            Command::Move(Move::Right(_) | Move::Down(_)) => {
                self.record_jump();
                self.views[self.current_view_idx].search_next();
            }
            Command::Move(Move::Up(_) | Move::Left(_)) => {
                self.record_jump();
                self.views[self.current_view_idx].search_prev();
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
        if !prompt_type.is_none() {
            self.update_message("");
        }
        match prompt_type {
            PromptType::None => self.message_bar.set_needs_redraw(true), //Ensures the message bar is properly painted during the next redraw cycle
            PromptType::Command => self.command_bar.set_prompt(":"),
            PromptType::Search => {
                self.views[self.current_view_idx].enter_search();
                self.command_bar
                    .set_prompt("Search (Esc to cancel, Arrows to navigate): ");
            }
        }
        self.command_bar.clear_value();
        self.prompt_type = prompt_type;
    }
    fn handle_complete_command(&mut self) {
        let (mut matches, mut index, mut original) = self.command_bar.get_completion_state();
        let current_value = self.command_bar.value();

        if original.is_none() {
            // First tab press: find matches
            let parts: Vec<&str> = current_value.split_whitespace().collect();
            if parts.is_empty() {
                return;
            }

            if parts.len() == 1 && !current_value.ends_with(' ') {
                let cmd_to_complete = parts[0];
                let commands = [
                    "q", "quit", "q!", "quit!", "w", "write", "syntax", "format", "wq", "x", "p",
                    "prev", "n", "next", "o", "open", "h", "help",
                ];

                matches = commands
                    .iter()
                    .filter(|cmd| cmd.starts_with(cmd_to_complete))
                    .map(|&s| s.to_string())
                    .collect();
                original = Some(cmd_to_complete.to_string());
            } else if parts.len() <= 2 {
                let cmd = parts[0];
                if matches!(cmd, "w" | "write" | "o" | "open" | "wq" | "x") {
                    let path_to_complete = if parts.len() == 2 { parts[1] } else { "" };
                    let (dir, file_prefix) = if let Some(last_slash_idx) = path_to_complete.rfind('/') {
                        let (d, f) = path_to_complete.split_at(last_slash_idx + 1);
                        (d, f)
                    } else {
                        (".", path_to_complete)
                    };

                    if let Ok(entries) = std::fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            if let Ok(name) = entry.file_name().into_string() {
                                if name.starts_with(file_prefix) {
                                    let mut full_path = if dir == "." {
                                        name
                                    } else {
                                        format!("{dir}{name}")
                                    };
                                    if entry.path().is_dir() {
                                        full_path.push('/');
                                    }
                                    matches.push(full_path);
                                }
                            }
                        }
                    }
                    original = Some(current_value.clone());
                }
            }
        }

        if matches.is_empty() {
            return;
        }

        match index {
            None => {
                // First Tab: Complete to Longest Common Prefix or unique match
                if matches.len() == 1 {
                    let new_val = if current_value.split_whitespace().count() <= 1
                        && !current_value.ends_with(' ')
                    {
                        matches[0].clone()
                    } else {
                        let cmd = current_value.split_whitespace().next().unwrap_or("");
                        format!("{cmd} {}", matches[0])
                    };
                    self.command_bar.set_value(&new_val);
                    index = Some(0);
                } else {
                    let lcp = longest_common_prefix(
                        &matches.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                    );
                    let prefix_len = if current_value.split_whitespace().count() <= 1
                        && !current_value.ends_with(' ')
                    {
                        original.as_ref().map_or(0, String::len)
                    } else {
                        let full_prefix = current_value.split_whitespace().nth(1).unwrap_or("");
                        if let Some(last_slash_idx) = full_prefix.rfind('/') {
                            full_prefix.len().saturating_sub(last_slash_idx + 1)
                        } else {
                            full_prefix.len()
                        }
                    };

                    if lcp.len() > prefix_len {
                        let new_val = if current_value.split_whitespace().count() <= 1
                            && !current_value.ends_with(' ')
                        {
                            lcp
                        } else {
                            let cmd = current_value.split_whitespace().next().unwrap_or("");
                            format!("{cmd} {lcp}")
                        };
                        self.command_bar.set_value(&new_val);
                    }
                    index = Some(usize::MAX); // Special value to indicate LCP was done
                }
            }
            Some(i) => {
                // Subsequent Tabs: Cycle through matches
                let new_index = if i == usize::MAX {
                    0
                } else {
                    (i + 1) % matches.len()
                };
                let new_val = if original.as_ref().map_or(false, |o| !o.contains(' ')) {
                    matches[new_index].clone()
                } else {
                    let cmd = original
                        .as_ref()
                        .and_then(|o| o.split_whitespace().next())
                        .unwrap_or("");
                    format!("{cmd} {}", matches[new_index])
                };
                self.command_bar.set_value(&new_val);
                index = Some(new_index);
            }
        }

        self.command_bar
            .set_completion_state(matches, index, original);
    }
}

fn longest_common_prefix(strings: &[&str]) -> String {
    if strings.is_empty() {
        return String::new();
    }
    let first = strings[0];
    for (i, char) in first.chars().enumerate() {
        for string in &strings[1..] {
            if i >= string.len() || string.chars().nth(i) != Some(char) {
                return first[..i].to_string();
            }
        }
    }
    first.to_string()
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = Terminal::terminate();
    }
}
