use super::*;
use fs2::FileExt;
use serde::de::DeserializeOwned;
use std::{collections::HashMap, fmt, fs::File, path::Path, sync::MutexGuard};
use zeroize::Zeroizing;

pub const APP_IDENTIFIER: &str = "com.app.key-switch";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StartupError {
    AlreadyRunning,
    Unavailable,
}

/// The file is never removed, truncated or replaced. Only the OS lock is authoritative.
pub struct InstanceLock {
    file: File,
    directory: PathBuf,
}
impl InstanceLock {
    pub fn acquire(directory: &Path) -> Result<Self, StartupError> {
        fs::create_dir_all(directory).map_err(|_| StartupError::Unavailable)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("key-switch.lock"))
            .map_err(|_| StartupError::Unavailable)?;
        file.try_lock_exclusive().map_err(|e| {
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.raw_os_error() == fs2::lock_contended_error().raw_os_error()
            {
                StartupError::AlreadyRunning
            } else {
                StartupError::Unavailable
            }
        })?;
        Ok(Self {
            file,
            directory: directory.to_owned(),
        })
    }
}
impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

/// Uses the same dirs::data_dir + bundle identifier rule as Tauri app_data_dir.
pub fn application_directory() -> Result<PathBuf, StartupError> {
    dirs::data_dir()
        .map(|p| p.join(APP_IDENTIFIER))
        .ok_or(StartupError::Unavailable)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Locale {
    ZhCn,
    ZhTw,
    En,
    Ja,
}
pub fn resolve_locale(value: Option<&str>) -> Locale {
    let Some(value) = value else {
        return Locale::ZhCn;
    };
    let value = value.replace('_', "-").to_ascii_lowercase();
    if value.starts_with("zh") {
        if value.contains("tw")
            || value.contains("hk")
            || value.contains("hant")
            || value.contains("mo")
        {
            Locale::ZhTw
        } else {
            Locale::ZhCn
        }
    } else if value.starts_with("ja") {
        Locale::Ja
    } else {
        Locale::En
    }
}
pub fn system_locale() -> Locale {
    resolve_locale(sys_locale::get_locale().as_deref())
}
pub fn startup_message(locale: Locale, error: StartupError) -> &'static str {
    match (locale, error) {
        (Locale::ZhCn, StartupError::AlreadyRunning) => {
            "Key Switch 已在运行。请先关闭桌面端或 TUI。"
        }
        (Locale::ZhTw, StartupError::AlreadyRunning) => {
            "Key Switch 已在執行。請先關閉桌面端或 TUI。"
        }
        (Locale::En, StartupError::AlreadyRunning) => {
            "Key Switch is already running. Close the desktop app or TUI first."
        }
        (Locale::Ja, StartupError::AlreadyRunning) => {
            "Key Switch は実行中です。デスクトップアプリまたは TUI を終了してください。"
        }
        (Locale::ZhCn, _) => "无法锁定应用数据目录，启动失败。",
        (Locale::ZhTw, _) => "無法鎖定應用資料目錄，啟動失敗。",
        (Locale::En, _) => "Cannot lock the application data directory. Startup failed.",
        (Locale::Ja, _) => "データディレクトリをロックできません。起動に失敗しました。",
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code)
    }
}
impl std::error::Error for CommandError {}
pub trait Credentials: Send + Sync {
    fn get(&self, id: &str) -> Result<String, CommandError>;
    fn set(&self, id: &str, value: &str) -> Result<(), CommandError>;
    /// Missing entries count as deleted, so recovery is idempotent.
    fn delete(&self, id: &str) -> Result<(), CommandError>;
}
pub struct SystemCredentials;
impl Credentials for SystemCredentials {
    fn get(&self, id: &str) -> Result<String, CommandError> {
        keyring::Entry::new(KEYRING_SERVICE, id)
            .map_err(|_| CommandError::from("系统密钥库不可用"))?
            .get_password()
            .map_err(|_| CommandError::from("无法读取系统密钥库"))
    }
    fn set(&self, id: &str, value: &str) -> Result<(), CommandError> {
        keyring::Entry::new(KEYRING_SERVICE, id)
            .map_err(|_| CommandError::from("系统密钥库不可用"))?
            .set_password(value)
            .map_err(|_| CommandError::from("无法写入系统密钥库"))
    }
    fn delete(&self, id: &str) -> Result<(), CommandError> {
        match keyring::Entry::new(KEYRING_SERVICE, id)
            .map_err(|_| CommandError::from("系统密钥库不可用"))?
            .delete_credential()
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            _ => Err("无法从系统密钥库删除".into()),
        }
    }
}
pub trait Clipboard {
    fn write(&self, value: &str) -> Result<(), CommandError>;
}

pub struct Core {
    pub(super) directory: PathBuf,
    pub(super) credentials: Arc<dyn Credentials>,
    _instance: InstanceLock,
    business: Mutex<()>,
    generations: Mutex<HashMap<String, u64>>,
}
impl Core {
    pub fn new(
        lock: InstanceLock,
        credentials: Arc<dyn Credentials>,
    ) -> Result<Self, CommandError> {
        let core = Self {
            directory: lock.directory.clone(),
            credentials,
            _instance: lock,
            business: Mutex::new(()),
            generations: Mutex::new(HashMap::new()),
        };
        let _guard = core.transaction()?;
        load_data(&core)?;
        drop(_guard);
        Ok(core)
    }
    pub(super) fn transaction(&self) -> Result<MutexGuard<'_, ()>, CommandError> {
        let guard = self
            .business
            .lock()
            .map_err(|_| CommandError::from("TRANSACTION_UNAVAILABLE"))?;
        self.recover_credentials()?;
        Ok(guard)
    }
    fn bump(&self, id: &str) -> Result<u64, CommandError> {
        let mut versions = self
            .generations
            .lock()
            .map_err(|_| CommandError::from("TRANSACTION_UNAVAILABLE"))?;
        let value = versions.entry(id.to_owned()).or_default();
        *value += 1;
        Ok(*value)
    }
    pub(super) fn invalidate_provider(&self, id: &str) -> Result<(), CommandError> {
        let data = load_data(self)?;
        if let Some(p) = data.providers.iter().find(|p| p.id == id) {
            for k in &p.keys {
                self.bump(&k.id)?;
            }
        }
        Ok(())
    }
    fn journal_path(&self) -> PathBuf {
        self.directory.join("key-switch-transaction.json")
    }
    fn clear_journal(&self) -> Result<(), CommandError> {
        // Remove backup first: interruption must never resurrect an obsolete journal.
        for p in [
            self.journal_path().with_extension("json.bak"),
            self.journal_path().with_extension("json.tmp"),
            self.journal_path(),
        ] {
            if p.exists() {
                fs::remove_file(p).map_err(|_| CommandError::from("RECOVERY_FAILED"))?;
            }
        }
        sync_directory(&self.directory).map_err(|_| CommandError::from("RECOVERY_FAILED"))
    }
    fn cleanup(&self, journal: &CredentialJournal, committed: bool) -> Result<(), CommandError> {
        if committed {
            for (id, _) in &journal.retired {
                self.credentials.delete(id)?;
            }
        } else {
            if journal.ready {
                for (id, backup) in &journal.retired {
                    let value = Zeroizing::new(self.credentials.get(backup)?);
                    self.credentials.set(id, &value)?;
                }
            }
            for id in &journal.created {
                self.credentials.delete(id)?;
            }
        }
        // Mark cleanup before deleting backups; retries no longer need their values.
        let mut cleaned = journal.clone();
        cleaned.cleaned = true;
        atomic_save(&self.journal_path(), &cleaned)
            .map_err(|_| CommandError::from("RECOVERY_FAILED"))?;
        for (_, backup) in &journal.retired {
            self.credentials.delete(backup)?;
        }
        self.clear_journal()
    }
    fn recover_credentials(&self) -> Result<(), CommandError> {
        let path = self.journal_path();
        recover_json::<CredentialJournal>(&path)
            .map_err(|_| CommandError::from("RECOVERY_FAILED"))?;
        if !path.exists() {
            return Ok(());
        }
        let journal: CredentialJournal = serde_json::from_slice(
            &fs::read(&path).map_err(|_| CommandError::from("RECOVERY_FAILED"))?,
        )
        .map_err(|_| CommandError::from("RECOVERY_FAILED"))?;
        if journal.cleaned {
            for (_, backup) in &journal.retired {
                self.credentials
                    .delete(backup)
                    .map_err(|_| CommandError::from("RECOVERY_FAILED"))?;
            }
            return self.clear_journal();
        }
        let current = load_data(self)?;
        let committed = journal.ready && current == journal.after;
        if !committed && current != journal.before {
            return Err("RECOVERY_FAILED".into());
        }
        self.cleanup(&journal, committed)
            .map_err(|_| CommandError::from("RECOVERY_FAILED"))
    }
    fn commit(
        &self,
        before: AppData,
        after: AppData,
        writes: Vec<(String, Zeroizing<String>)>,
        retired: Vec<String>,
    ) -> Result<(), CommandError> {
        let mut journal = CredentialJournal {
            before,
            after,
            created: writes.iter().map(|(id, _)| id.clone()).collect(),
            retired: retired
                .into_iter()
                .map(|id| (id, format!("recovery-{}", uuid::Uuid::new_v4())))
                .collect(),
            ready: false,
            cleaned: false,
        };
        atomic_save(&self.journal_path(), &journal)
            .map_err(|_| CommandError::from("无法保存本地数据"))?;
        let result = (|| {
            for (id, backup) in &journal.retired {
                let value = Zeroizing::new(self.credentials.get(id)?);
                self.credentials.set(backup, &value)?;
            }
            journal.ready = true;
            atomic_save(&self.journal_path(), &journal)
                .map_err(|_| CommandError::from("无法保存本地数据"))?;
            for (id, value) in &writes {
                self.credentials.set(id, value)?;
            }
            save_data(self, &journal.after)?;
            for (id, _) in &journal.retired {
                self.credentials.delete(id)?;
            }
            Ok::<_, CommandError>(())
        })();
        if let Err(error) = result {
            let metadata_unchanged = load_data(self).is_ok_and(|current| current == journal.before);
            if (!metadata_unchanged && save_data(self, &journal.before).is_err())
                || self.cleanup(&journal, false).is_err()
            {
                return Err("ROLLBACK_FAILED".into());
            }
            return Err(error);
        }
        self.cleanup(&journal, true)
            .map_err(|_| CommandError::from("RECOVERY_FAILED"))
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct CredentialJournal {
    before: AppData,
    after: AppData,
    created: Vec<String>,
    retired: Vec<(String, String)>,
    ready: bool,
    cleaned: bool,
}

fn sync_directory(directory: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        File::open(directory)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = directory;
    }
    Ok(())
}
pub(super) fn atomic_save<T: Serialize>(file: &Path, value: &T) -> std::io::Result<()> {
    let directory = file.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
    fs::create_dir_all(directory)?;
    let tmp = file.with_extension("json.tmp");
    let bak = file.with_extension("json.bak");
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut output = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&tmp)?;
    output.write_all(&bytes)?;
    output.sync_all()?;
    drop(output);
    if bak.exists() {
        fs::remove_file(&bak)?;
    }
    let existing = file.exists();
    if existing {
        fs::rename(file, &bak)?;
        sync_directory(directory)?;
    }
    if let Err(error) = fs::rename(&tmp, file) {
        if existing {
            fs::rename(&bak, file)?;
        }
        return Err(error);
    }
    // Once rename succeeds, the operation has committed. Keep the backup until next save.
    // A directory-sync failure is durability uncertainty, not grounds for credential rollback.
    let _ = sync_directory(directory);
    Ok(())
}
pub(super) fn recover_json<T: DeserializeOwned>(file: &Path) -> std::io::Result<()> {
    recover_json_validated::<T>(file, |_| true)
}
pub(super) fn recover_json_validated<T: DeserializeOwned>(
    file: &Path,
    validate: impl Fn(&T) -> bool,
) -> std::io::Result<()> {
    let bak = file.with_extension("json.bak");
    let tmp = file.with_extension("json.tmp");
    let valid = |p: &Path| {
        fs::read(p)
            .ok()
            .and_then(|b| serde_json::from_slice::<T>(&b).ok())
            .is_some_and(|value| validate(&value))
    };
    if file.exists() {
        if !valid(file) {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        return Ok(());
    }
    // No destination means replacement was interrupted. Only accept a complete candidate.
    let candidate = if valid(&tmp) {
        Some(&tmp)
    } else if valid(&bak) {
        Some(&bak)
    } else {
        None
    };
    if let Some(candidate) = candidate {
        fs::rename(candidate, file)?;
        sync_directory(file.parent().ok_or(std::io::ErrorKind::InvalidInput)?)?;
    } else if bak.exists() || tmp.exists() {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    Ok(())
}

pub(super) fn validate_provider_input(
    url: &Option<String>,
    logo: &Option<String>,
) -> Result<(), CommandError> {
    if let Some(url) = url.as_ref().filter(|u| !u.trim().is_empty()) {
        let parsed = reqwest::Url::parse(url.trim())
            .map_err(|_| CommandError::from("PROVIDER_INPUT_INVALID"))?;
        if !matches!(parsed.scheme(), "https" | "http")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err("PROVIDER_INPUT_INVALID".into());
        }
    }
    if let Some(logo) = logo {
        if logo.len() > 3 * 1024 * 1024
            || (logo.starts_with("data:")
                && ![
                    "data:image/png;base64,",
                    "data:image/jpeg;base64,",
                    "data:image/webp;base64,",
                    "data:image/svg+xml;base64,",
                ]
                .iter()
                .any(|prefix| logo.starts_with(prefix)))
        {
            return Err("PROVIDER_INPUT_INVALID".into());
        }
    }
    Ok(())
}

pub(super) fn validate_custom_logo(logo: Option<&str>) -> Result<(), CommandError> {
    use base64::Engine;
    let Some(logo) = logo else {
        return Ok(());
    };
    let Some((prefix, encoded)) = logo.split_once(',') else {
        return Err("PROVIDER_INPUT_INVALID".into());
    };
    if !matches!(
        prefix,
        "data:image/png;base64" | "data:image/jpeg;base64" | "data:image/webp;base64"
    ) || encoded.len() > 2 * 1024 * 1024 * 4 / 3 + 4
    {
        return Err("PROVIDER_INPUT_INVALID".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| CommandError::from("PROVIDER_INPUT_INVALID"))?;
    let valid = match prefix {
        "data:image/png;base64" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "data:image/jpeg;base64" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "data:image/webp;base64" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        _ => false,
    };
    if !valid || bytes.len() > 2 * 1024 * 1024 {
        return Err("PROVIDER_INPUT_INVALID".into());
    }
    Ok(())
}

pub fn create_api_key(app: &Core, input: CreateKeyInput) -> Result<ApiKeySummary, CommandError> {
    let value = Zeroizing::new(input.value);
    if value.trim().is_empty() {
        return Err("API Key 不能为空".into());
    }
    let _guard = app.transaction()?;
    let before = load_data(app)?;
    let mut after = before.clone();
    let provider = after
        .providers
        .iter_mut()
        .find(|p| p.id == input.provider_id)
        .ok_or("未找到供应商")?;
    let id = format!("key-{}", uuid::Uuid::new_v4());
    let key = ApiKeyRecord {
        id: id.clone(),
        secret_id: id.clone(),
        provider_id: input.provider_id,
        remark: input.remark.trim().into(),
        status: if key_validation_spec(provider).is_some() {
            "untested"
        } else {
            "unsupported"
        }
        .into(),
        check_error_code: None,
    };
    let result = ApiKeySummary {
        id: key.id.clone(),
        provider_id: key.provider_id.clone(),
        remark: key.remark.clone(),
        masked_value: mask(value.trim()),
        status: key.status.clone(),
        check_error_code: None,
    };
    provider.keys.push(key);
    app.commit(
        before,
        after,
        vec![(id, Zeroizing::new(value.trim().into()))],
        vec![],
    )?;
    Ok(result)
}
pub fn update_api_key(app: &Core, input: UpdateKeyInput) -> Result<ApiKeySummary, CommandError> {
    let value = Zeroizing::new(input.value);
    let _guard = app.transaction()?;
    let before = load_data(app)?;
    let mut after = before.clone();
    let key = after
        .providers
        .iter_mut()
        .flat_map(|p| &mut p.keys)
        .find(|k| k.id == input.id)
        .ok_or("未找到 API Key")?;
    key.remark = input.remark.trim().into();
    if value.trim().is_empty() {
        let result = key_summary(app, key)?;
        save_data(app, &after)?;
        return Ok(result);
    }
    let retired = key.secret_id.clone();
    key.secret_id = format!("key-{}", uuid::Uuid::new_v4());
    let next = key.secret_id.clone();
    key.status = "untested".into();
    key.check_error_code = None;
    let result = ApiKeySummary {
        id: key.id.clone(),
        provider_id: key.provider_id.clone(),
        remark: key.remark.clone(),
        masked_value: mask(value.trim()),
        status: key.status.clone(),
        check_error_code: None,
    };
    app.bump(&input.id)?;
    app.commit(
        before,
        after,
        vec![(next, Zeroizing::new(value.trim().into()))],
        vec![retired],
    )?;
    Ok(result)
}
pub fn delete_api_key(app: &Core, key_id: String) -> Result<(), CommandError> {
    let _guard = app.transaction()?;
    let before = load_data(app)?;
    let mut after = before.clone();
    let provider = after
        .providers
        .iter_mut()
        .find(|p| p.keys.iter().any(|k| k.id == key_id))
        .ok_or("未找到 API Key")?;
    let index = provider
        .keys
        .iter()
        .position(|k| k.id == key_id)
        .ok_or("未找到 API Key")?;
    let key = provider.keys.remove(index);
    app.bump(&key_id)?;
    app.commit(before, after, vec![], vec![key.secret_id])
}
pub fn delete_provider(app: &Core, provider_id: String) -> Result<(), CommandError> {
    let _guard = app.transaction()?;
    let before = load_data(app)?;
    let mut after = before.clone();
    let index = after
        .providers
        .iter()
        .position(|p| p.id == provider_id)
        .ok_or("未找到供应商")?;
    let provider = after.providers.remove(index);
    for key in &provider.keys {
        app.bump(&key.id)?;
    }
    app.commit(
        before,
        after,
        vec![],
        provider.keys.into_iter().map(|k| k.secret_id).collect(),
    )
}

pub(super) struct CheckSnapshot {
    provider: ProviderRecord,
    key: ApiKeyRecord,
    generation: u64,
    value: Zeroizing<String>,
}
pub(super) fn snapshots(
    app: &Core,
    provider_id: &str,
    key_id: Option<&str>,
) -> Result<Vec<CheckSnapshot>, CommandError> {
    let _guard = app.transaction()?;
    let data = load_data(app)?;
    let provider = data
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or("未找到供应商")?;
    if key_id.is_some_and(|id| !provider.keys.iter().any(|k| k.id == id)) {
        return Err("未找到 API Key".into());
    }
    provider
        .keys
        .iter()
        .filter(|k| key_id.is_none_or(|id| k.id == id))
        .map(|key| {
            Ok(CheckSnapshot {
                provider: provider.clone(),
                key: key.clone(),
                generation: app.bump(&key.id)?,
                value: key_value(app, key)?,
            })
        })
        .collect()
}
pub(super) fn merge_outcomes(
    app: &Core,
    snapshots: Vec<(CheckSnapshot, ValidationOutcome)>,
) -> Result<(), CommandError> {
    let _guard = app.transaction()?;
    let mut data = load_data(app)?;
    let generations = app
        .generations
        .lock()
        .map_err(|_| CommandError::from("TRANSACTION_UNAVAILABLE"))?;
    for (snapshot, outcome) in snapshots {
        let Some(provider) = data
            .providers
            .iter_mut()
            .find(|p| p.id == snapshot.provider.id)
        else {
            continue;
        };
        if provider.validation != snapshot.provider.validation
            || provider.kind != snapshot.provider.kind
        {
            continue;
        }
        let Some(key) = provider
            .keys
            .iter_mut()
            .find(|k| k.id == snapshot.key.id && k.secret_id == snapshot.key.secret_id)
        else {
            continue;
        };
        if generations.get(&key.id) != Some(&snapshot.generation) {
            continue;
        }
        key.status = outcome.status.into();
        key.check_error_code = outcome.error_code.map(str::to_owned);
    }
    save_data(app, &data)?;
    Ok(())
}
async fn perform_checks(app: &Core, snapshots: Vec<CheckSnapshot>) -> Result<(), CommandError> {
    let mut outcomes = Vec::new();
    for snapshot in snapshots {
        let outcome = if let Some(spec) = key_validation_spec(&snapshot.provider) {
            match resolve_validation_endpoint(&spec).await {
                Ok(endpoint) => {
                    validate_key(&validation_client(&endpoint)?, &spec, &snapshot.value).await
                }
                Err(code) => ValidationOutcome {
                    status: "error",
                    error_code: Some(code),
                },
            }
        } else {
            ValidationOutcome {
                status: "unsupported",
                error_code: None,
            }
        };
        outcomes.push((snapshot, outcome));
    }
    merge_outcomes(app, outcomes)
}
pub async fn check_provider_keys(
    app: &Core,
    provider_id: String,
) -> Result<Vec<ApiKeySummary>, CommandError> {
    perform_checks(app, snapshots(app, &provider_id, None)?).await?;
    let _guard = app.transaction()?;
    let data = load_data(app)?;
    data.providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or("未找到供应商")?
        .keys
        .iter()
        .map(|k| key_summary(app, k).map_err(CommandError::from))
        .collect()
}
pub async fn check_api_key(
    app: &Core,
    provider_id: String,
    key_id: String,
) -> Result<ApiKeySummary, CommandError> {
    perform_checks(app, snapshots(app, &provider_id, Some(&key_id))?).await?;
    let _guard = app.transaction()?;
    let data = load_data(app)?;
    let key = data
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or("未找到供应商")?
        .keys
        .iter()
        .find(|k| k.id == key_id)
        .ok_or("未找到 API Key")?;
    key_summary(app, key).map_err(CommandError::from)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProvider {
    pub id: String,
    pub name_zh: String,
    pub name_en: String,
    pub abbreviation: String,
    pub tone: String,
    pub platform_url: String,
}
pub fn provider_catalog() -> Vec<CatalogProvider> {
    serde_json::from_str(include_str!("../provider-catalog.json"))
        .expect("validated built-in catalog")
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn localized_error(locale: Locale, code: &str) -> String {
    let resources: serde_json::Value =
        serde_json::from_str(include_str!("../errors.json")).expect("validated translations");
    let language = match locale {
        Locale::ZhCn => "zh-CN",
        Locale::ZhTw => "zh-TW",
        Locale::En => "en-US",
        Locale::Ja => "ja-JP",
    };
    resources[language][code]
        .as_str()
        .or_else(|| resources[language]["UNKNOWN"].as_str())
        .unwrap_or("Key Switch")
        .to_owned()
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use crate::safety_tests::{add_key, fixture};

    #[test]
    fn recovers_credential_transaction_at_each_interruption_stage() {
        for phase in 0..4 {
            let (dir, credentials, core) = fixture();
            let key = add_key(&core);
            let before = load_data(&core).unwrap();
            let mut after = before.clone();
            let old = before.providers[0].keys[0].secret_id.clone();
            after.providers[0].keys[0].secret_id = "new-fixture-id".into();
            let journal = CredentialJournal {
                before: before.clone(),
                after: after.clone(),
                created: vec!["new-fixture-id".into()],
                retired: vec![(old.clone(), "backup-fixture-id".into())],
                ready: phase > 0,
                cleaned: phase == 3,
            };
            atomic_save(&core.journal_path(), &journal).unwrap();
            if phase > 0 {
                credentials
                    .set("backup-fixture-id", "fixture-private-credential")
                    .unwrap();
                credentials
                    .set("new-fixture-id", "replacement-fixture")
                    .unwrap();
            }
            if phase >= 2 {
                save_data(&core, &after).unwrap();
                credentials.delete(&old).unwrap();
            }
            if phase == 3 {
                credentials.delete("backup-fixture-id").unwrap();
            }
            drop(core);
            let recovered = Core::new(
                InstanceLock::acquire(dir.path()).unwrap(),
                credentials.clone(),
            )
            .unwrap();
            let data = load_data(&recovered).unwrap();
            assert_eq!(data.providers[0].keys[0].id, key.id);
            assert!(data == if phase >= 2 { after } else { before });
            assert_eq!(credentials.values.lock().unwrap().len(), 1);
            assert!(!recovered.journal_path().exists());
        }
    }
    #[test]
    fn locked_keyring_keeps_recovery_intent_and_blocks_initialization() {
        struct Locked;
        impl Credentials for Locked {
            fn get(&self, _: &str) -> Result<String, CommandError> {
                Err("系统密钥库不可用".into())
            }
            fn set(&self, _: &str, _: &str) -> Result<(), CommandError> {
                Err("系统密钥库不可用".into())
            }
            fn delete(&self, _: &str) -> Result<(), CommandError> {
                Err("系统密钥库不可用".into())
            }
        }
        let (dir, _, core) = fixture();
        let before = load_data(&core).unwrap();
        let journal = CredentialJournal {
            before: before.clone(),
            after: before,
            created: vec!["temporary-fixture-id".into()],
            retired: vec![],
            ready: false,
            cleaned: false,
        };
        atomic_save(&core.journal_path(), &journal).unwrap();
        drop(core);
        assert!(matches!(
            Core::new(InstanceLock::acquire(dir.path()).unwrap(), Arc::new(Locked)),
            Err(CommandError {
                code: "RECOVERY_FAILED"
            })
        ));
        assert!(dir.path().join("key-switch-transaction.json").exists());
    }
}
