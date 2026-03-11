use automerge::AutoCommit;
use autosurgeon::hydrate;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use crossterm::event::{KeyCode, KeyEvent};
use envilib::{
    crypto::wrap_dek,
    error::Result,
    projects::{add_project, remove_project, set_project_secrets, update_project},
    secrets::{add_secret, list_secrets, remove_secret, update_secret, PlaintextSecretFields},
    store::{Session, Store},
    types::{EnviDocument, Member, PlaintextSecret, Project},
};
use std::sync::Arc;

// --- Mode ---

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    List,
    NewSecret,
    NewProject,
    EditSecret,
    EditProject,
    ProjectSecrets,
    Invite,
}

// --- Focus ---

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Projects,
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

pub const PROJECT_FIELDS: &[FormField] = &[FormField {
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
    pub proj_idx: usize,
    pub sec_idx: usize,
    pub member_idx: usize,

    pub show_values: bool,
    pub syncing: bool,

    // Derived data (refreshed after every doc change)
    pub projects: Vec<Project>,
    pub secrets: Vec<PlaintextSecret>,
    pub members: Vec<Member>,

    // Form state
    pub field_idx: usize,
    pub field_input: String,
    pub cursor: usize,
    pub collected_values: Vec<String>,
    pub initial_values: Vec<String>,
    pub editing_id: Option<String>,

    // Project-secrets checklist
    pub ps_cursor: usize,
    pub ps_selected_ids: std::collections::HashSet<String>,

    // Confirmation dialogs
    pub member_to_delete: Option<String>,
    pub member_to_grant: Option<String>,

    // Invite clipboard status
    pub clipboard_ok: bool,

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
            focus: Focus::Projects,
            proj_idx: 0,
            sec_idx: 0,
            member_idx: 0,
            show_values: false,
            syncing: false,
            clipboard_ok: false,
            projects: vec![],
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
            member_to_delete: None,
            member_to_grant: None,
            persist_task: None,
        };
        app.refresh()?;
        Ok(app)
    }

    /// Refresh derived state from the automerge document.
    pub fn refresh(&mut self) -> Result<()> {
        let state: EnviDocument = hydrate(&self.doc)?;
        self.projects = state.projects.into_values().collect();
        self.secrets = list_secrets(&self.doc, &self.session.dek)?;
        self.members = state.members.into_values().collect();

        // Clamp indices
        if !self.projects.is_empty() {
            self.proj_idx = self.proj_idx.min(self.projects.len() - 1);
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
        let _member_id = self.session.member_id.clone();

        // Load a fresh doc from bytes to send to the task
        let handle = tokio::spawn(async move {
            if let Ok(mut doc) = AutoCommit::load(&doc_bytes) {
                let _ = store.persist(&mut doc).await;
            }
        });
        self.persist_task = Some(handle);
        self.syncing = true;
    }

    /// Poll the persist task for completion.
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
    }

    fn open_edit_form(&mut self, mode: Mode, id: &str, values: Vec<String>) {
        self.mode = mode;
        self.editing_id = Some(id.to_string());
        self.initial_values = values.clone();
        self.field_idx = 0;
        self.field_input = values.first().cloned().unwrap_or_default();
        self.cursor = self.field_input.len();
        self.collected_values = vec![];
    }

    fn advance_field(&mut self) {
        let value = self.field_input.clone();
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
            Mode::NewProject => {
                let [name, ..] = all_values_array(&all_values);
                add_project(&mut self.doc, &name)?;
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
            Mode::EditProject => {
                if let Some(id) = self.editing_id.clone() {
                    let [name, ..] = all_values_array(&all_values);
                    update_project(&mut self.doc, &id, &name)?;
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
        // Member delete confirmation
        if self.member_to_delete.is_some() {
            return self.handle_member_delete(key);
        }

        // Member grant confirmation
        if self.member_to_grant.is_some() {
            return self.handle_member_grant(key);
        }

        match &self.mode.clone() {
            Mode::Invite => {
                if key.code == KeyCode::Esc {
                    self.mode = Mode::List;
                }
                return Ok(false);
            }
            Mode::ProjectSecrets => {
                return self.handle_project_secrets(key);
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
                    Focus::Projects => Focus::Secrets,
                    Focus::Secrets => Focus::Members,
                    Focus::Members => Focus::Projects,
                };
            }
            KeyCode::Char('v') => {
                self.show_values = !self.show_values;
            }
            KeyCode::Char('n') => {
                if self.focus != Focus::Members {
                    let mode = if self.focus == Focus::Projects {
                        Mode::NewProject
                    } else {
                        Mode::NewSecret
                    };
                    self.open_new_form(mode);
                }
            }
            KeyCode::Up => match &self.focus {
                Focus::Projects => {
                    if self.proj_idx > 0 {
                        self.proj_idx -= 1;
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
                Focus::Projects => {
                    if self.proj_idx + 1 < self.projects.len() {
                        self.proj_idx += 1;
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
                Focus::Projects => {
                    if let Some(proj) = self.projects.get(self.proj_idx) {
                        let proj = proj.clone();
                        self.open_edit_form(Mode::EditProject, &proj.id, vec![proj.name.clone()]);
                    }
                }
                Focus::Members => {}
            },
            KeyCode::Char('s') => {
                if self.focus == Focus::Projects {
                    if let Some(proj) = self.projects.get(self.proj_idx) {
                        let proj_id = proj.id.clone();
                        let selected: std::collections::HashSet<String> =
                            proj.secret_ids.iter().cloned().collect();
                        self.ps_cursor = 0;
                        self.ps_selected_ids = selected;
                        self.editing_id = Some(proj_id);
                        self.mode = Mode::ProjectSecrets;
                    }
                }
            }
            KeyCode::Char('d') => match &self.focus {
                Focus::Projects => {
                    if let Some(proj) = self.projects.get(self.proj_idx) {
                        let id = proj.id.clone();
                        remove_project(&mut self.doc, &id)?;
                        if self.proj_idx > 0 {
                            self.proj_idx -= 1;
                        }
                        self.refresh()?;
                        self.schedule_persist();
                    }
                }
                Focus::Secrets => {
                    if let Some(sec) = self.secrets.get(self.sec_idx) {
                        let id = sec.id.clone();
                        remove_secret(&mut self.doc, &id)?;
                        if self.sec_idx > 0 {
                            self.sec_idx -= 1;
                        }
                        self.refresh()?;
                        self.schedule_persist();
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
            KeyCode::Char('i') => {
                if self.focus == Focus::Members {
                    self.clipboard_ok = arboard::Clipboard::new()
                        .and_then(|mut cb| cb.set_text(self.invite_link.clone()))
                        .is_ok();
                    self.mode = Mode::Invite;
                }
            }
            _ => {}
        }

        Ok(false)
    }

    fn handle_form(&mut self, key: KeyEvent) -> Result<bool> {
        let fields = if self.mode == Mode::NewSecret || self.mode == Mode::EditSecret {
            SECRET_FIELDS
        } else {
            PROJECT_FIELDS
        };

        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::List;
            }
            KeyCode::Enter => {
                if self.field_idx < fields.len() - 1 {
                    self.advance_field();
                } else {
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

    fn handle_project_secrets(&mut self, key: KeyEvent) -> Result<bool> {
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
                    set_project_secrets(&mut self.doc, &id, ids)?;
                    self.refresh()?;
                    self.schedule_persist();
                    self.mode = Mode::List;
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_member_delete(&mut self, key: KeyEvent) -> Result<bool> {
        if let KeyCode::Char('y') = key.code {
            if let Some(id) = self.member_to_delete.take() {
                use autosurgeon::reconcile;
                let mut state: EnviDocument = hydrate(&self.doc)?;
                state.members.remove(&id);
                reconcile(&mut self.doc, &state)?;
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

    fn handle_member_grant(&mut self, key: KeyEvent) -> Result<bool> {
        if let KeyCode::Char('y') = key.code {
            if let Some(id) = self.member_to_grant.take() {
                if let Some(member) = self.members.iter().find(|m| m.id == id) {
                    let pub_key_bytes = B64
                        .decode(&member.public_key)
                        .map_err(|_| envilib::error::Error::DecryptionFailed)?;
                    let pub_key: [u8; 32] = pub_key_bytes
                        .try_into()
                        .map_err(|_| envilib::error::Error::DecryptionFailed)?;
                    let wrapped = wrap_dek(&self.session.dek, &pub_key)?;

                    use autosurgeon::reconcile;
                    let mut state: EnviDocument = hydrate(&self.doc)?;
                    if let Some(m) = state.members.get_mut(&id) {
                        m.wrapped_dek = wrapped;
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
