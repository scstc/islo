//! Local secret-only prompts. Ordinary metadata stays in the Agent protocol.
use crate::{
    app::{App, Cmd},
    mail::{self, Account, CustomImap, Provider},
    model,
};
use eframe::egui;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub email: Option<String>,
    pub provider: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub enabled: Option<bool>,
    pub replace_secret: Option<bool>,
}

trait Vault {
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    fn remove(&self, key: &str) -> Result<(), String>;
}
struct NativeVault;
impl Vault for NativeVault {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        mail::secrets::get(key)
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        mail::secrets::save(key, value)
    }
    fn remove(&self, key: &str) -> Result<(), String> {
        mail::secrets::remove(key)
    }
}

pub(crate) fn row(account: &Account) -> Value {
    json!({"email":account.email,"provider":account.provider.key(),"enabled":account.enabled,
        "host":account.endpoint().ok().map(|e|e.0),"port":account.endpoint().ok().map(|e|e.1),
        "username":account.endpoint().ok().map(|e|e.2)})
}
fn existing(accounts: &[Account], email: &str) -> Result<Option<Account>, String> {
    let found: Vec<_> = accounts
        .iter()
        .filter(|a| a.email.eq_ignore_ascii_case(email))
        .collect();
    if found.len() > 1 {
        return Err("Duplicate mailbox addresses; resolve them in settings first".into());
    }
    Ok(found.first().map(|a| (*a).clone()))
}
fn valid_email(email: &str) -> bool {
    email.len() <= 254
        && email.chars().all(|c| !c.is_whitespace() && !c.is_control())
        && email
            .split_once('@')
            .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && !b.contains('@'))
}

struct Form {
    id: String,
    request: Change,
    expected: Option<Account>,
    email: String,
    provider: Option<Provider>,
    custom: CustomImap,
    enabled: bool,
    password: String,
    needs_password: bool,
    checked: Option<Account>,
    created: Instant,
    focus: bool,
    error: String,
}
impl Form {
    fn new(accounts: &[Account], request: Change) -> Result<Self, String> {
        if request.email.as_ref().is_some_and(|e| !valid_email(e)) {
            return Err("Invalid email address".into());
        }
        for value in [&request.host, &request.username].into_iter().flatten() {
            if value.len() > 256 || value.chars().any(char::is_control) {
                return Err("Invalid mailbox field".into());
            }
        }
        let expected = match request.email.as_deref() {
            Some(email) => existing(accounts, email)?,
            None => None,
        };
        if expected.is_none() && accounts.len() >= 100 {
            return Err("Mailbox account limit reached".into());
        }
        let provider = match request.provider.as_deref() {
            Some(key) => Some(
                *Provider::ALL
                    .iter()
                    .find(|p| p.key() == key)
                    .ok_or("Unknown mail provider")?,
            ),
            None => expected.as_ref().map(|a| a.provider),
        };
        if provider.is_some_and(|p| p != Provider::Custom)
            && (request.host.is_some() || request.port.is_some() || request.username.is_some())
        {
            return Err("Endpoint fields require provider custom".into());
        }
        let mut custom = expected
            .as_ref()
            .and_then(|a| a.custom.clone())
            .unwrap_or_default();
        if let Some(host) = &request.host {
            custom.host = host.trim().to_ascii_lowercase();
        }
        if let Some(port) = request.port {
            custom.port = port;
        }
        if let Some(user) = &request.username {
            custom.username = user.trim().into();
        }
        let email = request.email.clone().unwrap_or_default();
        let enabled = request
            .enabled
            .or(expected.as_ref().map(|a| a.enabled))
            .unwrap_or(true);
        let id = format!(
            "mail-{}-{:032x}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "Clock unavailable")?
                .as_nanos()
        );
        Ok(Self {
            id,
            request,
            expected,
            email,
            provider,
            custom,
            enabled,
            password: String::new(),
            needs_password: false,
            checked: None,
            created: Instant::now(),
            focus: true,
            error: String::new(),
        })
    }
    fn account(&self) -> Result<Account, String> {
        if !valid_email(&self.email) {
            return Err("请填写有效邮箱地址".into());
        }
        let provider = self.provider.ok_or("请选择邮箱服务")?;
        let account = Account {
            email: self.email.clone(),
            provider,
            enabled: self.enabled,
            custom: (provider == Provider::Custom).then(|| self.custom.clone()),
        };
        account.endpoint()?;
        Ok(account)
    }
    fn refresh(&mut self, vault: &impl Vault) -> Result<(), String> {
        let Ok(account) = self.account() else {
            self.needs_password = false;
            self.checked = None;
            self.password.clear();
            return Ok(());
        };
        if self.checked.as_ref() == Some(&account) {
            return Ok(());
        }
        self.password.clear();
        let changed = self.expected.as_ref().is_some_and(|old| {
            old.provider != account.provider || old.endpoint().ok() != account.endpoint().ok()
        });
        self.needs_password = self.request.replace_secret.unwrap_or(false)
            || (self.expected.is_none() && account.enabled)
            || (changed && (account.enabled || vault.get(&account.credential_key())?.is_some()))
            || (account.enabled && vault.get(&account.credential_key())?.is_none());
        self.checked = Some(account);
        Ok(())
    }
    fn password_label(&self) -> &'static str {
        match self.provider {
            Some(Provider::Gmail | Provider::ICloud) => "应用专用密码",
            Some(Provider::Custom) => "IMAP 密码或应用专用密码",
            _ => "邮箱授权码",
        }
    }
    fn missing_metadata(&self) -> Vec<Value> {
        let mut fields = vec![];
        if self.email.is_empty() {
            fields.push(json!({"key":"email","label":"邮箱地址","type":"email","sensitive":false}));
        }
        if self.provider.is_none() {
            fields.push(
                json!({"key":"provider","label":"邮箱服务","type":"select","sensitive":false}),
            );
        }
        if self.provider == Some(Provider::Custom) && self.custom.host.trim().is_empty() {
            fields.push(json!({"key":"host","label":"IMAP 主机","type":"text","sensitive":false}));
        }
        fields
    }
    fn public(&self) -> Value {
        let mut fields = vec![];
        if self.needs_password {
            fields.push(json!({"key":"password","label":self.password_label(),"type":"password","sensitive":true}));
        }
        json!({"id":self.id,"status":"awaiting_input","operation":"mail.configure","required_fields":fields,"input_location":"islo local secret dialog","secret_values_returned":false})
    }
}

#[derive(Default)]
pub(crate) struct State {
    active: Option<Form>,
    oauth: Option<OAuthPending>,
    history: VecDeque<Value>,
}
struct OAuthPending {
    form: Form,
    job: crate::outlook::Job,
    url: String,
    requires_cli: bool,
}
impl OAuthPending {
    fn public(&self) -> Value {
        json!({"id":self.form.id,"status":"awaiting_authorization","operation":"mail.configure","authorization_method":"microsoft_browser","secret_values_returned":false})
    }
}
impl State {
    pub(crate) fn is_open(&self) -> bool {
        self.active.is_some() || self.oauth.is_some()
    }
    fn finish(&mut self, mut form: Form, status: &str, result: Option<Value>) -> Value {
        form.password.clear();
        let value = json!({"id":form.id,"status":status,"operation":"mail.configure","result":result,"secret_values_returned":false});
        self.history.push_front(value.clone());
        self.history.truncate(32);
        value
    }
    fn status(&mut self, id: &str) -> Result<Value, String> {
        if let Some(oauth) = self.oauth.as_ref().filter(|o| o.form.id == id) {
            return Ok(oauth.public());
        }
        if let Some(form) = self.active.as_ref().filter(|f| f.id == id) {
            return Ok(form.public());
        }
        self.history
            .iter()
            .find(|s| s["id"].as_str() == Some(id))
            .cloned()
            .ok_or("Unknown or expired input request; application restarts discard requests".into())
    }
}

fn commit(
    accounts: &mut Vec<Account>,
    account: &Account,
    expected: Option<&Account>,
    password: Option<&str>,
    vault: &impl Vault,
    path: &Path,
) -> Result<Value, String> {
    if existing(accounts, &account.email)?.as_ref() != expected {
        return Err("Mailbox changed while input was open; cancel and configure it again".into());
    }
    account.endpoint()?;
    let key = account.credential_key();
    let previous = if password.is_some() || account.enabled {
        vault.get(&key)?
    } else {
        None
    };
    if password.is_none() && account.enabled && previous.is_none() {
        return Err("A local password is required".into());
    }
    let mut next = accounts.clone();
    if let Some(slot) = next.iter_mut().find(|a| mail::key(a) == mail::key(account)) {
        *slot = account.clone();
    } else {
        next.push(account.clone());
    }
    if let Some(password) = password {
        if password.is_empty() || password.len() > 4096 || password.contains(['\r', '\n', '\0']) {
            return Err("授权码不能为空或包含换行符".into());
        }
        vault.set(&key, password)?;
    }
    if let Err(error) = mail::save_accounts_to(&next, path) {
        if password.is_some() {
            let rollback = match previous {
                Some(old) => vault.set(&key, &old),
                None => vault.remove(&key),
            };
            if rollback.is_err() {
                return Err("Mailbox save failed; credential rollback also failed".into());
            }
        }
        return Err(error);
    }
    *accounts = next;
    let mut cleanup = true;
    if let Some(old) = expected.filter(|old| old.credential_key() != key) {
        cleanup = vault.remove(&old.credential_key()).is_ok();
    }
    Ok(
        json!({"account":row(account),"credentials_updated":password.is_some(),"old_credential_cleanup":cleanup,"connection_status":"not_verified"}),
    )
}

pub(crate) fn configure(app: &mut App, change: Change, directory: &Path) -> Result<Value, String> {
    configure_from(app, change, directory, true)
}
pub(crate) fn configure_outlook_gui(app: &mut App, email: String) -> Result<Value, String> {
    configure_from(
        app,
        Change {
            email: Some(email),
            provider: Some("outlook".into()),
            replace_secret: Some(true),
            ..Default::default()
        },
        &model::config_dir(),
        false,
    )
}
fn configure_from(
    app: &mut App,
    change: Change,
    directory: &Path,
    requires_cli: bool,
) -> Result<Value, String> {
    app.expire_local_mail_input();
    if let Some(oauth) = app.agent_mail.oauth.as_ref() {
        return if oauth.form.request == change {
            Ok(oauth.public())
        } else {
            Err("Finish or cancel the open local input form first".into())
        };
    }
    if let Some(form) = app.agent_mail.active.as_ref() {
        return if form.request == change {
            Ok(form.public())
        } else {
            Err("Finish or cancel the open local input form first".into())
        };
    }
    if app.mail.testing.is_some() || !app.mail.form.password.is_empty() {
        return Err("Mail settings contain an unfinished edit or test; finish it first".into());
    }
    mail::load_accounts_from(&directory.join("mail-accounts.json"))?;
    let mut form = Form::new(&app.mail.accounts, change)?;
    let missing = form.missing_metadata();
    if !missing.is_empty() {
        return Ok(
            json!({"status":"needs_metadata","operation":"mail.configure","required_fields":missing,"secret_values_returned":false}),
        );
    }
    let account = form.account()?;
    if account.provider == Provider::Outlook
        && (account.enabled || form.request.replace_secret == Some(true))
    {
        let client_id = app.settings.outlook_client_id().to_ascii_lowercase();
        if !crate::outlook::valid_client_id(&client_id) {
            return Ok(
                json!({"status":"needs_metadata","operation":"mail.configure","required_fields":[{"key":"outlook_client_id","label":"Microsoft Client ID","type":"string","sensitive":false,"configure_with":"settings set outlook_client_id CLIENT_ID"}],"secret_values_returned":false}),
            );
        }
        if form.request.replace_secret == Some(true)
            || !crate::outlook::has_credentials(&account, &client_id)?
        {
            crate::agent_input::begin(&app.clipboard)?;
            let (job, url) = match crate::outlook::Job::start(account, client_id) {
                Ok(value) => value,
                Err(error) => {
                    crate::agent_input::finish(&app.clipboard, "");
                    return Err(error);
                }
            };
            let pending = OAuthPending {
                form,
                job,
                url,
                requires_cli,
            };
            let result = pending.public();
            app.pending.push(Cmd::OpenUrl(pending.url.clone()));
            app.agent_mail.oauth = Some(pending);
            return Ok(result);
        }
    }
    if account.provider != Provider::Outlook {
        form.refresh(&NativeVault)?;
    }
    if let Ok(account) = form.account() {
        if !form.needs_password {
            let result = commit(
                &mut app.mail.accounts,
                &account,
                form.expected.as_ref(),
                None,
                &NativeVault,
                &directory.join("mail-accounts.json"),
            )?;
            app.mail.status.remove(&mail::key(&account));
            app.pending.push(Cmd::MailReconnect(account.email));
            return Ok(json!({"status":"saved","result":result,"secret_values_returned":false}));
        }
    }
    let result = form.public();
    crate::agent_input::begin(&app.clipboard)?;
    app.agent_mail.active = Some(form);
    Ok(result)
}

pub(crate) fn input_status(app: &mut App, id: &str) -> Result<Value, String> {
    app.expire_local_mail_input();
    app.agent_mail.status(id)
}
pub(crate) fn cancel(app: &mut App, id: &str) -> Result<Value, String> {
    app.expire_local_mail_input();
    if app
        .agent_mail
        .oauth
        .as_ref()
        .is_some_and(|o| o.form.id == id)
    {
        let oauth = app.agent_mail.oauth.take().unwrap();
        return Ok(app.finish_outlook_input(oauth, "cancelled", None));
    }
    if app.agent_mail.active.as_ref().is_some_and(|f| f.id == id) {
        let form = app.agent_mail.active.take().unwrap();
        return Ok(app.finish_local_mail_input(form, "cancelled", None));
    }
    app.agent_mail.status(id)
}
pub(crate) fn remove(app: &mut App, email: &str, directory: &Path) -> Result<Value, String> {
    if app.agent_mail.is_open() || app.mail.testing.is_some() || !app.mail.form.password.is_empty()
    {
        return Err("Finish or cancel the open mail edit first".into());
    }
    mail::load_accounts_from(&directory.join("mail-accounts.json"))?;
    let account = existing(&app.mail.accounts, email)?.ok_or("Mailbox not found")?;
    let next: Vec<_> = app
        .mail
        .accounts
        .iter()
        .filter(|a| mail::key(a) != mail::key(&account))
        .cloned()
        .collect();
    mail::save_accounts_to(&next, &directory.join("mail-accounts.json"))?;
    app.mail.accounts = next;
    app.mail.status.remove(&mail::key(&account));
    app.mail
        .recent
        .retain(|m| !m.account.eq_ignore_ascii_case(email));
    app.pending.push(Cmd::MailSync);
    Ok(
        json!({"removed":true,"email":account.email,"credential_removed":NativeVault.remove(&account.credential_key()).is_ok()}),
    )
}

impl App {
    fn outlook_authorization_ui(&mut self, ctx: &egui::Context) {
        let Some(oauth) = self.agent_mail.oauth.take() else {
            return;
        };
        if oauth.requires_cli && !self.settings.cli_enabled {
            self.finish_outlook_input(oauth, "cancelled", None);
            return;
        }
        if let Some(result) = oauth.job.poll() {
            let result = result.and_then(|tokens| {
                let account = oauth.form.account()?;
                if !tokens
                    .client_id
                    .eq_ignore_ascii_case(self.settings.outlook_client_id())
                {
                    return Err("Microsoft 应用已变更，请重新登录 Outlook".into());
                }
                if self.mail.testing.is_some() || !self.mail.form.password.is_empty() {
                    return Err("Mail settings are being edited; finish that edit first".into());
                }
                mail::load_accounts_from(&model::config_dir().join("mail-accounts.json"))?;
                let credential = tokens.credential()?;
                let mut result = commit(
                    &mut self.mail.accounts,
                    &account,
                    oauth.form.expected.as_ref(),
                    Some(&credential),
                    &NativeVault,
                    &model::config_dir().join("mail-accounts.json"),
                )?;
                result["connection_status"] = "verified".into();
                self.mail.status.remove(&mail::key(&account));
                self.pending.push(Cmd::MailReconnect(account.email));
                Ok(result)
            });
            match result {
                Ok(result) => {
                    self.mail.note = "Outlook 已授权，正在连接…".into();
                    self.finish_outlook_input(oauth, "saved", Some(result));
                }
                Err(error) if error == "已取消微软授权" => {
                    self.mail.note = error;
                    self.finish_outlook_input(oauth, "cancelled", None);
                }
                Err(error) => {
                    self.mail.note = error.clone();
                    self.finish_outlook_input(oauth, "failed", Some(json!({"error":error})));
                }
            }
            ctx.request_repaint();
            return;
        }
        let locale = self.settings.locale();
        let dark = self
            .settings
            .settings_theme
            .dark(crate::settings_theme::system_dark(ctx));
        let mut cancel = false;
        let mut open = false;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("islo-local-input"),
            egui::ViewportBuilder::default()
                .with_title(locale.text("Outlook 网页授权"))
                .with_icon(crate::theme::app_icon())
                .with_inner_size([420.0, 230.0])
                .with_resizable(false),
            |ctx, _| {
                cancel = ctx
                    .input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape));
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::central_panel(&ctx.style())
                            .inner_margin(20.0)
                            .fill(if dark {
                                egui::Color32::from_rgb(25, 25, 28)
                            } else {
                                egui::Color32::from_rgb(248, 249, 251)
                            }),
                    )
                    .show(ctx, |ui| {
                        ui.style_mut().visuals = if dark {
                            egui::Visuals::dark()
                        } else {
                            egui::Visuals::light()
                        };
                        ui.label(
                            egui::RichText::new(locale.text("登录 Outlook"))
                                .size(20.0)
                                .strong()
                                .color(ui.visuals().strong_text_color()),
                        );
                        ui.label(&oauth.form.email);
                        ui.add_space(8.0);
                        ui.label(
                            locale.text("请在微软官方页面登录并授权。islo 不收集你的微软密码。"),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            open = ui
                                .add_sized(
                                    [160.0, 32.0],
                                    egui::Button::new(locale.text("在浏览器中继续")),
                                )
                                .clicked();
                            if ui
                                .add_sized([90.0, 32.0], egui::Button::new(locale.text("取消")))
                                .clicked()
                            {
                                cancel = true;
                            }
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(locale.text("等待授权完成…"));
                        });
                    });
                ctx.request_repaint_after(Duration::from_millis(500));
            },
        );
        if cancel {
            self.finish_outlook_input(oauth, "cancelled", None);
            ctx.request_repaint();
            return;
        }
        if open {
            self.pending.push(Cmd::OpenUrl(oauth.url.clone()));
        }
        self.agent_mail.oauth = Some(oauth);
    }

    fn finish_outlook_input(
        &mut self,
        oauth: OAuthPending,
        status: &str,
        result: Option<Value>,
    ) -> Value {
        self.pending
            .retain(|cmd| !matches!(cmd,Cmd::OpenUrl(url) if url==&oauth.url));
        self.finish_local_mail_input(oauth.form, status, result)
    }

    fn finish_local_mail_input(
        &mut self,
        form: Form,
        status: &str,
        result: Option<Value>,
    ) -> Value {
        crate::agent_input::finish(&self.clipboard, &form.password);
        self.agent_mail.finish(form, status, result)
    }
    fn expire_local_mail_input(&mut self) {
        if self
            .agent_mail
            .oauth
            .as_ref()
            .is_some_and(|o| o.form.created.elapsed() > Duration::from_secs(900))
        {
            let oauth = self.agent_mail.oauth.take().unwrap();
            self.finish_outlook_input(oauth, "expired", None);
        }
        if self
            .agent_mail
            .active
            .as_ref()
            .is_some_and(|f| f.created.elapsed() > Duration::from_secs(900))
        {
            let form = self.agent_mail.active.take().unwrap();
            self.finish_local_mail_input(form, "expired", None);
        }
    }
    pub fn agent_mail_input_ui(&mut self, ctx: &egui::Context) {
        self.expire_local_mail_input();
        self.outlook_authorization_ui(ctx);
        let Some(mut form) = self.agent_mail.active.take() else {
            return;
        };
        if !self.settings.cli_enabled {
            self.finish_local_mail_input(form, "cancelled", None);
            return;
        }
        let locale = self.settings.locale();
        let first = std::mem::take(&mut form.focus);
        let dark = self
            .settings
            .settings_theme
            .dark(crate::settings_theme::system_dark(ctx));
        let destination = form
            .account()
            .ok()
            .and_then(|a| a.endpoint().ok())
            .map(|(host, port, _)| format!("{host}:{port} \u{00b7} TLS"))
            .unwrap_or_default();
        let label = locale.text(form.password_label());
        let action = crate::agent_input::show(
            ctx,
            locale,
            dark,
            crate::agent_input::SecretPrompt {
                label,
                subject: &form.email,
                destination: &destination,
                value: &mut form.password,
                error: &form.error,
                first,
            },
        );
        let cancel = action == Some(crate::agent_input::Action::Cancel);
        let save = action == Some(crate::agent_input::Action::Confirm);
        if cancel {
            self.finish_local_mail_input(form, "cancelled", None);
            ctx.request_repaint();
            return;
        }
        if save {
            let result = (|| {
                let account = form.account()?;
                mail::load_accounts_from(&model::config_dir().join("mail-accounts.json"))?;
                if self.mail.testing.is_some() || !self.mail.form.password.is_empty() {
                    return Err("Mail settings are being edited; finish that edit first".into());
                }
                let password = form.needs_password.then_some(form.password.as_str());
                let result = commit(
                    &mut self.mail.accounts,
                    &account,
                    form.expected.as_ref(),
                    password,
                    &NativeVault,
                    &model::config_dir().join("mail-accounts.json"),
                )?;
                self.mail.status.remove(&mail::key(&account));
                self.pending.push(Cmd::MailReconnect(account.email));
                Ok(result)
            })();
            match result {
                Ok(result) => {
                    self.finish_local_mail_input(form, "saved", Some(result));
                    ctx.request_repaint();
                    return;
                }
                Err(error) => form.error = error,
            }
        }
        self.agent_mail.active = Some(form);
    }
}

#[cfg(test)]
pub(crate) fn enter_test_password(app: &mut App, value: &str) {
    app.agent_mail.active.as_mut().unwrap().password = value.into();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashMap};
    #[derive(Default)]
    struct MemoryVault(RefCell<HashMap<String, String>>);
    impl Vault for MemoryVault {
        fn get(&self, key: &str) -> Result<Option<String>, String> {
            Ok(self.0.borrow().get(key).cloned())
        }
        fn set(&self, key: &str, value: &str) -> Result<(), String> {
            self.0.borrow_mut().insert(key.into(), value.into());
            Ok(())
        }
        fn remove(&self, key: &str) -> Result<(), String> {
            self.0.borrow_mut().remove(key);
            Ok(())
        }
    }
    fn account() -> Account {
        Account {
            email: "sample@example.test".into(),
            provider: Provider::Qq,
            enabled: true,
            custom: None,
        }
    }

    #[test]
    fn agent_mail_forms_request_only_needed_fields_and_never_return_the_password() {
        let vault = MemoryVault::default();
        let mut form = Form::new(&[], Change::default()).unwrap();
        form.refresh(&vault).unwrap();
        assert!(form.missing_metadata().iter().any(|f| f["key"] == "email"));
        assert!(
            !form.needs_password,
            "Do not request a password before the destination is known"
        );
        form.email = "sample@example.test".into();
        form.provider = Some(Provider::Qq);
        form.refresh(&vault).unwrap();
        assert!(form.needs_password);
        assert!(form.public()["required_fields"]
            .as_array()
            .unwrap()
            .iter()
            .all(|field| field["sensitive"] == true));
        form.password = "SENSITIVE_TEST_VALUE".into();
        assert!(!form.public().to_string().contains("SENSITIVE_TEST_VALUE"));
        let mut state = State::default();
        assert!(!state
            .finish(form, "cancelled", None)
            .to_string()
            .contains("SENSITIVE_TEST_VALUE"));
        let old = account();
        vault
            .set(&old.credential_key(), "SENSITIVE_TEST_VALUE")
            .unwrap();
        let mut existing = Form::new(
            &[old.clone()],
            Change {
                email: Some(old.email.clone()),
                enabled: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        existing.refresh(&vault).unwrap();
        assert!(
            !existing.needs_password,
            "An ordinary settings change must reuse credentials without prompting"
        );
        assert!(
            serde_json::from_value::<crate::agent_commands::Command>(
                json!({"op":"mail.configure","email":old.email,"password":"SENSITIVE_TEST_VALUE"})
            )
            .is_err(),
            "Sensitive values must be rejected by the command protocol"
        );
    }

    #[test]
    fn agent_mail_failed_save_rolls_back_credentials_and_preserves_live_configuration() {
        let vault = MemoryVault::default();
        let old = account();
        vault.set(&old.credential_key(), "old-value").unwrap();
        let mut accounts = vec![old.clone()];
        let root = std::env::temp_dir().join(format!(
            "islo-mail-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        assert!(commit(
            &mut accounts,
            &old,
            Some(&old),
            Some("new-value"),
            &vault,
            &root
        )
        .is_err());
        assert_eq!(
            vault.get(&old.credential_key()).unwrap().as_deref(),
            Some("old-value")
        );
        assert_eq!(accounts, [old.clone()]);
        let file = root.join("accounts.json");
        let result = commit(
            &mut accounts,
            &old,
            Some(&old),
            Some("new-value"),
            &vault,
            &file,
        )
        .unwrap();
        assert!(!result.to_string().contains("new-value"));
        assert!(!std::fs::read_to_string(&file)
            .unwrap()
            .contains("new-value"));
        let mut modified = old.clone();
        modified.enabled = false;
        accounts[0] = modified;
        assert!(
            commit(
                &mut accounts,
                &old,
                Some(&old),
                Some("other-value"),
                &vault,
                &file
            )
            .is_err(),
            "An input form must not overwrite an account edited while it was open"
        );
        assert_eq!(
            vault.get(&old.credential_key()).unwrap().as_deref(),
            Some("new-value")
        );
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn agent_mail_destination_changes_require_new_credentials() {
        let vault = MemoryVault::default();
        let old = Account {
            provider: Provider::Custom,
            custom: Some(CustomImap {
                host: "old.example.test".into(),
                port: 993,
                username: "user".into(),
            }),
            ..account()
        };
        vault.set(&old.credential_key(), "old-value").unwrap();
        let mut form = Form::new(
            &[old.clone()],
            Change {
                email: Some(old.email.clone()),
                host: Some("new.example.test".into()),
                ..Default::default()
            },
        )
        .unwrap();
        form.refresh(&vault).unwrap();
        assert!(form.needs_password);
        assert_ne!(
            form.account().unwrap().credential_key(),
            old.credential_key()
        );
        assert!(Form::new(
            &[],
            Change {
                provider: Some("not-a-provider".into()),
                ..Default::default()
            }
        )
        .is_err());
    }
}
