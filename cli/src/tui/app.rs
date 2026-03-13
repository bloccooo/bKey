use automerge::AutoCommit;
use autosurgeon::{hydrate, reconcile};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use crossterm::event::{KeyCode, KeyEvent};
use lib::{
    crypto::{compute_key_mac, wrap_dek},
    error::Result,
    members::{remove_member, rotate_dek},
    namespaces::{add_namespace, remove_namespace, set_namespace_secrets, update_namespace},
    secrets::{add_secret, list_secrets, remove_secret, update_secret, PlaintextSecretFields},
    store::{Session, Store},
    types::{EnviDocument, Member, Namespace, PlaintextSecret},
};
use std::sync::Arc;

// --- Mode ---

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    List,
    NewSecret,
    NewNamespace,
    EditSecret,
    EditNamespace,
    NamespaceSecrets,
    Invite,
}

// --- Focus ---

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Namespaces,
    Secrets,
    Members,
}

// --- Form field definitions ---

#[derive(Clone)]
pub struct FormField {
    pub label: &'static str,
    pub secret: bool,
}

pub const SECRET_FIELDS: &[FormField] = &[
    FormField {
        label: "Name",
        secret: false,
    },
    FormField {
        label: "Value",
        secret: true,
    },
    FormField {
        label: "Description",
        secret: false,
    },
    FormField {
        label: "Tags (comma-separated)",
        secret: false,
    },
];

pub const NAMESPACE_FIELDS: &[FormField] = &[FormField {
    label: "Name",
    secret: false,
}];

// --- App state ---

pub struct App {
    pub doc: AutoCommit,
    pub store: Arc<Store>,
    pub session: Session,
    pub invite_link: String,

    pub mode: Mode,
    pub focus: Focus,

    // List indices
    pub ns_idx: usize,
    pub sec_idx: usize,
    pub member_idx: usize,

    pub show_values: bool,
    pub syncing: bool,

    // Derived data (refreshed after every doc change)
    pub namespaces: Vec<Namespace>,
    pub secrets: Vec<PlaintextSecret>,
    pub members: Vec<Member>,

    // Form state
    pub field_idx: usize,
    pub field_input: String,
    pub cursor: usize,
    pub collected_values: Vec<String>,
    pub initial_values: Vec<String>,
    pub editing_id: Option<String>,

    // Textarea state (multi-line Value field in secret forms)
    pub ta_lines: Vec<String>,
    pub ta_row: usize,
    pub ta_col: usize,
    pub ta_scroll: usize,

    // Project-secrets checklist
    pub ps_cursor: usize,
    pub ps_selected_ids: std::collections::HashSet<String>,

    // Confirmation dialogs
    pub namespace_to_delete: Option<String>,
    pub secret_to_delete: Option<String>,
    pub member_to_delete: Option<String>,
    pub member_to_grant: Option<String>,
    pub confirming_rotate: bool,

    // Clipboard (kept alive so Linux clipboard manager can serve paste requests)
    clipboard: Option<arboard::Clipboard>,
    pub clipboard_ok: bool,
    pub copied_at: Option<std::time::Instant>,

    // Background persist task handle
    persist_task: Option<tokio::task::JoinHandle<()>>,
}

impl App {
    pub fn new(
        doc: AutoCommit,
        store: Store,
        session: Session,
        invite_link: String,
    ) -> Result<Self> {
        let store = Arc::new(store);
        let mut app = Self {
            doc,
            store,
            session,
            invite_link,
            mode: Mode::List,
            focus: Focus::Namespaces,
            ns_idx: 0,
            sec_idx: 0,
            member_idx: 0,
            show_values: false,
            syncing: false,
            clipboard: arboard::Clipboard::new().ok(),
            clipboard_ok: false,
            copied_at: None,
            namespaces: vec![],
            secrets: vec![],
            members: vec![],
            field_idx: 0,
            field_input: String::new(),
            cursor: 0,
            collected_values: vec![],
            initial_values: vec![],
            editing_id: None,
            ps_cursor: 0,
            ps_selected_ids: std::collections::HashSet::new(),
            namespace_to_delete: None,
            secret_to_delete: None,
            member_to_delete: None,
            member_to_grant: None,
            confirming_rotate: false,
            persist_task: None,
            ta_lines: vec![String::new()],
            ta_row: 0,
            ta_col: 0,
            ta_scroll: 0,
        };
        app.refresh()?;
        Ok(app)
    }

    /// Refresh derived state from the automerge document.
    pub fn refresh(&mut self) -> Result<()> {
        let state: EnviDocument = hydrate(&self.doc)?;
        self.namespaces = state.namespaces.into_values().collect();
        self.secrets = list_secrets(&self.doc, &self.session.dek)?;
        self.members = state.members.into_values().collect();

        // Clamp indices
        if !self.namespaces.is_empty() {
            self.ns_idx = self.ns_idx.min(self.namespaces.len() - 1);
        }
        if !self.secrets.is_empty() {
            self.sec_idx = self.sec_idx.min(self.secrets.len() - 1);
        }
        if !self.members.is_empty() {
            self.member_idx = self.member_idx.min(self.members.len() - 1);
        }
        Ok(())
    }

    /// Kick off async persist in the background (fire and forget).
    fn schedule_persist(&mut self) {
        let doc_bytes = self.doc.save();
        let store = Arc::clone(&self.store);
        let signing_key = self.session.signing_key.clone();

        // Load a fresh doc from bytes to send to the task
        let handle = tokio::spawn(async move {
            if let Ok(mut doc) = AutoCommit::load(&doc_bytes) {
                let _ = store.persist(&mut doc, &signing_key).await;
            }
        });
        self.persist_task = Some(handle);
        self.syncing = true;
    }

    pub async fn tick(&mut self) {
        if let Some(handle) = &self.persist_task {
            if handle.is_finished() {
                self.persist_task = None;
                self.syncing = false;
            }
        }
    }

    // --- Form helpers ---

    fn open_new_form(&mut self, mode: Mode) {
        self.mode = mode;
        self.editing_id = None;
        self.initial_values = vec![];
        self.field_idx = 0;
        self.field_input = String::new();
        self.cursor = 0;
        self.collected_values = vec![];
        self.ta_lines = vec![String::new()];
        self.ta_row = 0;
        self.ta_col = 0;
        self.ta_scroll = 0;
    }

    fn open_edit_form(&mut self, mode: Mode, id: &str, values: Vec<String>) {
        self.mode = mode;
        self.editing_id = Some(id.to_string());
        self.initial_values = values.clone();
        self.field_idx = 0;
        self.field_input = values.first().cloned().unwrap_or_default();
        self.cursor = self.field_input.len();
        self.collected_values = vec![];
        // Pre-populate textarea from the value field (index 1)
        let value_str = values.get(1).cloned().unwrap_or_default();
        self.ta_lines = if value_str.is_empty() {
            vec![String::new()]
        } else {
            value_str.split('\n').map(|s| s.to_string()).collect()
        };
        self.ta_row = 0;
        self.ta_col = 0;
        self.ta_scroll = 0;
    }

    /// True when the active form field should use the multi-line textarea.
    pub fn is_textarea_field(&self) -> bool {
        matches!(self.mode, Mode::NewSecret | Mode::EditSecret) && self.field_idx == 1
    }

    fn go_back_field(&mut self) {
        if self.field_idx == 0 {
            return;
        }
        let prev_idx = self.field_idx - 1;
        let prev_value = self.collected_values.pop().unwrap_or_default();
        self.field_idx = prev_idx;

        let is_ta = matches!(self.mode, Mode::NewSecret | Mode::EditSecret) && prev_idx == 1;
        if is_ta {
            self.ta_lines = if prev_value.is_empty() {
                vec![String::new()]
            } else {
                prev_value.split('\n').map(|s| s.to_string()).collect()
            };
            self.ta_row = self.ta_lines.len().saturating_sub(1);
            self.ta_col = self.ta_lines[self.ta_row].chars().count();
            self.ta_scroll = 0;
        } else {
            self.field_input = prev_value;
            self.cursor = self.field_input.len();
        }
    }

    fn advance_field(&mut self) {
        let value = if self.is_textarea_field() {
            self.ta_lines.join("\n")
        } else {
            self.field_input.clone()
        };
        let next_idx = self.field_idx + 1;
        let next_initial = self
            .initial_values
            .get(next_idx)
            .cloned()
            .unwrap_or_default();
        self.collected_values.push(value);
        self.field_idx = next_idx;
        self.field_input = next_initial.clone();
        self.cursor = next_initial.len();
    }

    fn submit_form(&mut self) -> Result<()> {
        let mut all_values = self.collected_values.clone();
        all_values.push(self.field_input.clone());

        match &self.mode.clone() {
            Mode::NewSecret => {
                let [name, value, description, tags_str] = all_values_array(&all_values);
                add_secret(
                    &mut self.doc,
                    &self.session.dek,
                    PlaintextSecretFields {
                        name,
                        value,
                        description,
                        tags: split_tags(&tags_str),
                    },
                )?;
                self.refresh()?;
                self.schedule_persist();
            }
            Mode::NewNamespace => {
                let [name, ..] = all_values_array(&all_values);
                add_namespace(&mut self.doc, &name)?;
                self.refresh()?;
                self.schedule_persist();
            }
            Mode::EditSecret => {
                if let Some(id) = self.editing_id.clone() {
                    let [name, value, description, tags_str] = all_values_array(&all_values);
                    update_secret(
                        &mut self.doc,
                        &self.session.dek,
                        &id,
                        PlaintextSecretFields {
                            name,
                            value,
                            description,
                            tags: split_tags(&tags_str),
                        },
                    )?;
                    self.refresh()?;
                    self.schedule_persist();
                }
            }
            Mode::EditNamespace => {
                if let Some(id) = self.editing_id.clone() {
                    let [name, ..] = all_values_array(&all_values);
                    update_namespace(&mut self.doc, &id, &name)?;
                    self.refresh()?;
                    self.schedule_persist();
                }
            }
            _ => {}
        }

        self.mode = Mode::List;
        Ok(())
    }

    // --- Main key handler ---

    /// Returns true if the app should quit.
    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Namespace / secret delete confirmations
        if self.namespace_to_delete.is_some() {
            return self.handle_namespace_delete(key);
        }
        if self.secret_to_delete.is_some() {
            return self.handle_secret_delete(key);
        }

        // Member delete confirmation
        if self.member_to_delete.is_some() {
            return self.handle_member_delete(key);
        }

        // Member grant confirmation
        if self.member_to_grant.is_some() {
            return self.handle_member_grant(key);
        }

        // DEK rotation confirmation
        if self.confirming_rotate {
            return self.handle_dek_rotate(key);
        }

        match &self.mode.clone() {
            Mode::Invite => {
                if key.code == KeyCode::Esc {
                    self.mode = Mode::List;
                }
                return Ok(false);
            }
            Mode::NamespaceSecrets => {
                return self.handle_namespace_secrets(key);
            }
            Mode::List => {}
            _ => {
                return self.handle_form(key);
            }
        }

        // --- List mode ---
        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Tab => {
                self.focus = match &self.focus {
                    Focus::Namespaces => Focus::Secrets,
                    Focus::Secrets => Focus::Members,
                    Focus::Members => Focus::Namespaces,
                };
            }
            KeyCode::Char('v') => {
                self.show_values = !self.show_values;
            }
            KeyCode::Char('n') => {
                if self.focus != Focus::Members {
                    let mode = if self.focus == Focus::Namespaces {
                        Mode::NewNamespace
                    } else {
                        Mode::NewSecret
                    };
                    self.open_new_form(mode);
                }
            }
            KeyCode::Up => match &self.focus {
                Focus::Namespaces => {
                    if self.ns_idx > 0 {
                        self.ns_idx -= 1;
                    }
                }
                Focus::Secrets => {
                    if self.sec_idx > 0 {
                        self.sec_idx -= 1;
                    }
                }
                Focus::Members => {
                    if self.member_idx > 0 {
                        self.member_idx -= 1;
                    }
                }
            },
            KeyCode::Down => match &self.focus {
                Focus::Namespaces => {
                    if self.ns_idx + 1 < self.namespaces.len() {
                        self.ns_idx += 1;
                    }
                }
                Focus::Secrets => {
                    if self.sec_idx + 1 < self.secrets.len() {
                        self.sec_idx += 1;
                    }
                }
                Focus::Members => {
                    if self.member_idx + 1 < self.members.len() {
                        self.member_idx += 1;
                    }
                }
            },
            KeyCode::Char('e') => match &self.focus {
                Focus::Secrets => {
                    if let Some(sec) = self.secrets.get(self.sec_idx) {
                        let sec = sec.clone();
                        self.open_edit_form(
                            Mode::EditSecret,
                            &sec.id,
                            vec![
                                sec.name.clone(),
                                sec.value.clone(),
                                sec.description.clone(),
                                sec.tags.join(", "),
                            ],
                        );
                    }
                }
                Focus::Namespaces => {
                    if let Some(ns) = self.namespaces.get(self.ns_idx) {
                        let ns = ns.clone();
                        self.open_edit_form(Mode::EditNamespace, &ns.id, vec![ns.name.clone()]);
                    }
                }
                Focus::Members => {}
            },
            KeyCode::Char('s') => {
                if self.focus == Focus::Namespaces {
                    if let Some(ns) = self.namespaces.get(self.ns_idx) {
                        let ns_id = ns.id.clone();
                        let selected: std::collections::HashSet<String> =
                            ns.secret_ids.iter().cloned().collect();
                        self.ps_cursor = 0;
                        self.ps_selected_ids = selected;
                        self.editing_id = Some(ns_id);
                        self.mode = Mode::NamespaceSecrets;
                    }
                }
            }
            KeyCode::Char('d') => match &self.focus {
                Focus::Namespaces => {
                    if let Some(ns) = self.namespaces.get(self.ns_idx) {
                        self.namespace_to_delete = Some(ns.id.clone());
                    }
                }
                Focus::Secrets => {
                    if let Some(sec) = self.secrets.get(self.sec_idx) {
                        self.secret_to_delete = Some(sec.id.clone());
                    }
                }
                Focus::Members => {
                    if let Some(member) = self.members.get(self.member_idx) {
                        if member.id != self.session.member_id {
                            self.member_to_delete = Some(member.id.clone());
                        }
                    }
                }
            },
            KeyCode::Char('g') => {
                if self.focus == Focus::Members {
                    if let Some(member) = self.members.get(self.member_idx) {
                        if member.wrapped_dek.is_empty() {
                            self.member_to_grant = Some(member.id.clone());
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                if self.focus == Focus::Members {
                    self.confirming_rotate = true;
                }
            }
            KeyCode::Char('y') => {
                if self.focus == Focus::Secrets {
                    if let Some(sec) = self.secrets.get(self.sec_idx) {
                        let value = sec.value.clone();
                        self.clipboard_ok = self.clipboard
                            .as_mut()
                            .map(|cb| cb.set_text(value).is_ok())
                            .unwrap_or(false);
                        if self.clipboard_ok {
                            self.copied_at = Some(std::time::Instant::now());
                        }
                    }
                }
            }
            KeyCode::Char('i') => {
                if self.focus == Focus::Members {
                    let link = self.invite_link.clone();
                    self.clipboard_ok = self.clipboard
                        .as_mut()
                        .map(|cb| cb.set_text(link).is_ok())
                        .unwrap_or(false);
                    self.mode = Mode::Invite;
                }
            }
            _ => {}
        }

        Ok(false)
    }

    fn handle_form(&mut self, key: KeyEvent) -> Result<bool> {
        if self.is_textarea_field() {
            return self.handle_textarea(key);
        }

        let fields = if self.mode == Mode::NewSecret || self.mode == Mode::EditSecret {
            SECRET_FIELDS
        } else {
            NAMESPACE_FIELDS
        };

        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::List;
            }
            KeyCode::Up => {
                self.go_back_field();
            }
            KeyCode::Down | KeyCode::Enter => {
                if self.field_idx < fields.len() - 1 {
                    self.advance_field();
                } else if key.code == KeyCode::Enter {
                    self.submit_form()?;
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor < self.field_input.len() {
                    self.cursor += 1;
                }
            }
            KeyCode::Home => {
                self.cursor = 0;
            }
            KeyCode::End => {
                self.cursor = self.field_input.len();
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let mut chars: Vec<char> = self.field_input.chars().collect();
                    chars.remove(self.cursor - 1);
                    self.field_input = chars.into_iter().collect();
                    self.cursor -= 1;
                }
            }
            KeyCode::Delete => {
                let len = self.field_input.chars().count();
                if self.cursor < len {
                    let mut chars: Vec<char> = self.field_input.chars().collect();
                    chars.remove(self.cursor);
                    self.field_input = chars.into_iter().collect();
                }
            }
            KeyCode::Char(c) => {
                let mut chars: Vec<char> = self.field_input.chars().collect();
                chars.insert(self.cursor, c);
                self.field_input = chars.into_iter().collect();
                self.cursor += 1;
            }
            _ => {}
        }

        Ok(false)
    }

    fn handle_textarea(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::List;
            }
            // Tab advances to the next field
            KeyCode::Tab => {
                self.advance_field();
            }
            // Enter inserts a newline
            KeyCode::Enter => {
                let row = self.ta_row;
                let col = self.ta_col;
                let chars: Vec<char> = self.ta_lines[row].chars().collect();
                let before: String = chars[..col].iter().collect();
                let after: String = chars[col..].iter().collect();
                self.ta_lines[row] = before;
                self.ta_lines.insert(row + 1, after);
                self.ta_row += 1;
                self.ta_col = 0;
            }
            KeyCode::Backspace => {
                let row = self.ta_row;
                let col = self.ta_col;
                if col > 0 {
                    let mut chars: Vec<char> = self.ta_lines[row].chars().collect();
                    chars.remove(col - 1);
                    self.ta_lines[row] = chars.into_iter().collect();
                    self.ta_col -= 1;
                } else if row > 0 {
                    let current = self.ta_lines.remove(row);
                    let prev_len = self.ta_lines[row - 1].chars().count();
                    self.ta_lines[row - 1].push_str(&current);
                    self.ta_row -= 1;
                    self.ta_col = prev_len;
                }
            }
            KeyCode::Delete => {
                let row = self.ta_row;
                let col = self.ta_col;
                let line_len = self.ta_lines[row].chars().count();
                if col < line_len {
                    let mut chars: Vec<char> = self.ta_lines[row].chars().collect();
                    chars.remove(col);
                    self.ta_lines[row] = chars.into_iter().collect();
                } else if row + 1 < self.ta_lines.len() {
                    let next = self.ta_lines.remove(row + 1);
                    self.ta_lines[row].push_str(&next);
                }
            }
            KeyCode::Left => {
                if self.ta_col > 0 {
                    self.ta_col -= 1;
                } else if self.ta_row > 0 {
                    self.ta_row -= 1;
                    self.ta_col = self.ta_lines[self.ta_row].chars().count();
                }
            }
            KeyCode::Right => {
                let line_len = self.ta_lines[self.ta_row].chars().count();
                if self.ta_col < line_len {
                    self.ta_col += 1;
                } else if self.ta_row + 1 < self.ta_lines.len() {
                    self.ta_row += 1;
                    self.ta_col = 0;
                }
            }
            KeyCode::Up => {
                if self.ta_row > 0 {
                    self.ta_row -= 1;
                    let line_len = self.ta_lines[self.ta_row].chars().count();
                    self.ta_col = self.ta_col.min(line_len);
                } else {
                    self.go_back_field();
                }
            }
            KeyCode::Down => {
                if self.ta_row + 1 < self.ta_lines.len() {
                    self.ta_row += 1;
                    let line_len = self.ta_lines[self.ta_row].chars().count();
                    self.ta_col = self.ta_col.min(line_len);
                } else {
                    self.advance_field();
                }
            }
            KeyCode::Home => {
                self.ta_col = 0;
            }
            KeyCode::End => {
                self.ta_col = self.ta_lines[self.ta_row].chars().count();
            }
            KeyCode::Char(c) => {
                let row = self.ta_row;
                let col = self.ta_col;
                let mut chars: Vec<char> = self.ta_lines[row].chars().collect();
                chars.insert(col, c);
                self.ta_lines[row] = chars.into_iter().collect();
                self.ta_col += 1;
            }
            _ => {}
        }

        self.update_ta_scroll();
        Ok(false)
    }

    fn update_ta_scroll(&mut self) {
        // Keep the cursor row inside the visible viewport.
        // Textarea block is Constraint::Length(6) → 4 inner lines (6 - 2 borders).
        const VIEWPORT: usize = 4;
        if self.ta_row < self.ta_scroll {
            self.ta_scroll = self.ta_row;
        } else if self.ta_row >= self.ta_scroll + VIEWPORT {
            self.ta_scroll = self.ta_row + 1 - VIEWPORT;
        }
    }

    fn handle_namespace_secrets(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::List;
            }
            KeyCode::Up => {
                if self.ps_cursor > 0 {
                    self.ps_cursor -= 1;
                }
            }
            KeyCode::Down => {
                if self.ps_cursor + 1 < self.secrets.len() {
                    self.ps_cursor += 1;
                }
            }
            KeyCode::Char(' ') => {
                if let Some(secret) = self.secrets.get(self.ps_cursor) {
                    let id = secret.id.clone();
                    if self.ps_selected_ids.contains(&id) {
                        self.ps_selected_ids.remove(&id);
                    } else {
                        self.ps_selected_ids.insert(id);
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(id) = self.editing_id.clone() {
                    let ids: Vec<String> = self.ps_selected_ids.iter().cloned().collect();
                    set_namespace_secrets(&mut self.doc, &id, ids)?;
                    self.refresh()?;
                    self.schedule_persist();
                    self.mode = Mode::List;
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_namespace_delete(&mut self, key: KeyEvent) -> Result<bool> {
        if key.code == KeyCode::Char('y') {
            if let Some(id) = self.namespace_to_delete.take() {
                remove_namespace(&mut self.doc, &id)?;
                if self.ns_idx > 0 {
                    self.ns_idx -= 1;
                }
                self.refresh()?;
                self.schedule_persist();
            }
        } else if key.code == KeyCode::Char('n') || key.code == KeyCode::Esc {
            self.namespace_to_delete = None;
        }
        Ok(false)
    }

    fn handle_secret_delete(&mut self, key: KeyEvent) -> Result<bool> {
        if key.code == KeyCode::Char('y') {
            if let Some(id) = self.secret_to_delete.take() {
                remove_secret(&mut self.doc, &id)?;
                if self.sec_idx > 0 {
                    self.sec_idx -= 1;
                }
                self.refresh()?;
                self.schedule_persist();
            }
        } else if key.code == KeyCode::Char('n') || key.code == KeyCode::Esc {
            self.secret_to_delete = None;
        }
        Ok(false)
    }

    fn handle_member_delete(&mut self, key: KeyEvent) -> Result<bool> {
        if let KeyCode::Char('y') = key.code {
            if let Some(id) = self.member_to_delete.take() {
                let new_dek = remove_member(&mut self.doc, &self.session.dek, &id)?;
                self.session.dek = new_dek;
                if self.member_idx > 0 {
                    self.member_idx -= 1;
                }
                self.refresh()?;
                self.schedule_persist();
            }
        } else if key.code == KeyCode::Char('n') || key.code == KeyCode::Esc {
            self.member_to_delete = None;
        }
        Ok(false)
    }

    fn handle_dek_rotate(&mut self, key: KeyEvent) -> Result<bool> {
        if let KeyCode::Char('y') = key.code {
            self.confirming_rotate = false;
            let new_dek = rotate_dek(&mut self.doc, &self.session.dek)?;
            self.session.dek = new_dek;
            self.refresh()?;
            self.schedule_persist();
        } else if key.code == KeyCode::Char('n') || key.code == KeyCode::Esc {
            self.confirming_rotate = false;
        }
        Ok(false)
    }

    fn handle_member_grant(&mut self, key: KeyEvent) -> Result<bool> {
        if let KeyCode::Char('y') = key.code {
            if let Some(id) = self.member_to_grant.take() {
                if let Some(member) = self.members.iter().find(|m| m.id == id) {
                    let pub_key_bytes = B64
                        .decode(&member.public_key)
                        .map_err(|_| lib::error::Error::DecryptionFailed)?;
                    let pub_key: [u8; 32] = pub_key_bytes
                        .try_into()
                        .map_err(|_| lib::error::Error::DecryptionFailed)?;
                    let wrapped = wrap_dek(&self.session.dek, &pub_key)?;
                    let key_mac = compute_key_mac(
                        &self.session.dek,
                        &member.id,
                        &member.public_key,
                        &member.signing_key,
                    );

                    let mut state: EnviDocument = hydrate(&self.doc)?;
                    if let Some(m) = state.members.get_mut(&id) {
                        m.wrapped_dek = wrapped;
                        m.key_mac = key_mac;
                    }
                    reconcile(&mut self.doc, &state)?;
                    self.refresh()?;
                    self.schedule_persist();
                }
            }
        } else if key.code == KeyCode::Char('n') || key.code == KeyCode::Esc {
            self.member_to_grant = None;
        }
        Ok(false)
    }
}

// --- Helpers ---

fn all_values_array(v: &[String]) -> [String; 4] {
    [
        v.get(0).cloned().unwrap_or_default(),
        v.get(1).cloned().unwrap_or_default(),
        v.get(2).cloned().unwrap_or_default(),
        v.get(3).cloned().unwrap_or_default(),
    ]
}

fn split_tags(s: &str) -> Vec<String> {
    s.split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}
