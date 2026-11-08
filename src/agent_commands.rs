//! Shared CLI/GUI operations. Persistence succeeds before live state is replaced.
use crate::{
    app::{App, Cmd, Phase},
    local_calendar::{self, Draft, Entry},
    model::{self, Shelf, TodoItem, TodoList},
    smtc::Transport,
};
use chrono::{Local, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub enum Command {
    #[serde(rename = "tasks.list")]
    TasksList {
        limit: Option<usize>,
        offset: Option<usize>,
    },
    #[serde(rename = "tasks.add")]
    TasksAdd {
        text: String,
        due: Option<String>,
        repeat: Option<u8>,
    },
    #[serde(rename = "tasks.update")]
    TasksUpdate {
        reference: String,
        text: Option<String>,
        due: Option<String>,
        repeat: Option<u8>,
    },
    #[serde(rename = "tasks.complete")]
    TasksComplete { reference: String },
    #[serde(rename = "tasks.reopen")]
    TasksReopen { reference: String },
    #[serde(rename = "tasks.remove")]
    TasksRemove { reference: String },
    #[serde(rename = "calendar.list")]
    CalendarList {
        limit: Option<usize>,
        offset: Option<usize>,
    },
    #[serde(rename = "calendar.add")]
    CalendarAdd {
        title: String,
        start: String,
        end: String,
        all_day: Option<bool>,
        reminder: Option<u32>,
    },
    #[serde(rename = "calendar.remove")]
    CalendarRemove { reference: String },
    #[serde(rename = "calendar.status")]
    CalendarStatus {},
    #[serde(rename = "calendar.sync")]
    CalendarSync {},
    #[serde(rename = "shelf.list")]
    ShelfList {
        limit: Option<usize>,
        offset: Option<usize>,
    },
    #[serde(rename = "shelf.add")]
    ShelfAdd { path: String },
    #[serde(rename = "shelf.remove")]
    ShelfRemove { path: String },
    #[serde(rename = "focus.status")]
    FocusStatus {},
    #[serde(rename = "focus.start")]
    FocusStart {},
    #[serde(rename = "focus.pause")]
    FocusPause {},
    #[serde(rename = "focus.reset")]
    FocusReset {},
    #[serde(rename = "focus.phase")]
    FocusPhase { phase: String },
    #[serde(rename = "water.status")]
    WaterStatus {},
    #[serde(rename = "water.drink")]
    WaterDrink { ml: u32 },
    #[serde(rename = "water.undo")]
    WaterUndo {},
    #[serde(rename = "music.status")]
    MusicStatus {},
    #[serde(rename = "music.toggle")]
    MusicToggle {},
    #[serde(rename = "music.next")]
    MusicNext {},
    #[serde(rename = "music.previous")]
    MusicPrevious {},
    #[serde(rename = "music.seek")]
    MusicSeek { seconds: f64 },
    #[serde(rename = "music.volume")]
    MusicVolume { percent: u32 },
    #[serde(rename = "music.mute")]
    MusicMute {},
    #[serde(rename = "mail.status")]
    MailStatus {},
    #[serde(rename = "mail.list")]
    MailList {
        limit: Option<usize>,
        offset: Option<usize>,
    },
    #[serde(rename = "mail.configure")]
    MailConfigure {
        email: Option<String>,
        provider: Option<String>,
        host: Option<String>,
        port: Option<u16>,
        username: Option<String>,
        enabled: Option<bool>,
        replace_secret: Option<bool>,
    },
    #[serde(rename = "mail.remove")]
    MailRemove { email: String },
    #[serde(rename = "mail.input-status")]
    MailInputStatus { id: String },
    #[serde(rename = "mail.input-cancel")]
    MailInputCancel { id: String },
    #[serde(rename = "notifications.status")]
    NotificationsStatus {},
    #[serde(rename = "quota.status")]
    QuotaStatus {},
    #[serde(rename = "quota.refresh")]
    QuotaRefresh {},
    #[serde(rename = "app.status")]
    AppStatus {},
}

// One source for CLI flags and Agent discovery. Required fields remain enforced by serde.
const SPECS: &[(&str, &str, bool)] = &[
    ("tasks.list", "limit:uint offset:uint", true),
    ("tasks.add", "text:string due:string repeat:uint", true),
    (
        "tasks.update",
        "reference:string text:string due:string repeat:uint",
        true,
    ),
    ("tasks.complete", "reference:string", true),
    ("tasks.reopen", "reference:string", true),
    ("tasks.remove", "reference:string", true),
    ("calendar.list", "limit:uint offset:uint", true),
    (
        "calendar.add",
        "title:string start:string end:string all_day:bool reminder:uint",
        true,
    ),
    ("calendar.remove", "reference:string", true),
    ("calendar.status", "", false),
    ("calendar.sync", "", false),
    ("shelf.list", "limit:uint offset:uint", true),
    ("shelf.add", "path:string", true),
    ("shelf.remove", "path:string", true),
    ("focus.status", "", false),
    ("focus.start", "", false),
    ("focus.pause", "", false),
    ("focus.reset", "", false),
    ("focus.phase", "phase:string", false),
    ("water.status", "", false),
    ("water.drink", "ml:uint", false),
    ("water.undo", "", false),
    ("music.status", "", false),
    ("music.toggle", "", false),
    ("music.next", "", false),
    ("music.previous", "", false),
    ("music.seek", "seconds:number", false),
    ("music.volume", "percent:uint", false),
    ("music.mute", "", false),
    ("mail.status", "", false),
    ("mail.list","limit:uint offset:uint",true),
    ("mail.configure","email:string provider:string host:string port:uint username:string enabled:bool replace_secret:bool",false),
    ("mail.remove","email:string",false),
    ("mail.input-status","id:string",false),
    ("mail.input-cancel","id:string",false),
    ("notifications.status", "", false),
    ("quota.status", "", false),
    ("quota.refresh", "", false),
    ("app.status", "", false),
];

pub fn capabilities() -> Value {
    json!({"protocol":1,"commands":SPECS.iter().map(|(op,fields,offline)|json!({
        "command":op.replace('.'," "),"flags":fields.split_whitespace().collect::<Vec<_>>(),"offline":offline
    })).collect::<Vec<_>>(),"settings":"settings list|get|set|apply (existing interface)",
        "interaction_policy":{"scope":"all Agent/CLI operations","ordinary_fields":"command arguments; missing fields return requirements or validation errors, never a configuration form","sensitive_fields":"local secret-only prompt or supported provider OAuth browser consent; never command arguments, stdin, IPC, returned values or logs","prompt":"purpose and destination, one masked field, cancel/confirm; reuse available credentials","unsupported_secret_configuration":"report unsupported; never request the secret in chat or edit credential stores directly"},
        "required":{"tasks.add":["text"],"tasks.update":["reference"],"tasks.complete":["reference"],
            "tasks.reopen":["reference"],"tasks.remove":["reference"],"calendar.add":["title","start","end"],
            "calendar.remove":["reference"],"shelf.add":["path"],"shelf.remove":["path"],
            "focus.phase":["phase"],"water.drink":["ml"],"music.seek":["seconds"],"music.volume":["percent"],
            "mail.remove":["email"],"mail.input-status":["id"],"mail.input-cancel":["id"]},
        "rules":{"time":"local YYYY-MM-DD HH:MM; app must run for reminders to fire",
            "repeat":"0=once, bit0=Monday .. bit6=Sunday; 0..127",
            "due":"tasks update --due none clears reminder", "phase":["focus","rest"],
            "pagination":"limit 1..100 (default 50), offset >=0; result includes total",
            "reference":"snapshot reference, re-list after changes; stale references rejected",
            "calendar":"local entries only; all-day start/end are YYYY-MM-DD, end day inclusive; reminder omitted disables it, 0=on time (all-day 09:00), max 10080 minutes",
            "water":"ml 1..6000; undo last live drink only", "music":"seek in seconds within current duration; volume 0..100; transport actions return accepted, re-query status",
            "timeout":"outcome unknown: query before retrying any mutation",
            "mail":"configure returns needs_metadata for missing ordinary fields, without opening a window. Supply these through the Agent. Only missing passwords open a compact secret-only prompt. Poll mail input-status --id ID while awaiting_input; never retry a pending mutation. Saved does not mean connection verified.",
            "mail_providers":crate::mail::Provider::ALL.iter().map(|p|p.key()).collect::<Vec<_>>(),
            "outlook":"Public desktop app Client ID: settings set outlook_client_id CLIENT_ID. Configure with provider outlook. awaiting_authorization means Microsoft browser consent; poll mail input-status or cancel with mail input-cancel. Authorization codes and tokens are never returned.",
            "secrets":"password values, login files and autostart are not exposed; password flags and password JSON fields are rejected"}})
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.len() < 2 {
        return Err("Use capabilities to discover commands".into());
    }
    let op = format!("{}.{}", args[0], args[1]);
    let (_, fields, _) = SPECS
        .iter()
        .find(|(name, _, _)| *name == op)
        .ok_or("Unknown command; use capabilities")?;
    if (args.len() - 2) % 2 != 0 {
        return Err("Flags require values, e.g. --text TITLE".into());
    }
    let mut row = serde_json::Map::new();
    row.insert("op".into(), op.into());
    for pair in args[2..].chunks_exact(2) {
        let key = pair[0]
            .strip_prefix("--")
            .ok_or("Expected --flag VALUE")?
            .replace('-', "_");
        let ty = fields
            .split_whitespace()
            .find_map(|field| {
                field
                    .split_once(':')
                    .filter(|(name, _)| *name == key)
                    .map(|(_, ty)| ty)
            })
            .ok_or_else(|| format!("Unknown flag: {}", pair[0]))?;
        if row.contains_key(&key) {
            return Err(format!("Duplicate flag: {}", pair[0]));
        }
        let value = match ty {
            "uint" => json!(pair[1]
                .parse::<u64>()
                .map_err(|_| "Expected unsigned integer")?),
            "number" => {
                let n = pair[1].parse::<f64>().map_err(|_| "Expected number")?;
                if !n.is_finite() {
                    return Err("Expected finite number".into());
                }
                json!(n)
            }
            "bool" => json!(pair[1]
                .parse::<bool>()
                .map_err(|_| "Expected true or false")?),
            _ => json!(pair[1]),
        };
        row.insert(key, value);
    }
    // Resolve paths in the caller's directory, never the GUI's installation directory.
    if let Some(Value::String(path)) = row.get_mut("path") {
        let p = PathBuf::from(&*path);
        let absolute = if p.is_absolute() {
            p
        } else {
            std::env::current_dir().map_err(|e| e.to_string())?.join(p)
        };
        *path = absolute.to_str().ok_or("Path must be Unicode")?.into();
    }
    serde_json::from_value(Value::Object(row)).map_err(|e| e.to_string())
}

fn text(value: &str) -> Result<String, String> {
    let s = value.trim();
    if s.is_empty() || s.len() > 4096 || s.chars().any(char::is_control) {
        return Err("Text must be 1..4096 bytes without control characters".into());
    }
    Ok(s.into())
}

fn validate_task(item: &mut TodoItem, validate_due: bool) -> Result<(), String> {
    item.text = text(&item.text)?;
    if item.text.starts_with("@due:") {
        return Err("Reserved task prefix: @due:".into());
    }
    if item.repeat > 127 {
        return Err("repeat must be 0..127".into());
    }
    if let Some(due) = &item.due {
        let dt = NaiveDateTime::parse_from_str(due, "%Y-%m-%d %H:%M")
            .map_err(|_| "due must be YYYY-MM-DD HH:MM")?;
        if dt.format("%Y-%m-%d %H:%M").to_string() != *due {
            return Err("Use canonical YYYY-MM-DD HH:MM".into());
        }
        let at = dt
            .and_local_timezone(Local)
            .single()
            .ok_or("Local time is ambiguous or does not exist")?;
        if validate_due && item.repeat == 0 && at < Local::now() {
            return Err("Reminder must be in the future".into());
        }
    }
    Ok(())
}

fn reference(index: usize, data: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, data);
    format!(
        "{index}-{}",
        digest
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
fn task_row(index: usize, item: &TodoItem, revision: &str) -> Value {
    json!({"reference":format!("{index}:{revision}"),"text":item.text,"due":item.due,
        "repeat":item.repeat,"done_on":item.done_on,"done_today":item.done_for(&Local::now().format("%Y-%m-%d").to_string()),
        "reminded":item.reminded,"reminded_on":item.reminded_on,"snoozed_until":item.snoozed_until,"reminder_pending":item.reminder_pending})
}
fn page(
    rows: impl Iterator<Item = Value>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Value, String> {
    let limit = limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err("limit must be 1..100".into());
    }
    let offset = offset.unwrap_or(0);
    let mut total = 0;
    let mut items = Vec::new();
    for row in rows {
        if total >= offset && items.len() < limit {
            items.push(row);
        }
        total += 1;
    }
    Ok(json!({"items":items,"total":total,"offset":offset,"limit":limit}))
}

fn task_command(list: &mut TodoList, command: &Command, path: &Path) -> Result<Value, String> {
    let revision = reference(0, list.serialize().as_bytes());
    if let Command::TasksList { limit, offset } = command {
        return page(
            list.items
                .iter()
                .enumerate()
                .map(|(i, t)| task_row(i, t, &revision)),
            *limit,
            *offset,
        );
    }
    let mut next = list.clone();
    let index;
    match command {
        Command::TasksAdd { text, due, repeat } => {
            if next.items.len() >= 10000 {
                return Err("Task limit reached".into());
            }
            let mut item = TodoItem {
                text: text.clone(),
                due: due.clone(),
                repeat: repeat.unwrap_or(0),
                ..Default::default()
            };
            validate_task(&mut item, true)?;
            index = next.items.len();
            next.items.push(item);
        }
        Command::TasksUpdate { reference, .. }
        | Command::TasksComplete { reference }
        | Command::TasksReopen { reference }
        | Command::TasksRemove { reference } => {
            index = next
                .items
                .iter()
                .enumerate()
                .position(|(i, t)| {
                    task_row(i, t, &revision)["reference"].as_str() == Some(reference)
                })
                .ok_or("Stale or missing task reference; list tasks again")?;
            let item = &mut next.items[index];
            match command {
                Command::TasksUpdate {
                    text, due, repeat, ..
                } => {
                    if text.is_none() && due.is_none() && repeat.is_none() {
                        return Err("No task changes supplied".into());
                    }
                    if let Some(v) = text {
                        item.text = v.clone();
                    }
                    if let Some(v) = due {
                        item.due = (v != "none").then(|| v.clone());
                    }
                    if let Some(v) = repeat {
                        if *v != item.repeat {
                            item.done_on = None;
                        }
                        item.repeat = *v;
                    }
                    validate_task(item, due.is_some() || repeat.is_some())?;
                    if due.is_some() || repeat.is_some() {
                        item.reminded = false;
                        item.reminded_on = None;
                        item.snoozed_until = None;
                        item.reminder_pending = false;
                    }
                }
                Command::TasksComplete { .. } => {
                    item.done_on = Some(Local::now().format("%Y-%m-%d").to_string());
                    item.snoozed_until = None;
                    item.reminder_pending = false;
                }
                Command::TasksReopen { .. } => {
                    item.done_on = None;
                    item.snoozed_until = None;
                    item.reminder_pending = false;
                }
                Command::TasksRemove { .. } => {
                    next.items.remove(index);
                }
                _ => unreachable!(),
            }
        }
        _ => return Err("Not a task command".into()),
    }
    next.save_to(path)
        .map_err(|e| format!("Cannot save tasks: {e}"))?;
    let revision = reference(0, next.serialize().as_bytes());
    let result = if matches!(command, Command::TasksRemove { .. }) {
        json!({"removed":true})
    } else {
        task_row(index, &next.items[index], &revision)
    };
    *list = next;
    Ok(result)
}

fn calendar_row(index: usize, entry: &Entry) -> Value {
    let mut row = json!({"uid":entry.event.uid,"title":entry.event.title,"start":entry.event.start,
        "end":entry.event.end,"all_day":entry.event.all_day,"reminder":entry.reminder});
    row["reference"] = reference(index, row.to_string().as_bytes()).into();
    row
}
fn calendar_command(
    entries: &mut Vec<Entry>,
    command: &Command,
    path: &Path,
) -> Result<Value, String> {
    if let Command::CalendarList { limit, offset } = command {
        return page(
            entries.iter().enumerate().map(|(i, e)| calendar_row(i, e)),
            *limit,
            *offset,
        );
    }
    let mut next = entries.clone();
    let result = match command {
        Command::CalendarAdd {
            title,
            start,
            end,
            all_day,
            reminder,
        } => {
            if next.len() >= 5000 {
                return Err("Calendar entry limit reached".into());
            }
            if reminder.is_some_and(|m| m > 10080) {
                return Err("reminder must be 0..10080 minutes".into());
            }
            let all_day = all_day.unwrap_or(false);
            let split = |s: &str| -> Result<(String, String), String> {
                if all_day {
                    let d = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .map_err(|_| "Use YYYY-MM-DD")?;
                    if d.to_string() != s {
                        return Err("Use canonical YYYY-MM-DD".into());
                    }
                    Ok((s.into(), "00:00".into()))
                } else {
                    let dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")
                        .map_err(|_| "Use YYYY-MM-DD HH:MM")?;
                    if dt.format("%Y-%m-%d %H:%M").to_string() != s {
                        return Err("Use canonical YYYY-MM-DD HH:MM".into());
                    }
                    Ok((dt.date().to_string(), dt.format("%H:%M").to_string()))
                }
            };
            let (sd, st) = split(start)?;
            let (ed, et) = split(end)?;
            let mut draft = Draft::new(Local::now().date_naive());
            draft.title = text(title)?;
            draft.start_date = sd;
            draft.start_time = st;
            draft.end_date = ed;
            draft.end_time = et;
            draft.all_day = all_day;
            draft.reminder = *reminder;
            let entry = draft.entry()?;
            let row = calendar_row(next.len(), &entry);
            next.push(entry);
            row
        }
        Command::CalendarRemove { reference } => {
            let index = next
                .iter()
                .enumerate()
                .position(|(i, e)| calendar_row(i, e)["reference"].as_str() == Some(reference))
                .ok_or("Stale or missing calendar reference; list calendar again")?;
            let removed = next.remove(index);
            json!({"removed":true,"uid":removed.event.uid})
        }
        _ => return Err("Not a local calendar command".into()),
    };
    local_calendar::save_to(&next, path)?;
    *entries = next;
    Ok(result)
}

fn shelf_command(shelf: &mut Shelf, command: &Command, path: &Path) -> Result<Value, String> {
    if let Command::ShelfList { limit, offset } = command {
        return page(
            shelf
                .items
                .iter()
                .map(|p| json!({"path":p,"exists":p.exists()})),
            *limit,
            *offset,
        );
    }
    let (path_value, add) = match command {
        Command::ShelfAdd { path } => (path, true),
        Command::ShelfRemove { path } => (path, false),
        _ => return Err("Not a shelf command".into()),
    };
    let value = text(path_value)?;
    if value != *path_value {
        return Err("Path has surrounding whitespace".into());
    }
    let p = PathBuf::from(value);
    if !p.is_absolute() {
        return Err("Path must be absolute".into());
    }
    let mut next = shelf.clone();
    if add {
        if !p.exists() {
            return Err("File does not exist".into());
        }
        if !next.items.contains(&p) {
            next.items.push(p.clone());
        }
    } else {
        if !next.items.contains(&p) {
            return Err("Shelf reference does not exist".into());
        }
        next.items.retain(|item| item != &p);
    }
    let body = next
        .items
        .iter()
        .map(|p| format!("{}\n", p.display()))
        .collect::<String>();
    model::atomic_write(path, body.as_bytes()).map_err(|e| e.to_string())?;
    *shelf = next;
    Ok(json!({"path":p,"added":add,"removed":!add,"original_deleted":false}))
}

fn local_domain(command: &Command) -> Option<&'static str> {
    match command {
        Command::TasksList { .. }
        | Command::TasksAdd { .. }
        | Command::TasksUpdate { .. }
        | Command::TasksComplete { .. }
        | Command::TasksReopen { .. }
        | Command::TasksRemove { .. } => Some("tasks"),
        Command::CalendarList { .. }
        | Command::CalendarAdd { .. }
        | Command::CalendarRemove { .. } => Some("calendar"),
        Command::ShelfList { .. } | Command::ShelfAdd { .. } | Command::ShelfRemove { .. } => {
            Some("shelf")
        }
        _ => None,
    }
}

pub fn offline(directory: &Path, command: &Command) -> Result<Value, String> {
    if let Command::MailList { limit, offset } = command {
        let accounts = crate::mail::load_accounts_from(&directory.join("mail-accounts.json"))?;
        return page(accounts.iter().map(crate::agent_mail::row), *limit, *offset);
    }
    let domain = local_domain(command).ok_or("This command requires a running islo application")?;
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("agent-data.lock"))
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    match domain {
        "tasks" => {
            let path = directory.join("todo.txt");
            let mut list = TodoList::load_from(&path).map_err(|e| e.to_string())?;
            task_command(&mut list, command, &path)
        }
        "calendar" => {
            let path = directory.join("local-calendar.json");
            let mut entries = local_calendar::load_from(&path)?;
            calendar_command(&mut entries, command, &path)
        }
        _ => {
            let path = directory.join("shelf.txt");
            let body = match fs::read_to_string(&path) {
                Ok(body) => body,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(e) => return Err(e.to_string()),
            };
            let mut shelf = Shelf {
                items: body
                    .lines()
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| PathBuf::from(s.trim()))
                    .collect(),
            };
            shelf_command(&mut shelf, command, &path)
        }
    }
}

pub(crate) fn live(app: &mut App, command: &Command, directory: &Path) -> Result<Value, String> {
    match command {
        Command::MailList { limit, offset } => {
            return page(
                app.mail.accounts.iter().map(crate::agent_mail::row),
                *limit,
                *offset,
            )
        }
        Command::MailConfigure {
            email,
            provider,
            host,
            port,
            username,
            enabled,
            replace_secret,
        } => {
            return crate::agent_mail::configure(
                app,
                crate::agent_mail::Change {
                    email: email.clone(),
                    provider: provider.clone(),
                    host: host.clone(),
                    port: *port,
                    username: username.clone(),
                    enabled: *enabled,
                    replace_secret: *replace_secret,
                },
                directory,
            )
        }
        Command::MailRemove { email } => return crate::agent_mail::remove(app, email, directory),
        Command::MailInputStatus { id } => return crate::agent_mail::input_status(app, id),
        Command::MailInputCancel { id } => return crate::agent_mail::cancel(app, id),
        _ => {}
    }
    match local_domain(command) {
        Some("tasks") => {
            if !matches!(command, Command::TasksList { .. })
                && (app.adding || app.editing.is_some() || app.task_dialog.is_some())
            {
                return Err("Task editor is busy; finish or cancel the unsaved edit first".into());
            }
            let old = app.todos.clone();
            let result = task_command(&mut app.todos, command, &directory.join("todo.txt"))?;
            if !matches!(
                command,
                Command::TasksList { .. } | Command::TasksAdd { .. }
            ) {
                let target = match command {
                    Command::TasksUpdate { reference, .. }
                    | Command::TasksComplete { reference }
                    | Command::TasksReopen { reference }
                    | Command::TasksRemove { reference } => reference,
                    _ => unreachable!(),
                };
                let revision = reference(0, old.serialize().as_bytes());
                if let Some((_, item)) =
                    old.items.iter().enumerate().find(|(i, t)| {
                        task_row(*i, t, &revision)["reference"].as_str() == Some(target)
                    })
                {
                    app.alerts.retain(|a| {
                        !(a.page == 4 && a.title == "待办到时间了" && a.body == item.text)
                    });
                    if app.notice.as_ref().is_some_and(|a| {
                        a.page == 4 && a.title == "待办到时间了" && a.body == item.text
                    }) {
                        app.notice = None;
                    }
                }
                app.todo_first = app.todo_first.min(app.todos.items.len().saturating_sub(1));
            }
            return Ok(result);
        }
        Some("calendar") => {
            if !app.calendar.local_error.is_empty() {
                return Err(app.calendar.local_error.clone());
            }
            if !matches!(command, Command::CalendarList { .. }) && app.calendar.draft.is_some() {
                return Err(
                    "Calendar editor is busy; finish or cancel the unsaved edit first".into(),
                );
            }
            let result = calendar_command(
                &mut app.calendar.local,
                command,
                &directory.join("local-calendar.json"),
            )?;
            if matches!(command, Command::CalendarRemove { .. }) {
                let uid = result["uid"].as_str().unwrap();
                app.alerts
                    .retain(|a| !a.meeting.as_ref().is_some_and(|(e, _)| e.uid == uid));
                if app
                    .notice
                    .as_ref()
                    .is_some_and(|a| a.meeting.as_ref().is_some_and(|(e, _)| e.uid == uid))
                {
                    app.notice = None;
                }
                if app.calendar.view.focus.as_ref().is_some_and(|e| e == uid) {
                    app.calendar.view.focus = None;
                }
            }
            return Ok(result);
        }
        Some("shelf") => {
            return shelf_command(&mut app.shelf, command, &directory.join("shelf.txt"))
        }
        _ => {}
    }
    match command {
        Command::FocusStatus {}
        | Command::FocusStart {}
        | Command::FocusPause {}
        | Command::FocusReset {}
        | Command::FocusPhase { .. } => {
            match command {
                Command::FocusStart {} => {
                    if app.pomo.remaining <= 0.0 {
                        app.pomo.remaining = app.phase_secs(app.pomo.phase);
                    }
                    app.pomo.running = true;
                }
                Command::FocusPause {} => app.pomo.running = false,
                Command::FocusReset {} => {
                    app.pomo.running = false;
                    app.pomo.remaining = app.phase_secs(app.pomo.phase);
                }
                Command::FocusPhase { phase } => {
                    let phase = match phase.as_str() {
                        "focus" => Phase::Focus,
                        "rest" => Phase::Rest,
                        _ => return Err("phase must be focus or rest".into()),
                    };
                    if app.pomo.phase != phase {
                        app.pomo.phase = phase;
                        app.pomo.running = false;
                        app.pomo.remaining = app.phase_secs(phase);
                    }
                }
                _ => {}
            }
            Ok(
                json!({"phase":if app.pomo.phase==Phase::Focus{"focus"}else{"rest"},"running":app.pomo.running,"remaining_seconds":app.pomo.remaining,"persistent":false}),
            )
        }
        Command::WaterStatus {} | Command::WaterDrink { .. } | Command::WaterUndo {} => {
            if app.water.start_day != Local::now().format("%Y-%m-%d").to_string() {
                return Err(
                    "Day rollover pending; query again after the application refreshes".into(),
                );
            }
            if !matches!(command, Command::WaterStatus {}) {
                let mut next = app.water.clone();
                let last;
                match command {
                    Command::WaterDrink { ml } => {
                        if !(1..=6000).contains(ml) {
                            return Err("ml must be 1..6000".into());
                        }
                        let hour = Local::now().hour() as usize;
                        let before = next.drunk_ml;
                        next.record(*ml, hour);
                        last = Some((next.drunk_ml - before, hour));
                    }
                    _ => {
                        let (ml, hour) = app.last_drink.ok_or("No drink available to undo")?;
                        next.drunk_ml = next.drunk_ml.saturating_sub(ml);
                        next.hours[hour] = next.hours[hour].saturating_sub(ml);
                        last = None;
                    }
                }
                next.save_progress_to(&directory.join("water-extra.txt"))
                    .map_err(|e| e.to_string())?;
                app.water = next;
                app.last_drink = last;
                app.last_reminder = app.t;
            }
            Ok(
                json!({"day":app.water.start_day,"drunk_ml":app.water.drunk_ml,"goal_ml":app.water.goal_ml,"hours":app.water.hours,"can_undo":app.last_drink.is_some()}),
            )
        }
        Command::MusicStatus {} => Ok(
            json!({"title":app.media.title,"artist":app.media.artist,"source":app.media.source,"playing":app.media.playing,"position_seconds":app.media.position,"duration_seconds":app.media.duration,"volume":app.volume_ui.map(|(p,m)|json!({"percent":p,"muted":m}))}),
        ),
        Command::MusicToggle {}
        | Command::MusicNext {}
        | Command::MusicPrevious {}
        | Command::MusicSeek { .. } => {
            if app.media.title.is_empty() {
                return Err("No active media session".into());
            }
            let transport = match command {
                Command::MusicToggle {} => Transport::Toggle,
                Command::MusicNext {} => Transport::Next,
                Command::MusicPrevious {} => Transport::Prev,
                Command::MusicSeek { seconds } => {
                    if !seconds.is_finite()
                        || *seconds < 0.0
                        || app.media.duration <= 0.0
                        || *seconds > app.media.duration
                    {
                        return Err("seconds must be within the known media duration".into());
                    }
                    Transport::Seek(*seconds)
                }
                _ => unreachable!(),
            };
            app.pending.push(Cmd::Media(transport));
            Ok(json!({"accepted":true,"completed":false}))
        }
        Command::MusicVolume { percent } => {
            if *percent > 100 {
                return Err("percent must be 0..100".into());
            }
            app.pending.push(Cmd::SetVolume(*percent));
            Ok(json!({"accepted":true,"completed":false}))
        }
        Command::MusicMute {} => {
            app.pending.push(Cmd::ToggleMute);
            Ok(json!({"accepted":true,"completed":false}))
        }
        Command::CalendarSync {} => {
            app.pending.push(Cmd::CalendarSync);
            Ok(json!({"accepted":true,"completed":false}))
        }
        Command::CalendarStatus {} => Ok(
            json!({"local_count":app.calendar.local.len(),"local_read_error":!app.calendar.local_error.is_empty(),"feeds":app.calendar.feeds.iter().map(|f|json!({"provider":f.config.provider.key(),"connected":f.connected,"last_sync":f.last_sync,"event_count":f.events.len(),"has_error":!f.error.is_empty()})).collect::<Vec<_>>()}),
        ),
        Command::MailStatus {} => Ok(
            json!({"recent_count":app.mail.recent.len(),"accounts":app.mail.accounts.iter().map(|a|json!({"email":a.email,"provider":a.provider.key(),"enabled":a.enabled,"status":match app.mail.status_of(&a.email){crate::mail::Status::Idle=>"idle",crate::mail::Status::Connecting=>"connecting",crate::mail::Status::Ready=>"ready",crate::mail::Status::Auth(_)=>"auth_required",crate::mail::Status::Failed(_)=>"failed"}})).collect::<Vec<_>>()}),
        ),
        Command::NotificationsStatus {} => Ok(
            json!({"queued":app.alerts.len(),"history_count":app.history.len(),"visible":app.notice.is_some(),"fullscreen_pending":app.todos.items.iter().filter(|t|t.reminder_pending).count(),"fullscreen_enabled":app.settings.fullscreen_task_reminders}),
        ),
        Command::QuotaStatus {} => Ok(
            json!({"accounts":app.quota.config.accounts.iter().map(|a|json!({"id":a.id,"provider":a.provider.name(),"alias":a.alias,"enabled":a.enabled,"pinned":a.pinned,"busy":app.quota.busy.contains(&a.id),"stale":app.quota.stale.contains_key(&a.id),"snapshot":app.quota.rows.get(&a.id).and_then(|r|r.as_ref().ok()),"has_error":app.quota.rows.get(&a.id).is_some_and(|r|r.is_err())})).collect::<Vec<_>>()}),
        ),
        Command::QuotaRefresh {} => {
            app.pending.push(Cmd::QuotaRefresh);
            Ok(json!({"accepted":true,"completed":false}))
        }
        Command::AppStatus {} => Ok(
            json!({"version":env!("ISLO_APP_VERSION"),"local_time":Local::now().to_rfc3339(),"tasks":app.todos.items.len(),"calendar":app.calendar.local.len(),"shelf":app.shelf.items.len(),"task_editor_busy":app.adding||app.editing.is_some()||app.task_dialog.is_some(),"calendar_editor_busy":app.calendar.draft.is_some(),"private_input_pending":app.clipboard.private_input_active()}),
        ),
        _ => Err("Unsupported command".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "islo-agent-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn add(title: &str) -> Command {
        Command::TasksAdd {
            text: title.into(),
            due: None,
            repeat: None,
        }
    }
    #[test]
    fn task_references_protect_against_concurrent_edits_and_deletions() {
        let temp = Temp::new();
        let root = &temp.0;
        offline(root, &add("First")).unwrap();
        let second = offline(root, &add("Second")).unwrap();
        let listing = offline(
            root,
            &Command::TasksList {
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        let first = &listing["items"][0];
        offline(
            root,
            &Command::TasksRemove {
                reference: first["reference"].as_str().unwrap().into(),
            },
        )
        .unwrap();
        let before = fs::read(root.join("todo.txt")).unwrap();
        assert!(
            offline(
                root,
                &Command::TasksComplete {
                    reference: second["reference"].as_str().unwrap().into()
                }
            )
            .is_err(),
            "An old index must never complete a different task"
        );
        assert_eq!(fs::read(root.join("todo.txt")).unwrap(), before);
        let current = offline(
            root,
            &Command::TasksList {
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        let reference = current["items"][0]["reference"]
            .as_str()
            .unwrap()
            .to_string();
        let done = offline(
            root,
            &Command::TasksComplete {
                reference: reference.clone(),
            },
        )
        .unwrap();
        assert_eq!(done["done_today"], true);
        assert!(
            offline(root, &Command::TasksRemove { reference }).is_err(),
            "Completion invalidates old references"
        );
        let undone = offline(
            root,
            &Command::TasksReopen {
                reference: done["reference"].as_str().unwrap().into(),
            },
        )
        .unwrap();
        assert_eq!(undone["done_today"], false);
    }

    #[test]
    fn identical_tasks_cannot_reuse_a_reference_after_deletion() {
        let temp = Temp::new();
        offline(&temp.0, &add("Same title")).unwrap();
        offline(&temp.0, &add("Same title")).unwrap();
        let rows = offline(
            &temp.0,
            &Command::TasksList {
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        let reference = rows["items"][0]["reference"].as_str().unwrap().to_owned();
        offline(
            &temp.0,
            &Command::TasksRemove {
                reference: reference.clone(),
            },
        )
        .unwrap();
        assert!(
            offline(&temp.0, &Command::TasksComplete { reference }).is_err(),
            "Deleting one identical task must not make its reference identify another"
        );
    }
    #[test]
    fn invalid_tasks_and_unreadable_stores_never_overwrite_existing_data() {
        let temp = Temp::new();
        let root = &temp.0;
        offline(root, &add("Keep | this")).unwrap();
        let before = fs::read(root.join("todo.txt")).unwrap();
        for command in [
            add("\n"),
            add("title\nInjected"),
            add("@due:reserved"),
            Command::TasksAdd {
                text: "bad date".into(),
                due: Some("2026-02-30 15:00".into()),
                repeat: None,
            },
            Command::TasksAdd {
                text: "past".into(),
                due: Some("2000-01-01 15:00".into()),
                repeat: None,
            },
            Command::TasksAdd {
                text: "bad mask".into(),
                due: None,
                repeat: Some(128),
            },
        ] {
            assert!(offline(root, &command).is_err());
            assert_eq!(fs::read(root.join("todo.txt")).unwrap(), before);
        }
        let due = (Local::now() + chrono::Duration::days(2))
            .format("%Y-%m-%d %H:%M")
            .to_string();
        let row = offline(
            root,
            &Command::TasksAdd {
                text: "Submit".into(),
                due: Some(due.clone()),
                repeat: None,
            },
        )
        .unwrap();
        assert_eq!(
            TodoList::load_from(&root.join("todo.txt")).unwrap().items[1]
                .due
                .as_deref(),
            Some(due.as_str())
        );
        let changed = offline(
            root,
            &Command::TasksUpdate {
                reference: row["reference"].as_str().unwrap().into(),
                text: None,
                due: Some("none".into()),
                repeat: None,
            },
        )
        .unwrap();
        assert!(changed["due"].is_null());
        fs::write(root.join("todo.txt"), [0xff]).unwrap();
        assert!(offline(root, &add("must fail")).is_err());
        assert_eq!(fs::read(root.join("todo.txt")).unwrap(), [0xff]);
    }
    #[test]
    fn concurrent_agents_preserve_all_task_additions() {
        let temp = Temp::new();
        let threads: Vec<_> = (0..8)
            .map(|n| {
                let path = temp.0.clone();
                std::thread::spawn(move || offline(&path, &add(&format!("Task {n}"))).unwrap())
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let rows = offline(
            &temp.0,
            &Command::TasksList {
                limit: Some(3),
                offset: Some(3),
            },
        )
        .unwrap();
        assert_eq!(rows["total"], 8);
        assert_eq!(rows["items"].as_array().unwrap().len(), 3);
    }
    #[test]
    fn calendar_validation_and_shelf_removal_preserve_user_files() {
        let temp = Temp::new();
        let root = &temp.0;
        let invalid = Command::CalendarAdd {
            title: "Meeting".into(),
            start: "2027-01-02 15:00".into(),
            end: "2027-01-02 14:00".into(),
            all_day: None,
            reminder: None,
        };
        assert!(offline(root, &invalid).is_err());
        assert!(!root.join("local-calendar.json").exists());
        let valid = Command::CalendarAdd {
            title: "Meeting".into(),
            start: "2027-01-02 15:00".into(),
            end: "2027-01-02 16:00".into(),
            all_day: None,
            reminder: Some(15),
        };
        let row = offline(root, &valid).unwrap();
        assert_eq!(
            row["end"].as_i64().unwrap() - row["start"].as_i64().unwrap(),
            3600
        );
        offline(
            root,
            &Command::CalendarRemove {
                reference: row["reference"].as_str().unwrap().into(),
            },
        )
        .unwrap();
        assert!(local_calendar::load_from(&root.join("local-calendar.json"))
            .unwrap()
            .is_empty());
        let original = root.join("original.txt");
        fs::write(&original, "keep").unwrap();
        let path = original.to_str().unwrap().to_owned();
        offline(root, &Command::ShelfAdd { path: path.clone() }).unwrap();
        offline(root, &Command::ShelfRemove { path }).unwrap();
        assert_eq!(fs::read_to_string(original).unwrap(), "keep");
    }
    #[test]
    fn all_agent_commands_keep_raw_secrets_out_of_the_protocol() {
        for (operation,fields,_) in SPECS {
            for field in fields.split_whitespace() {
                let name=field.split(':').next().unwrap();
                assert!(!["password","secret","authorization_code","api_key","token","access_token","refresh_token","private_key"].contains(&name),"{operation} must request {name} through the shared local secret prompt");
            }
        }
    }

    #[test]
    fn parser_and_protocol_fail_on_typos_instead_of_ignoring_them() {
        let parse_strings =
            |args: &[&str]| parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(parse_strings(&["tasks", "add", "--text", "hello"]).is_ok());
        for args in [
            vec!["tasks", "add"],
            vec!["tasks", "add", "--text", "a", "--text", "b"],
            vec!["tasks", "add", "--typo", "a"],
            vec!["focus", "start", "--extra", "x"],
            vec!["music", "seek", "--seconds", "NaN"],
        ] {
            assert!(parse_strings(&args).is_err());
        }
        assert!(serde_json::from_value::<Command>(
            json!({"op":"tasks.add","text":"hello","typo":true})
        )
        .is_err());
        let temp = Temp::new();
        assert!(
            offline(&temp.0, &Command::FocusStart {}).is_err(),
            "Runtime operations must never pretend to work offline"
        );
        // Every advertised command's field types are accepted by the real CLI parser.
        for (op, fields, _) in SPECS {
            let mut args: Vec<String> = op.split('.').map(str::to_string).collect();
            for field in fields.split_whitespace() {
                let (name, ty) = field.split_once(':').unwrap();
                args.push(format!("--{}", name.replace('_', "-")));
                args.push(
                    match ty {
                        "uint" | "number" => "1",
                        "bool" => "true",
                        _ => "example",
                    }
                    .into(),
                );
            }
            assert!(parse(&args).is_ok(), "Advertised parser contract: {op}");
        }
    }
}
