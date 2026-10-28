mod i18n;
use crossterm::{
    cursor::Show,
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use key_switch_core::*;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::{
    io::{self, IsTerminal},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};
use unicode_width::UnicodeWidthStr;
use zeroize::Zeroizing;

struct TerminalGuard;
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableBracketedPaste,
        Show
    );
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}
impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let guard = Self;
        execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
        Ok(guard)
    }
}
struct DesktopClipboard(Mutex<Option<arboard::Clipboard>>);
impl Clipboard for DesktopClipboard {
    fn write(&self, value: &str) -> Result<(), CommandError> {
        let mut clipboard = self
            .0
            .lock()
            .map_err(|_| CommandError::from("无法写入剪贴板"))?;
        if clipboard.is_none() {
            *clipboard =
                Some(arboard::Clipboard::new().map_err(|_| CommandError::from("无法写入剪贴板"))?);
        }
        clipboard
            .as_mut()
            .ok_or("无法写入剪贴板")?
            .set_text(value.to_owned())
            .map_err(|_| "无法写入剪贴板".into())
    }
}
enum Job {
    Reload,
    CreateProvider(CreateProviderInput),
    UpdateProvider(UpdateProviderInput),
    DeleteProvider(String),
    Reorder(Vec<String>),
    CreateKey(CreateKeyInput),
    UpdateKey(UpdateKeyInput),
    DeleteKey(String),
    Copy(String),
    Check(String, String),
    CheckAll(String),
}
type Reply = Result<Vec<ProviderSummary>, CommandError>;
fn worker(
    core: Arc<Core>,
    jobs: mpsc::Receiver<Job>,
    replies: mpsc::Sender<Reply>,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) {
    let clipboard = DesktopClipboard(Mutex::new(None));
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(_) => {
            let _ = replies.send(Err("无法初始化网络客户端".into()));
            return;
        }
    };
    for job in jobs {
        if *cancel.borrow() {
            break;
        }
        let result = match job {
            Job::Reload => Ok(()),
            Job::CreateProvider(input) => create_provider(&core, input).map(|_| ()),
            Job::UpdateProvider(input) => update_provider(&core, input).map(|_| ()),
            Job::DeleteProvider(id) => delete_provider(&core, id),
            Job::Reorder(ids) => reorder_providers(&core, ids),
            Job::CreateKey(input) => create_api_key(&core, input).map(|_| ()),
            Job::UpdateKey(input) => update_api_key(&core, input).map(|_| ()),
            Job::DeleteKey(id) => delete_api_key(&core, id),
            Job::Copy(id) => copy_api_key(&core, id, &clipboard),
            Job::Check(p, k) => {
                let outcome=runtime.block_on(async {tokio::select!{result=check_api_key(&core,p,k)=>Some(result),_=cancel.changed()=>None}});
                let Some(result) = outcome else {
                    break;
                };
                result.map(|_| ())
            }
            Job::CheckAll(p) => {
                let outcome=runtime.block_on(async {tokio::select!{result=check_provider_keys(&core,p)=>Some(result),_=cancel.changed()=>None}});
                let Some(result) = outcome else {
                    break;
                };
                result.map(|_| ())
            }
        };
        if replies
            .send(result.and_then(|_| list_providers(&core)))
            .is_err()
        {
            break;
        }
    }
}
fn safe(value: &str) -> String {
    value
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
        })
        .collect()
}
struct Field {
    label: &'static str,
    value: Zeroizing<String>,
    hidden: bool,
    cursor: usize,
}
impl Field {
    fn new(label: &'static str, value: String, hidden: bool) -> Self {
        let cursor = value.chars().count();
        Self {
            label,
            value: Zeroizing::new(value),
            hidden,
            cursor,
        }
    }
    fn insert(&mut self, value: &str) {
        let value = Zeroizing::new(safe(value));
        // Bound paste/form size. No input buffer or secret is displayed in errors.
        if self.value.len() + value.len() > 16 * 1024 {
            return;
        }
        let index = self
            .value
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        self.value.insert_str(index, &value);
        self.cursor += value.chars().count();
    }
    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            let index = self
                .value
                .char_indices()
                .nth(self.cursor)
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.value.remove(index);
        }
    }
}
enum FormKind {
    Provider(Option<String>),
    Key(String, Option<String>),
}
struct Form {
    kind: FormKind,
    fields: Vec<Field>,
    active: usize,
}
enum Modal {
    Form(Form),
    Delete(Job),
    Catalog(usize),
    Search,
}
struct App {
    providers: Vec<ProviderSummary>,
    provider_id: Option<String>,
    key_id: Option<String>,
    keys_pane: bool,
    search: String,
    sorted: bool,
    modal: Option<Modal>,
    pending_form: Option<Form>,
    busy: bool,
    failed: bool,
    status: String,
    locale: Locale,
}
impl App {
    fn apply_reply(&mut self, reply: Reply) {
        self.busy = false;
        let form = self.pending_form.take();
        match reply {
            Ok(providers) => {
                self.failed = false;
                self.providers = providers;
                self.reconcile();
                self.status = i18n::text(self.locale, "ready").into();
            }
            Err(error) => {
                self.fail(error.code);
                if let Some(form) = form {
                    self.modal = Some(Modal::Form(form));
                }
            }
        }
    }
    fn fail(&mut self, code: &str) {
        self.failed = true;
        self.status = i18n::error(self.locale, code);
    }
    fn provider(&self) -> Option<&ProviderSummary> {
        self.providers
            .iter()
            .find(|p| Some(&p.id) == self.provider_id.as_ref())
    }
    fn visible_providers(&self) -> Vec<&ProviderSummary> {
        self.providers
            .iter()
            .filter(|p| {
                self.keys_pane || p.name.to_lowercase().contains(&self.search.to_lowercase())
            })
            .collect()
    }
    fn visible_keys(&self) -> Vec<&ApiKeySummary> {
        let mut keys: Vec<_> = self
            .provider()
            .map(|p| {
                p.keys
                    .iter()
                    .filter(|k| {
                        !self.keys_pane
                            || format!("{} {}", k.remark, k.masked_value)
                                .to_lowercase()
                                .contains(&self.search.to_lowercase())
                    })
                    .collect()
            })
            .unwrap_or_default();
        if self.sorted {
            keys.sort_by(|a, b| {
                a.remark
                    .to_lowercase()
                    .cmp(&b.remark.to_lowercase())
                    .then(a.id.cmp(&b.id))
            });
        }
        keys
    }
    fn key(&self) -> Option<&ApiKeySummary> {
        self.visible_keys()
            .into_iter()
            .find(|k| Some(&k.id) == self.key_id.as_ref())
    }
    fn reconcile(&mut self) {
        if !self
            .visible_providers()
            .iter()
            .any(|p| Some(&p.id) == self.provider_id.as_ref())
        {
            self.provider_id = self.visible_providers().first().map(|p| p.id.clone());
        }
        if !self
            .visible_keys()
            .iter()
            .any(|k| Some(&k.id) == self.key_id.as_ref())
        {
            self.key_id = self.visible_keys().first().map(|k| k.id.clone());
        }
    }
    fn navigate(&mut self, delta: isize) {
        let ids: Vec<_> = if self.keys_pane {
            self.visible_keys().iter().map(|k| k.id.clone()).collect()
        } else {
            self.visible_providers()
                .iter()
                .map(|p| p.id.clone())
                .collect()
        };
        let selected = if self.keys_pane {
            &mut self.key_id
        } else {
            &mut self.provider_id
        };
        if !ids.is_empty() {
            let index = ids
                .iter()
                .position(|id| Some(id) == selected.as_ref())
                .unwrap_or(0);
            *selected = Some(
                ids[(index as isize + delta).clamp(0, ids.len() as isize - 1) as usize].clone(),
            );
        }
        self.reconcile();
    }
    fn send(&mut self, sender: &mpsc::Sender<Job>, job: Job) {
        if self.busy {
            return;
        }
        if sender.send(job).is_ok() {
            self.busy = true;
            self.failed = false;
            self.status = i18n::text(self.locale, "loading").into();
        } else {
            self.fail("TRANSACTION_UNAVAILABLE");
        }
    }
    fn form(&mut self, edit: bool) {
        if self.keys_pane {
            let Some(provider) = self.provider() else {
                return;
            };
            let provider_id = provider.id.clone();
            let key = if edit { self.key() } else { None };
            if edit && key.is_none() {
                return;
            }
            let id = key.map(|k| k.id.clone());
            let remark = key.map(|k| k.remark.clone()).unwrap_or_default();
            self.modal = Some(Modal::Form(Form {
                kind: FormKind::Key(provider_id, id),
                fields: vec![
                    Field::new("remark", remark, false),
                    Field::new("secret", String::new(), true),
                ],
                active: 0,
            }));
        } else {
            let provider = if edit { self.provider() } else { None };
            if edit && provider.is_none() {
                return;
            }
            let (mode, endpoint, header) = match provider.map(|p| &p.validation) {
                Some(ValidationConfig::OpenAiCompatible { base_url }) => {
                    ("openai-compatible", base_url.clone(), String::new())
                }
                Some(ValidationConfig::Bearer { endpoint }) => {
                    ("bearer", endpoint.clone(), String::new())
                }
                Some(ValidationConfig::ApiKeyHeader {
                    endpoint,
                    header_name,
                }) => ("api-key-header", endpoint.clone(), header_name.clone()),
                _ => ("none", String::new(), String::new()),
            };
            let id = provider.map(|p| p.id.clone());
            let name = provider.map(|p| p.name.clone()).unwrap_or_default();
            let url = provider
                .and_then(|p| p.platform_url.clone())
                .unwrap_or_default();
            let mut fields = vec![
                Field::new("name", name, false),
                Field::new("url", url, false),
                Field::new("validation", mode.into(), false),
                Field::new("endpoint", endpoint, false),
                Field::new("header", header, false),
            ];
            if provider.is_some_and(|p| p.kind == "builtin") {
                fields.truncate(2);
            }
            self.modal = Some(Modal::Form(Form {
                kind: FormKind::Provider(id),
                fields,
                active: 0,
            }));
        }
    }
}
fn submit(form: &Form) -> Result<Job, CommandError> {
    let take = |index: usize| {
        form.fields
            .get(index)
            .map(|f| f.value.to_string())
            .unwrap_or_default()
    };
    match &form.kind {
        FormKind::Key(provider_id, id) => {
            let provider_id = provider_id.clone();
            let remark = take(0);
            let value = take(1);
            Ok(if let Some(id) = id.clone() {
                Job::UpdateKey(UpdateKeyInput { id, remark, value })
            } else {
                Job::CreateKey(CreateKeyInput {
                    provider_id,
                    remark,
                    value,
                })
            })
        }
        FormKind::Provider(id) => {
            let name = take(0);
            let url = take(1);
            let mode = take(2);
            let endpoint = take(3);
            let header_name = take(4);
            let validation = match mode.trim() {
                "" | "none" => ValidationConfig::None,
                "openai-compatible" => ValidationConfig::OpenAiCompatible { base_url: endpoint },
                "bearer" => ValidationConfig::Bearer { endpoint },
                "api-key-header" => ValidationConfig::ApiKeyHeader {
                    endpoint,
                    header_name,
                },
                _ => return Err("供应商检测配置无效".into()),
            };
            let platform_url = if url.trim().is_empty() {
                None
            } else {
                Some(url)
            };
            Ok(if let Some(id) = id.clone() {
                Job::UpdateProvider(UpdateProviderInput {
                    id,
                    name,
                    platform_url,
                    validation,
                })
            } else {
                Job::CreateProvider(CreateProviderInput {
                    id: format!("custom-{}", unique_id()),
                    name,
                    abbreviation: "CU".into(),
                    tone: "gray".into(),
                    logo: None,
                    kind: "custom".into(),
                    platform_url,
                    validation,
                })
            })
        }
    }
}
fn unique_id() -> String {
    key_switch_core::new_id()
}
fn handle(app: &mut App, event: KeyEvent, sender: &mpsc::Sender<Job>) -> bool {
    if event.kind == KeyEventKind::Release {
        return false;
    }
    if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('c') {
        return true;
    }
    if let Some(modal) = app.modal.take() {
        match modal {
            Modal::Form(mut form) => {
                match event.code {
                    KeyCode::Esc => return false,
                    KeyCode::Enter => {
                        match submit(&form) {
                            Ok(job) => {
                                app.send(sender, job);
                                if app.busy {
                                    app.pending_form = Some(form);
                                } else {
                                    app.modal = Some(Modal::Form(form));
                                }
                            }
                            Err(e) => {
                                app.fail(e.code);
                                app.modal = Some(Modal::Form(form));
                            }
                        };
                        return false;
                    }
                    KeyCode::Tab | KeyCode::Down => {
                        form.active = (form.active + 1) % form.fields.len()
                    }
                    KeyCode::BackTab | KeyCode::Up => {
                        form.active = (form.active + form.fields.len() - 1) % form.fields.len()
                    }
                    KeyCode::Backspace => form.fields[form.active].backspace(),
                    KeyCode::Left => {
                        form.fields[form.active].cursor =
                            form.fields[form.active].cursor.saturating_sub(1)
                    }
                    KeyCode::Right => {
                        let field = &mut form.fields[form.active];
                        field.cursor = (field.cursor + 1).min(field.value.chars().count());
                    }
                    KeyCode::Home => form.fields[form.active].cursor = 0,
                    KeyCode::End => {
                        let field = &mut form.fields[form.active];
                        field.cursor = field.value.chars().count();
                    }
                    KeyCode::Char(c) if !event.modifiers.contains(KeyModifiers::CONTROL) => {
                        form.fields[form.active].insert(&c.to_string())
                    }
                    _ => {}
                }
                app.modal = Some(Modal::Form(form));
            }
            Modal::Delete(job) => match event.code {
                KeyCode::Enter => app.send(sender, job),
                KeyCode::Esc => {}
                _ => app.modal = Some(Modal::Delete(job)),
            },
            Modal::Catalog(mut index) => {
                let catalog = provider_catalog();
                match event.code {
                    KeyCode::Esc => return false,
                    KeyCode::Down => index = (index + 1).min(catalog.len() - 1),
                    KeyCode::Up => index = index.saturating_sub(1),
                    KeyCode::Enter => {
                        let p = &catalog[index];
                        let name = if matches!(app.locale, Locale::ZhCn | Locale::ZhTw) {
                            p.name_zh.clone()
                        } else {
                            p.name_en.clone()
                        };
                        app.send(
                            sender,
                            Job::CreateProvider(CreateProviderInput {
                                id: p.id.clone(),
                                name,
                                abbreviation: p.abbreviation.clone(),
                                tone: p.tone.clone(),
                                logo: None,
                                kind: "builtin".into(),
                                platform_url: Some(p.platform_url.clone()),
                                validation: ValidationConfig::None,
                            }),
                        );
                        return false;
                    }
                    _ => {}
                }
                app.modal = Some(Modal::Catalog(index));
            }
            Modal::Search => {
                match event.code {
                    KeyCode::Esc => app.search.clear(),
                    KeyCode::Enter => {}
                    KeyCode::Backspace => {
                        app.search.pop();
                        app.modal = Some(Modal::Search);
                    }
                    KeyCode::Char(c) => {
                        app.search.push(c);
                        app.search = safe(&app.search);
                        app.modal = Some(Modal::Search);
                    }
                    _ => app.modal = Some(Modal::Search),
                };
                app.reconcile();
            }
        }
        return false;
    }
    if matches!(event.code, KeyCode::Char('q') | KeyCode::Esc) {
        if app.search.is_empty() {
            return true;
        }
        app.search.clear();
        app.reconcile();
        return false;
    }
    if event.modifiers.contains(KeyModifiers::ALT)
        && matches!(event.code, KeyCode::Up | KeyCode::Down)
    {
        if !app.busy && !app.keys_pane && app.search.is_empty() {
            let mut ids: Vec<_> = app.providers.iter().map(|p| p.id.clone()).collect();
            if let Some(index) = ids
                .iter()
                .position(|id| Some(id) == app.provider_id.as_ref())
            {
                let next = if event.code == KeyCode::Up {
                    index.saturating_sub(1)
                } else {
                    (index + 1).min(ids.len() - 1)
                };
                ids.swap(index, next);
                app.send(sender, Job::Reorder(ids));
            }
        }
        return false;
    }
    match event.code {
        KeyCode::Up => app.navigate(-1),
        KeyCode::Down => app.navigate(1),
        KeyCode::Tab | KeyCode::BackTab | KeyCode::Enter => {
            app.keys_pane = !app.keys_pane;
            app.search.clear();
            app.reconcile();
        }
        KeyCode::Char('/') => app.modal = Some(Modal::Search),
        KeyCode::Char('s') => {
            app.sorted = !app.sorted;
            app.reconcile();
        }
        KeyCode::Char('r') if !app.busy => app.send(sender, Job::Reload),
        KeyCode::Char('n') if !app.busy => app.form(false),
        KeyCode::Char('e') if !app.busy => app.form(true),
        KeyCode::Char('b') if !app.busy => app.modal = Some(Modal::Catalog(0)),
        KeyCode::Char('d') if !app.busy => {
            let job = if app.keys_pane {
                app.key().map(|k| Job::DeleteKey(k.id.clone()))
            } else {
                app.provider().map(|p| Job::DeleteProvider(p.id.clone()))
            };
            app.modal = job.map(Modal::Delete);
        }
        KeyCode::Char('c') if !app.busy => {
            if let Some(k) = app.key() {
                let id = k.id.clone();
                app.send(sender, Job::Copy(id));
            }
        }
        KeyCode::Char('t') if !app.busy => {
            if let (Some(p), Some(k)) = (app.provider(), app.key()) {
                if p.validation_supported {
                    let job = Job::Check(p.id.clone(), k.id.clone());
                    app.send(sender, job);
                }
            }
        }
        KeyCode::Char('a') if !app.busy => {
            if let Some(p) = app.provider() {
                if p.validation_supported {
                    let id = p.id.clone();
                    app.send(sender, Job::CheckAll(id));
                }
            }
        }
        _ => {}
    }
    false
}
fn draw(frame: &mut ratatui::Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(2),
            Constraint::Length(4),
        ])
        .split(frame.area());
    let panes = Layout::default()
        .direction(if frame.area().width < 75 {
            Direction::Vertical
        } else {
            Direction::Horizontal
        })
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(outer[0]);
    let providers = app.visible_providers();
    let keys = app.visible_keys();
    let highlight = Style::default().fg(Color::Black).bg(Color::Cyan);
    let block = |label: &str, focused: bool| {
        Block::default()
            .borders(Borders::ALL)
            .title(label.to_owned())
            .border_style(Style::default().fg(if focused { Color::Cyan } else { Color::Gray }))
    };
    let mut provider_state = ListState::default().with_selected(
        providers
            .iter()
            .position(|p| Some(&p.id) == app.provider_id.as_ref()),
    );
    frame.render_stateful_widget(
        List::new(providers.iter().map(|p| ListItem::new(safe(&p.name))))
            .block(block(i18n::text(app.locale, "providers"), !app.keys_pane))
            .highlight_style(highlight),
        panes[0],
        &mut provider_state,
    );
    let mut key_state = ListState::default()
        .with_selected(keys.iter().position(|k| Some(&k.id) == app.key_id.as_ref()));
    frame.render_stateful_widget(
        List::new(keys.iter().map(|k| {
            ListItem::new(format!(
                "{}  {}  {}",
                safe(&k.remark),
                safe(&k.masked_value),
                if let Some(code) = &k.check_error_code {
                    format!(
                        "{}: {}",
                        i18n::text(app.locale, &k.status),
                        i18n::error(app.locale, code)
                    )
                } else {
                    i18n::text(app.locale, &k.status).to_owned()
                }
            ))
        }))
        .block(block(i18n::text(app.locale, "keys"), app.keys_pane))
        .highlight_style(highlight),
        panes[1],
        &mut key_state,
    );
    let status = if app.busy {
        i18n::text(app.locale, "loading").to_owned()
    } else if app.failed {
        app.status.clone()
    } else if providers.is_empty() || (app.keys_pane && keys.is_empty()) {
        i18n::text(app.locale, "empty").to_owned()
    } else {
        app.status.clone()
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{}  {}{}",
            status,
            i18n::text(app.locale, "search"),
            safe(&app.search)
        ))
        .wrap(Wrap { trim: false }),
        outer[1],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "{}\n{}",
            i18n::text(app.locale, "help"),
            i18n::text(app.locale, "order")
        ))
        .wrap(Wrap { trim: false }),
        outer[2],
    );
    if let Some(modal) = &app.modal {
        let area = frame.area();
        let popup = ratatui::layout::Rect::new(
            area.x + area.width / 20,
            area.y + area.height / 10,
            area.width * 9 / 10,
            area.height * 8 / 10,
        );
        frame.render_widget(Clear, popup);
        match modal {
            Modal::Form(form) => {
                let mut lines = form
                    .fields
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| {
                        popup.height as usize >= form.fields.len() * 3 + 2 || *index == form.active
                    })
                    .map(|(i, f)| {
                        let mut value = if f.hidden {
                            "•".repeat(f.value.chars().count().min(80))
                        } else {
                            safe(&f.value)
                        };
                        if i == form.active {
                            let index = value
                                .char_indices()
                                .nth(f.cursor)
                                .map(|(offset, _)| offset)
                                .unwrap_or(value.len());
                            value.insert(index, '│');
                            // Keep the cursor in view for long or wide Unicode fields.
                            let available = popup.width.saturating_sub(4) as usize;
                            while UnicodeWidthStr::width(value.as_str()) > available
                                && !value.starts_with('│')
                            {
                                value.remove(0);
                            }
                        }
                        let prefix = if i == form.active { "> " } else { "  " };
                        format!("{}{}: {}", prefix, i18n::text(app.locale, f.label), value)
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                if form.fields.len() > 2
                    && !form.fields[2].value.trim().is_empty()
                    && form.fields[2].value.trim() != "none"
                {
                    lines.push_str("\n\n");
                    lines.push_str(i18n::text(app.locale, "validationNotice"));
                }
                frame.render_widget(
                    Paragraph::new(lines)
                        .block(block(i18n::text(app.locale, "formHelp"), true))
                        .wrap(Wrap { trim: false }),
                    popup,
                );
            }
            Modal::Delete(_) => frame.render_widget(
                Paragraph::new(i18n::text(app.locale, "confirm"))
                    .block(block("Key Switch", true))
                    .wrap(Wrap { trim: false }),
                popup,
            ),
            Modal::Search => frame.render_widget(
                Paragraph::new(format!(
                    "{}{}",
                    i18n::text(app.locale, "search"),
                    safe(&app.search)
                ))
                .block(block(i18n::text(app.locale, "searchHelp"), true)),
                popup,
            ),
            Modal::Catalog(index) => {
                let catalog = provider_catalog();
                let mut state = ListState::default().with_selected(Some(*index));
                frame.render_stateful_widget(
                    List::new(catalog.iter().map(|p| {
                        ListItem::new(if matches!(app.locale, Locale::ZhCn | Locale::ZhTw) {
                            p.name_zh.clone()
                        } else {
                            p.name_en.clone()
                        })
                    }))
                    .block(block(i18n::text(app.locale, "providers"), true))
                    .highlight_style(highlight),
                    popup,
                    &mut state,
                );
            }
        }
    }
}
fn run(core: Arc<Core>, locale: Locale) -> io::Result<()> {
    let (jobs_tx, jobs_rx) = mpsc::channel();
    let (replies_tx, replies_rx) = mpsc::channel();
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    let worker = thread::spawn(move || worker(core, jobs_rx, replies_tx, cancel_rx));
    let outcome = (|| {
        let _guard = TerminalGuard::enter()?;
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        let mut app = App {
            providers: vec![],
            provider_id: None,
            key_id: None,
            keys_pane: false,
            search: String::new(),
            sorted: false,
            modal: None,
            pending_form: None,
            busy: false,
            failed: false,
            status: String::new(),
            locale,
        };
        app.send(&jobs_tx, Job::Reload);
        loop {
            loop {
                let reply = match replies_rx.try_recv() {
                    Ok(reply) => reply,
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return Err(io::Error::other("background task failed"))
                    }
                };
                app.apply_reply(reply);
            }
            terminal.draw(|frame| draw(frame, &app))?;
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) => {
                        if handle(&mut app, key, &jobs_tx) {
                            break;
                        }
                    }
                    Event::Paste(value) => {
                        let value = Zeroizing::new(value);
                        if let Some(Modal::Form(form)) = &mut app.modal {
                            form.fields[form.active].insert(&value);
                        } else if matches!(app.modal, Some(Modal::Search)) {
                            app.search.push_str(&safe(&value));
                            app.reconcile();
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    })();
    let _ = cancel_tx.send(true);
    drop(jobs_tx);
    let _ = worker.join();
    outcome
}
fn main() {
    let mut locale = system_locale();
    let mut arguments = std::env::args_os().skip(1);
    if let Some(argument) = arguments.next() {
        if arguments.next().is_none() && argument == "--version" {
            println!("Key Switch TUI {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        if argument == "--help" {
            println!(
                "Key Switch TUI {}\n{}\n{}",
                env!("CARGO_PKG_VERSION"),
                i18n::text(locale, "help"),
                i18n::text(locale, "order")
            );
            return;
        }
        eprintln!("{}", i18n::text(locale, "arguments"));
        std::process::exit(1);
    }
    let unsupported_environment = std::env::var_os("SSH_CONNECTION").is_some()
        || std::env::var_os("SSH_TTY").is_some()
        || std::env::var_os("WSL_DISTRO_NAME").is_some()
        || (cfg!(target_os = "linux")
            && std::env::var_os("DISPLAY").is_none()
            && std::env::var_os("WAYLAND_DISPLAY").is_none());
    if !cfg!(any(target_os = "windows", target_os = "linux"))
        || unsupported_environment
        || !io::stdin().is_terminal()
        || !io::stdout().is_terminal()
    {
        eprintln!("{}", i18n::text(locale, "unsupportedPlatform"));
        std::process::exit(1);
    }
    let lock = application_directory()
        .and_then(|p| InstanceLock::acquire(&p))
        .unwrap_or_else(|e| {
            eprintln!("{}", startup_message(locale, e));
            std::process::exit(1)
        });
    let core = Arc::new(
        Core::new(lock, Arc::new(SystemCredentials)).unwrap_or_else(|e| {
            eprintln!("{}", i18n::error(locale, e.code));
            std::process::exit(1)
        }),
    );
    match read_app_settings(&core) {
        Ok(settings) => {
            if settings.locale_preference != "system" {
                locale = resolve_locale(Some(&settings.locale_preference));
            }
        }
        Err(e) => {
            eprintln!("{}", i18n::error(locale, e.code));
            std::process::exit(1)
        }
    }
    std::panic::set_hook(Box::new(move |_| {
        restore_terminal();
        eprintln!("{}", i18n::text(locale, "terminal"));
    }));
    if run(core, locale).is_err() {
        eprintln!("{}", i18n::text(locale, "terminal"));
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_form(form: Form) -> App {
        App {
            providers: vec![],
            provider_id: None,
            key_id: None,
            keys_pane: false,
            search: String::new(),
            sorted: false,
            modal: Some(Modal::Form(form)),
            pending_form: None,
            busy: false,
            failed: false,
            status: String::new(),
            locale: Locale::ZhCn,
        }
    }

    #[test]
    fn invalid_validation_mode_keeps_form_and_can_be_corrected() {
        let form = Form {
            kind: FormKind::Provider(None),
            fields: vec![
                Field::new("name", "我的供应商".into(), false),
                Field::new("url", "https://example.com".into(), false),
                Field::new("validation", "bearre".into(), false),
                Field::new("endpoint", "https://example.com/check".into(), false),
                Field::new("header", "X-Example".into(), false),
            ],
            active: 2,
        };
        let original: Vec<_> = form
            .fields
            .iter()
            .map(|f| (f.value.to_string(), f.cursor))
            .collect();
        let mut app = app_with_form(form);
        let (tx, rx) = mpsc::channel();
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert!(!handle(&mut app, enter, &tx));
        assert!(app.failed && !app.busy);
        assert!(matches!(rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        let Some(Modal::Form(form)) = &mut app.modal else {
            panic!("validation failure discarded the form");
        };
        assert_eq!(form.active, 2);
        assert_eq!(
            form.fields
                .iter()
                .map(|f| (f.value.to_string(), f.cursor))
                .collect::<Vec<_>>(),
            original
        );
        form.fields[2] = Field::new("validation", "bearer".into(), false);
        handle(&mut app, enter, &tx);
        assert!(app.busy && app.modal.is_none() && app.pending_form.is_some());
        let Job::CreateProvider(input) = rx.try_recv().unwrap() else {
            panic!("corrected form did not submit provider creation");
        };
        assert_eq!(input.name, "我的供应商");
        assert_eq!(input.platform_url.as_deref(), Some("https://example.com"));
        assert!(
            matches!(input.validation, ValidationConfig::Bearer { endpoint } if endpoint == "https://example.com/check")
        );
        app.apply_reply(Ok(vec![]));
        assert!(!app.busy && !app.failed && app.pending_form.is_none() && app.modal.is_none());
    }

    #[test]
    fn background_failure_restores_hidden_input_and_allows_retry_or_cancel() {
        let mut app = app_with_form(Form {
            kind: FormKind::Key("p".into(), None),
            fields: vec![
                Field::new("remark", "备注".into(), false),
                Field::new("secret", "fixture-private-value".into(), true),
            ],
            active: 1,
        });
        let (tx, rx) = mpsc::channel();
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        handle(&mut app, enter, &tx);
        drop(rx.try_recv().unwrap());
        app.apply_reply(Err("无法写入系统密钥库".into()));
        let Some(Modal::Form(form)) = &app.modal else {
            panic!("background failure discarded the form");
        };
        assert_eq!(form.active, 1);
        assert_eq!(&**form.fields[0].value, "备注");
        assert_eq!(&**form.fields[1].value, "fixture-private-value");
        assert!(form.fields[1].hidden);
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(100, 24)).unwrap();
        terminal.draw(|frame| draw(frame, &app)).unwrap();
        assert!(!format!("{:?}", terminal.backend().buffer()).contains("fixture-private-value"));
        handle(&mut app, enter, &tx);
        assert!(matches!(rx.try_recv(), Ok(Job::CreateKey(_))));
        app.apply_reply(Err("无法保存本地数据".into()));
        handle(
            &mut app,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &tx,
        );
        assert!(app.modal.is_none() && app.pending_form.is_none());
    }

    #[test]
    fn disconnected_worker_keeps_form() {
        let mut app = app_with_form(Form {
            kind: FormKind::Key("p".into(), None),
            fields: vec![Field::new("remark", "备注".into(), false)],
            active: 0,
        });
        let (tx, rx) = mpsc::channel();
        drop(rx);
        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &tx,
        );
        assert!(app.failed && !app.busy && app.pending_form.is_none());
        assert!(matches!(app.modal, Some(Modal::Form(_))));
    }
    #[test]
    fn strips_terminal_controls_and_edits_unicode() {
        assert_eq!(safe("a\u{1b}[31m\n中\u{202e}"), "a[31m中");
        let mut field = Field::new("secret", "中文".into(), true);
        field.cursor = 1;
        field.insert("日");
        assert_eq!(&*field.value, "中日文");
        field.backspace();
        assert_eq!(&*field.value, "中文");
    }
    #[test]
    fn renders_small_terminal_without_secret() {
        let app = App {
            providers: vec![],
            provider_id: None,
            key_id: None,
            keys_pane: false,
            search: String::new(),
            sorted: false,
            modal: Some(Modal::Form(Form {
                kind: FormKind::Key("p".into(), None),
                fields: vec![Field::new("secret", "fixture-private-value".into(), true)],
                active: 0,
            })),
            pending_form: None,
            busy: false,
            failed: false,
            status: String::new(),
            locale: Locale::ZhCn,
        };
        for (width, height) in [(20, 8), (80, 24), (1, 1)] {
            let backend = ratatui::backend::TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal.draw(|f| draw(f, &app)).unwrap();
            let output = format!("{:?}", terminal.backend().buffer());
            assert!(!output.contains("fixture-private-value"));
        }
    }

    #[test]
    fn loading_failure_is_visible_when_list_is_empty() {
        let mut app = App {
            providers: vec![],
            provider_id: None,
            key_id: None,
            keys_pane: false,
            search: String::new(),
            sorted: false,
            modal: None,
            pending_form: None,
            busy: false,
            failed: false,
            status: String::new(),
            locale: Locale::En,
        };
        app.fail("DATA_READ_FAILED");
        let backend = ratatui::backend::TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &app)).unwrap();
        let text = format!("{:?}", terminal.backend().buffer());
        assert!(text.contains("Could not read local data"));
    }
}
