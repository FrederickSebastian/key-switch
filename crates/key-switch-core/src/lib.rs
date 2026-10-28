use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
mod runtime;
pub use runtime::*;
#[cfg(test)]
mod safety_tests;
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
}

impl CommandError {
    pub fn from_message(message: &str) -> Self {
        Self {
            code: command_error_code(message),
        }
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self::from_message(&message)
    }
}

impl From<&str> for CommandError {
    fn from(message: &str) -> Self {
        Self::from_message(message)
    }
}

pub fn command_error_code(message: &str) -> &'static str {
    if message == "TRANSACTION_UNAVAILABLE" {
        "TRANSACTION_UNAVAILABLE"
    } else if message == "ROLLBACK_FAILED" {
        "ROLLBACK_FAILED"
    } else if message == "RECOVERY_FAILED" {
        "RECOVERY_FAILED"
    } else if message == "PROVIDER_INPUT_INVALID" {
        "PROVIDER_INPUT_INVALID"
    } else if message.starts_with("无法解析应用数据目录") {
        "DATA_DIRECTORY_UNAVAILABLE"
    } else if message.starts_with("无法创建日志目录")
        || message.starts_with("无法创建目录")
        || message.starts_with("无法创建应用数据目录")
    {
        "DIRECTORY_CREATE_FAILED"
    } else if message.starts_with("日志写入锁不可用")
        || message.starts_with("无法轮转")
        || message.starts_with("无法打开日志文件")
        || message.starts_with("无法写入日志")
        || message.starts_with("日志格式无效")
    {
        "LOG_UNAVAILABLE"
    } else if message.starts_with("无法打开目录") {
        "DIRECTORY_OPEN_FAILED"
    } else if message.starts_with("无法读取本地数据") {
        "DATA_READ_FAILED"
    } else if message.starts_with("本地数据格式错误") {
        "DATA_INVALID"
    } else if message.starts_with("无法序列化本地数据") || message.starts_with("无法保存本地数据")
    {
        "DATA_SAVE_FAILED"
    } else if message.starts_with("系统密钥库不可用") {
        "KEYRING_UNAVAILABLE"
    } else if message.starts_with("无法读取系统密钥库") {
        "KEYRING_READ_FAILED"
    } else if message.starts_with("无法写入系统密钥库") {
        "KEYRING_WRITE_FAILED"
    } else if message.starts_with("无法从系统密钥库删除") {
        "KEYRING_DELETE_FAILED"
    } else if message.starts_with("无法初始化网络客户端") {
        "NETWORK_CLIENT_FAILED"
    } else if message.starts_with("当前应用版本不符合")
        || message.starts_with("更新版本不符合")
        || message.starts_with("签名更新清单中的版本不符合")
    {
        "VERSION_INVALID"
    } else if message.starts_with("GitHub Releases 响应过大") {
        "UPDATE_RESPONSE_TOO_LARGE"
    } else if message.starts_with("GitHub Releases 数据格式错误") {
        "UPDATE_DATA_INVALID"
    } else if message.starts_with("无法初始化更新检测客户端")
        || message.starts_with("无法连接 GitHub Releases")
        || message.starts_with("GitHub Releases 返回异常状态")
        || message.starts_with("无法读取 GitHub Releases 响应")
    {
        "UPDATE_CHECK_FAILED"
    } else if message.starts_with("更新版本标签无效") {
        "UPDATE_TAG_INVALID"
    } else if message.starts_with("无法生成更新清单地址")
        || message.starts_with("无法配置更新端点")
        || message.starts_with("无法初始化自动更新")
        || message.starts_with("无法读取签名更新清单")
    {
        "UPDATE_MANIFEST_FAILED"
    } else if message.starts_with("更新下载超时") {
        "UPDATE_DOWNLOAD_TIMEOUT"
    } else if message.starts_with("该 Release 没有可安装的更新") {
        "UPDATE_NOT_AVAILABLE"
    } else if message.starts_with("Release 标签与签名更新清单版本不一致") {
        "UPDATE_DATA_INVALID"
    } else if message.starts_with("更新下载或签名验证失败") {
        "UPDATE_DOWNLOAD_FAILED"
    } else if message.starts_with("更新安装失败") {
        "UPDATE_INSTALL_FAILED"
    } else if message.starts_with("无法清空日志") {
        "LOG_CLEAR_FAILED"
    } else if message.starts_with("供应商名称不能为空") {
        "PROVIDER_NAME_REQUIRED"
    } else if message.starts_with("供应商名称已存在") {
        "PROVIDER_NAME_EXISTS"
    } else if message.starts_with("供应商已存在") {
        "PROVIDER_EXISTS"
    } else if message.starts_with("未找到供应商") {
        "PROVIDER_NOT_FOUND"
    } else if message.starts_with("供应商排序数据不完整") {
        "PROVIDER_ORDER_INVALID"
    } else if message.starts_with("供应商检测配置无效") {
        "PROVIDER_VALIDATION_INVALID"
    } else if message.starts_with("API Key 不能为空") {
        "API_KEY_REQUIRED"
    } else if message.starts_with("未找到 API Key") {
        "API_KEY_NOT_FOUND"
    } else if message.starts_with("无法写入剪贴板") {
        "CLIPBOARD_WRITE_FAILED"
    } else if message.starts_with("无法读取设置文件") {
        "SETTINGS_READ_FAILED"
    } else if message.starts_with("设置文件格式错误")
        || message.starts_with("设置文件版本不受支持")
        || message.starts_with("语言设置无效")
        || message.starts_with("主题设置无效")
    {
        "SETTINGS_INVALID"
    } else if message.starts_with("无法序列化设置文件")
        || message.starts_with("无法保存设置文件")
        || message.starts_with("无法写入设置临时文件")
        || message.starts_with("无法备份设置文件")
        || message.starts_with("无法替换设置文件")
        || message.starts_with("无法恢复设置文件")
    {
        "SETTINGS_SAVE_FAILED"
    } else {
        "UNKNOWN"
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiKeyRecord {
    id: String,
    provider_id: String,
    remark: String,
    secret_id: String,
    status: String,
    #[serde(default)]
    check_error_code: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum ValidationConfig {
    #[serde(rename = "none")]
    #[default]
    None,
    #[serde(rename = "openai-compatible")]
    OpenAiCompatible {
        #[serde(rename = "baseUrl")]
        base_url: String,
    },
    #[serde(rename = "bearer")]
    Bearer { endpoint: String },
    #[serde(rename = "api-key-header")]
    ApiKeyHeader {
        endpoint: String,
        #[serde(rename = "headerName")]
        header_name: String,
    },
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderRecord {
    id: String,
    name: String,
    abbreviation: String,
    tone: String,
    logo: Option<String>,
    kind: String,
    platform_url: Option<String>,
    #[serde(default)]
    validation: ValidationConfig,
    keys: Vec<ApiKeyRecord>,
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
struct AppData {
    providers: Vec<ProviderRecord>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
// 仅保存非敏感应用偏好；API Key 等凭据仍必须保留在系统密钥库中。
pub struct AppSettings {
    pub schema_version: u32,
    pub locale_preference: String,
    pub theme_preference: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientLogInput {
    pub level: String,
    pub event: String,
    #[serde(default)]
    pub detail: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            locale_preference: "system".into(),
            theme_preference: "system".into(),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeySummary {
    pub id: String,
    pub provider_id: String,
    pub remark: String,
    pub masked_value: String,
    pub status: String,
    pub check_error_code: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSummary {
    pub id: String,
    pub name: String,
    pub abbreviation: String,
    pub tone: String,
    pub logo: Option<String>,
    pub kind: String,
    pub platform_url: Option<String>,
    pub validation: ValidationConfig,
    pub validation_supported: bool,
    pub keys: Vec<ApiKeySummary>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProviderInput {
    pub id: String,
    pub name: String,
    pub abbreviation: String,
    pub tone: String,
    pub logo: Option<String>,
    pub kind: String,
    pub platform_url: Option<String>,
    #[serde(default)]
    pub validation: ValidationConfig,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderInput {
    pub id: String,
    pub name: String,
    pub platform_url: Option<String>,
    #[serde(default)]
    pub validation: ValidationConfig,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateKeyInput {
    pub provider_id: String,
    pub remark: String,
    pub value: String,
}

#[derive(Deserialize)]
pub struct UpdateKeyInput {
    pub id: String,
    pub remark: String,
    pub value: String,
}

fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}
const KEYRING_SERVICE: &str = "com.app.key-switch";
const LOG_FILE_NAME: &str = "key-switch.log";
const LOG_BACKUP_FILE_NAME: &str = "key-switch.log.1";
const SETTINGS_FILE_NAME: &str = "settings.json";
const SETTINGS_BACKUP_FILE_NAME: &str = "settings.json.bak";
const SETTINGS_TEMP_FILE_NAME: &str = "settings.json.tmp";
const SETTINGS_SCHEMA_VERSION: u32 = 1;
const MAX_LOG_FILE_SIZE: u64 = 1024 * 1024;
const MAX_CLIENT_LOG_DETAIL_CHARS: usize = 2400;

static LOG_LOCK: Mutex<()> = Mutex::new(());
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

pub fn data_file(app: &Core) -> Result<PathBuf, String> {
    Ok(app.directory.join("key-switch-data.json"))
}

pub fn settings_file(app: &Core) -> Result<PathBuf, String> {
    Ok(app.directory.join(SETTINGS_FILE_NAME))
}

fn settings_backup_file(app: &Core) -> Result<PathBuf, String> {
    Ok(settings_file(app)?
        .parent()
        .ok_or("无法创建应用数据目录")?
        .join(SETTINGS_BACKUP_FILE_NAME))
}

fn valid_locale_preference(value: &str) -> bool {
    matches!(value, "system" | "zh-CN" | "zh-TW" | "en-US" | "ja-JP")
}

fn valid_theme_preference(value: &str) -> bool {
    matches!(value, "system" | "light" | "dark")
}

fn validate_settings(settings: &AppSettings) -> Result<(), String> {
    if settings.schema_version != SETTINGS_SCHEMA_VERSION {
        return Err(format!("设置文件版本不受支持：{}", settings.schema_version));
    }
    if !valid_locale_preference(&settings.locale_preference) {
        return Err("语言设置无效".into());
    }
    if !valid_theme_preference(&settings.theme_preference) {
        return Err("主题设置无效".into());
    }
    Ok(())
}

fn load_settings(app: &Core) -> Result<AppSettings, String> {
    let file = settings_file(app)?;
    load_settings_from_file(&file)
}

fn load_settings_from_file(file: &PathBuf) -> Result<AppSettings, String> {
    if !file.exists() {
        recover_json_validated::<AppSettings>(file, |settings| validate_settings(settings).is_ok())
            .map_err(|_| "无法恢复设置文件")?;
    }

    let content = fs::read_to_string(file).map_err(|e| format!("无法读取设置文件：{e}"))?;
    let settings: AppSettings =
        serde_json::from_str(&content).map_err(|e| format!("设置文件格式错误：{e}"))?;
    validate_settings(&settings)?;
    Ok(settings)
}

fn save_settings(app: &Core, settings: &AppSettings) -> Result<(), String> {
    let file = settings_file(app)?;
    save_settings_to_file(&file, settings)
}

fn save_settings_to_file(file: &PathBuf, settings: &AppSettings) -> Result<(), String> {
    validate_settings(settings)?;
    let directory = file.parent().ok_or("无法创建应用数据目录")?;
    fs::create_dir_all(directory).map_err(|e| format!("无法创建应用数据目录：{e}"))?;

    let serialized =
        serde_json::to_string_pretty(settings).map_err(|e| format!("无法序列化设置文件：{e}"))?;
    let temporary = directory.join(SETTINGS_TEMP_FILE_NAME);
    let backup = directory.join(SETTINGS_BACKUP_FILE_NAME);
    let mut output = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&temporary)
        .map_err(|e| format!("无法写入设置临时文件：{e}"))?;
    output
        .write_all(serialized.as_bytes())
        .and_then(|_| output.sync_all())
        .map_err(|e| format!("无法写入设置临时文件：{e}"))?;
    drop(output);

    if backup.exists() {
        fs::remove_file(&backup).map_err(|e| format!("无法备份设置文件：{e}"))?;
    }
    let had_existing_file = file.exists();
    if had_existing_file {
        fs::rename(file, &backup).map_err(|e| format!("无法备份设置文件：{e}"))?;
    }

    if let Err(error) = fs::rename(&temporary, file) {
        if had_existing_file && backup.exists() {
            fs::rename(&backup, file).map_err(|restore_error| {
                format!("无法恢复设置文件：{restore_error}；原始错误：{error}")
            })?;
        }
        let _ = fs::remove_file(&temporary);
        return Err(format!("无法替换设置文件：{error}"));
    }

    if backup.exists() {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

pub fn log_directory(app: &Core) -> Result<PathBuf, String> {
    Ok(app.directory.join("logs"))
}

pub fn log_file(app: &Core) -> Result<PathBuf, String> {
    Ok(log_directory(app)?.join(LOG_FILE_NAME))
}

pub fn append_log(app: &Core, level: &str, event: &str, detail: &str) -> Result<(), String> {
    let _guard = LOG_LOCK
        .lock()
        .map_err(|_| "日志写入锁不可用".to_string())?;
    let directory = log_directory(app)?;
    fs::create_dir_all(&directory).map_err(|e| format!("无法创建日志目录：{e}"))?;
    let file = log_file(app)?;

    if file.metadata().map(|metadata| metadata.len()).unwrap_or(0) >= MAX_LOG_FILE_SIZE {
        let backup = directory.join(LOG_BACKUP_FILE_NAME);
        if backup.exists() {
            fs::remove_file(&backup).map_err(|e| format!("无法轮转旧日志：{e}"))?;
        }
        fs::rename(&file, backup).map_err(|e| format!("无法轮转日志：{e}"))?;
    }

    let mut output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
        .map_err(|e| format!("无法打开日志文件：{e}"))?;
    writeln!(output, "{} [{}] {} {}", now(), level, event, detail)
        .and_then(|_| output.flush())
        .and_then(|_| output.sync_data())
        .map_err(|e| format!("无法写入日志：{e}"))
}

pub fn valid_client_log_level(level: &str) -> bool {
    matches!(level, "INFO" | "WARN" | "ERROR")
}

pub fn valid_client_log_event(event: &str) -> bool {
    !event.is_empty()
        && event.chars().count() <= 80
        && event.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}

pub fn sanitize_client_log_detail(detail: &str) -> String {
    detail
        .chars()
        .filter(|character| !character.is_control() || *character == '\t')
        .take(MAX_CLIENT_LOG_DETAIL_CHARS)
        .collect()
}

fn load_data(app: &Core) -> Result<AppData, String> {
    recover_json::<AppData>(&data_file(app)?).map_err(|_| "本地数据格式错误".to_owned())?;
    let file = data_file(app)?;
    if !file.exists() {
        return Ok(AppData::default());
    }
    serde_json::from_slice(&fs::read(file).map_err(|_| "无法读取本地数据")?)
        .map_err(|_| "本地数据格式错误".into())
}
fn save_data(app: &Core, data: &AppData) -> Result<(), String> {
    atomic_save(&data_file(app)?, data).map_err(|_| "无法保存本地数据".into())
}

fn key_value(app: &Core, key: &ApiKeyRecord) -> Result<zeroize::Zeroizing<String>, String> {
    app.credentials
        .get(&key.secret_id)
        .map(zeroize::Zeroizing::new)
        .map_err(|_| "无法读取系统密钥库".into())
}
fn mask(value: &str) -> String {
    let count = value.chars().count();
    if count <= 8 {
        return "••••••••".into();
    }
    format!(
        "{}••••{}",
        value.chars().take(4).collect::<String>(),
        value.chars().skip(count - 4).collect::<String>()
    )
}
fn key_summary(app: &Core, key: &ApiKeyRecord) -> Result<ApiKeySummary, String> {
    Ok(ApiKeySummary {
        id: key.id.clone(),
        provider_id: key.provider_id.clone(),
        remark: key.remark.clone(),
        masked_value: mask(&key_value(app, key)?),
        status: key.status.clone(),
        check_error_code: key.check_error_code.clone(),
    })
}
fn summary(app: &Core, provider: &ProviderRecord) -> Result<ProviderSummary, String> {
    let validation_supported = key_validation_spec(provider).is_some();
    Ok(ProviderSummary {
        id: provider.id.clone(),
        name: provider.name.clone(),
        abbreviation: provider.abbreviation.clone(),
        tone: provider.tone.clone(),
        logo: provider.logo.clone(),
        kind: provider.kind.clone(),
        platform_url: provider.platform_url.clone(),
        validation: provider.validation.clone(),
        validation_supported,
        keys: provider
            .keys
            .iter()
            .map(|key| {
                let mut result = key_summary(app, key)?;
                if !validation_supported {
                    result.status = "unsupported".into();
                    result.check_error_code = None;
                }
                Ok::<ApiKeySummary, String>(result)
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

enum KeyValidationSpec {
    Bearer(String),
    ApiKeyHeader { url: String, header_name: String },
    Anthropic,
}

impl KeyValidationSpec {
    fn endpoint(&self) -> &str {
        match self {
            Self::Bearer(url) | Self::ApiKeyHeader { url, .. } => url,
            Self::Anthropic => "https://api.anthropic.com/v1/models?limit=1",
        }
    }
}

fn key_validation_spec(provider: &ProviderRecord) -> Option<KeyValidationSpec> {
    let builtin = match provider.id.as_str() {
        "openai" => Some(KeyValidationSpec::Bearer(
            "https://api.openai.com/v1/models".into(),
        )),
        "claude" | "anthropic" => Some(KeyValidationSpec::Anthropic),
        "gemini" | "aistudio" => Some(KeyValidationSpec::ApiKeyHeader {
            url: "https://generativelanguage.googleapis.com/v1beta/models?pageSize=1".into(),
            header_name: "x-goog-api-key".into(),
        }),
        "deepseek" => Some(KeyValidationSpec::Bearer(
            "https://api.deepseek.com/models".into(),
        )),
        "mimo" => Some(KeyValidationSpec::ApiKeyHeader {
            url: "https://api.xiaomimimo.com/v1/models".into(),
            header_name: "api-key".into(),
        }),
        "qwen" => Some(KeyValidationSpec::Bearer(
            "https://dashscope.aliyuncs.com/api/v1/deployments?page_no=1&page_size=1".into(),
        )),
        "kimi" => Some(KeyValidationSpec::Bearer(
            "https://api.moonshot.cn/v1/models".into(),
        )),
        "grok" => Some(KeyValidationSpec::Bearer(
            "https://api.x.ai/v1/models".into(),
        )),
        "openrouter" => Some(KeyValidationSpec::Bearer(
            "https://openrouter.ai/api/v1/key".into(),
        )),
        "minimax" => Some(KeyValidationSpec::Bearer(
            "https://api.minimaxi.com/v1/models".into(),
        )),
        "doubao" => Some(KeyValidationSpec::Bearer(
            "https://ark.cn-beijing.volces.com/ping".into(),
        )),
        "hunyuan" => Some(KeyValidationSpec::Bearer(
            "https://tokenhub.tencentmaas.com/v1/models".into(),
        )),
        "qianfan" => Some(KeyValidationSpec::Bearer(
            "https://qianfan.baidubce.com/v2/models".into(),
        )),
        "zhipu" => Some(KeyValidationSpec::Bearer(
            "https://open.bigmodel.cn/api/paas/v4/files".into(),
        )),
        _ => None,
    };
    if provider.kind == "builtin" {
        return builtin;
    }

    match &provider.validation {
        ValidationConfig::None => None,
        ValidationConfig::OpenAiCompatible { base_url } => Some(KeyValidationSpec::Bearer(
            format!("{}/models", base_url.trim_end_matches('/')),
        )),
        ValidationConfig::Bearer { endpoint } => Some(KeyValidationSpec::Bearer(endpoint.clone())),
        ValidationConfig::ApiKeyHeader {
            endpoint,
            header_name,
        } => Some(KeyValidationSpec::ApiKeyHeader {
            url: endpoint.clone(),
            header_name: header_name.clone(),
        }),
    }
}

#[derive(Debug, PartialEq)]
struct ValidationOutcome {
    status: &'static str,
    error_code: Option<&'static str>,
}

fn classify_validation_status(status: reqwest::StatusCode) -> ValidationOutcome {
    if status.is_success() {
        ValidationOutcome {
            status: "valid",
            error_code: None,
        }
    } else if status == reqwest::StatusCode::BAD_REQUEST
        || status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
    {
        ValidationOutcome {
            status: "invalid",
            error_code: None,
        }
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        ValidationOutcome {
            status: "error",
            error_code: Some("rateLimited"),
        }
    } else if status.is_server_error() {
        ValidationOutcome {
            status: "error",
            error_code: Some("serverUnavailable"),
        }
    } else if status.is_redirection() {
        ValidationOutcome {
            status: "error",
            error_code: Some("endpointInvalid"),
        }
    } else {
        ValidationOutcome {
            status: "error",
            error_code: Some("unexpectedStatus"),
        }
    }
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [first, second, ..] = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.is_multicast()
                || first == 0
                || (first == 100 && (64..=127).contains(&second))
                || (first == 198 && (18..=19).contains(&second))
                || first >= 240
        }
        IpAddr::V6(ip) => {
            let segments = ip.segments();
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast()
                || (segments[0] == 0x2001 && segments[1] == 0x0db8)
                || ip
                    .to_ipv4()
                    .is_some_and(|mapped| is_blocked_ip(IpAddr::V4(mapped)))
        }
    }
}

fn parse_safe_validation_url(value: &str) -> Result<reqwest::Url, String> {
    if value.len() > 2048 {
        return Err("供应商检测配置无效：检测地址过长".into());
    }
    let url = reqwest::Url::parse(value)
        .map_err(|_| "供应商检测配置无效：检测地址格式错误".to_string())?;
    if url.scheme() != "https" {
        return Err("供应商检测配置无效：检测地址必须使用 HTTPS".into());
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("供应商检测配置无效：检测地址不能包含凭据、查询参数或片段".into());
    }
    let host = url
        .host_str()
        .ok_or_else(|| "供应商检测配置无效：检测地址缺少域名".to_string())?;
    let lower_host = host.to_ascii_lowercase();
    if lower_host == "localhost"
        || lower_host.ends_with(".localhost")
        || lower_host.ends_with(".local")
        || lower_host.ends_with(".internal")
        || host.parse::<IpAddr>().is_ok_and(is_blocked_ip)
    {
        return Err("供应商检测配置无效：不允许本地或私有网络地址".into());
    }
    Ok(url)
}

fn valid_header_name(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    !value.is_empty()
        && value.len() <= 64
        && reqwest::header::HeaderName::from_bytes(value.as_bytes()).is_ok()
        && !matches!(
            lower.as_str(),
            "authorization"
                | "cookie"
                | "content-length"
                | "host"
                | "proxy-authorization"
                | "set-cookie"
        )
}

fn normalize_validation_config(
    kind: &str,
    validation: ValidationConfig,
) -> Result<ValidationConfig, String> {
    if kind != "custom" {
        return Ok(ValidationConfig::None);
    }
    match validation {
        ValidationConfig::None => Ok(ValidationConfig::None),
        ValidationConfig::OpenAiCompatible { base_url } => {
            let normalized = base_url.trim().trim_end_matches('/').to_string();
            parse_safe_validation_url(&normalized)?;
            Ok(ValidationConfig::OpenAiCompatible {
                base_url: normalized,
            })
        }
        ValidationConfig::Bearer { endpoint } => {
            let normalized = endpoint.trim().to_string();
            parse_safe_validation_url(&normalized)?;
            Ok(ValidationConfig::Bearer {
                endpoint: normalized,
            })
        }
        ValidationConfig::ApiKeyHeader {
            endpoint,
            header_name,
        } => {
            let normalized_endpoint = endpoint.trim().to_string();
            let normalized_header = header_name.trim().to_string();
            parse_safe_validation_url(&normalized_endpoint)?;
            if !valid_header_name(&normalized_header) {
                return Err("供应商检测配置无效：请求头名称无效或不受允许".into());
            }
            Ok(ValidationConfig::ApiKeyHeader {
                endpoint: normalized_endpoint,
                header_name: normalized_header,
            })
        }
    }
}

struct ResolvedValidationEndpoint {
    host: String,
    addresses: Vec<SocketAddr>,
}

async fn resolve_validation_endpoint(
    spec: &KeyValidationSpec,
) -> Result<ResolvedValidationEndpoint, &'static str> {
    let url = reqwest::Url::parse(spec.endpoint()).map_err(|_| "endpointInvalid")?;
    let host = url.host_str().ok_or("endpointInvalid")?;
    if host.parse::<IpAddr>().is_ok_and(is_blocked_ip) {
        return Err("endpointInvalid");
    }
    let port = url.port_or_known_default().ok_or("endpointInvalid")?;
    let addresses = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::lookup_host((host, port)),
    )
    .await
    .map_err(|_| "timeout")?
    .map_err(|_| "network")?;
    let mut safe_addresses = Vec::new();
    for address in addresses {
        if is_blocked_ip(address.ip()) {
            return Err("endpointInvalid");
        }
        safe_addresses.push(address);
    }
    if safe_addresses.is_empty() {
        return Err("network");
    }
    Ok(ResolvedValidationEndpoint {
        host: host.into(),
        addresses: safe_addresses,
    })
}

fn validation_client(resolved: &ResolvedValidationEndpoint) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Key-Switch/", env!("CARGO_PKG_VERSION")))
        .resolve_to_addrs(&resolved.host, &resolved.addresses)
        .build()
        .map_err(|e| format!("无法初始化网络客户端：{e}"))
}

fn build_validation_request(
    client: &reqwest::Client,
    spec: &KeyValidationSpec,
    value: &str,
) -> Result<reqwest::Request, ()> {
    let request = match spec {
        KeyValidationSpec::Bearer(url) => client.get(url).bearer_auth(value),
        KeyValidationSpec::ApiKeyHeader { url, header_name } => {
            let header =
                reqwest::header::HeaderName::from_bytes(header_name.as_bytes()).map_err(|_| ())?;
            client.get(url).header(header, value)
        }
        KeyValidationSpec::Anthropic => client
            .get("https://api.anthropic.com/v1/models?limit=1")
            .header("x-api-key", value)
            .header("anthropic-version", "2023-06-01"),
    };
    request.build().map_err(|_| ())
}

async fn validate_key(
    client: &reqwest::Client,
    spec: &KeyValidationSpec,
    value: &str,
) -> ValidationOutcome {
    let request = match build_validation_request(client, spec, value) {
        Ok(request) => request,
        Err(()) => {
            return ValidationOutcome {
                status: "error",
                error_code: Some("endpointInvalid"),
            }
        }
    };

    match client.execute(request).await {
        Ok(response) => classify_validation_status(response.status()),
        Err(error) => ValidationOutcome {
            status: "error",
            error_code: Some(if error.is_timeout() {
                "timeout"
            } else {
                "network"
            }),
        },
    }
}

pub fn load_app_settings(
    app: &Core,
    legacy_locale_preference: Option<String>,
) -> Result<AppSettings, CommandError> {
    let _guard = SETTINGS_LOCK
        .lock()
        .map_err(|_| CommandError::from("无法读取设置文件：设置锁不可用"))?;
    let file = settings_file(app)?;
    let backup = settings_backup_file(app)?;

    let settings = if file.exists() || backup.exists() || file.with_extension("json.tmp").exists() {
        load_settings(app)?
    } else {
        let mut settings = AppSettings::default();
        if let Some(preference) = legacy_locale_preference {
            if valid_locale_preference(&preference) {
                settings.locale_preference = preference;
            }
        }
        save_settings(app, &settings)?;
        let _ = append_log(app, "INFO", "settings_initialized", "success");
        settings
    };

    Ok(settings)
}

/// TUI uses existing preferences without creating or writing a settings file.
pub fn read_app_settings(app: &Core) -> Result<AppSettings, CommandError> {
    let _guard = SETTINGS_LOCK
        .lock()
        .map_err(|_| CommandError::from("无法读取设置文件"))?;
    let file = settings_file(app)?;
    if !file.exists()
        && !file.with_extension("json.bak").exists()
        && !file.with_extension("json.tmp").exists()
    {
        return Ok(AppSettings::default());
    }
    load_settings_from_file(&file).map_err(CommandError::from)
}

pub fn save_app_settings(app: &Core, settings: AppSettings) -> Result<AppSettings, CommandError> {
    let _guard = SETTINGS_LOCK
        .lock()
        .map_err(|_| CommandError::from("无法写入设置临时文件：设置锁不可用"))?;
    save_settings(app, &settings)?;
    let _ = append_log(app, "INFO", "settings_saved", "success");
    Ok(settings)
}

pub fn write_client_log(app: &Core, input: ClientLogInput) -> Result<(), CommandError> {
    if !valid_client_log_level(&input.level) || !valid_client_log_event(&input.event) {
        return Err("日志格式无效".into());
    }
    let detail = input
        .detail
        .as_deref()
        .map(sanitize_client_log_detail)
        .unwrap_or_default();
    append_log(app, &input.level, &input.event, &detail).map_err(CommandError::from)
}

pub fn clear_logs(app: &Core) -> Result<(), CommandError> {
    let _guard = LOG_LOCK
        .lock()
        .map_err(|_| "日志写入锁不可用".to_string())?;
    let directory = log_directory(app)?;
    fs::create_dir_all(&directory).map_err(|e| format!("无法创建日志目录：{e}"))?;
    for file_name in [LOG_FILE_NAME, LOG_BACKUP_FILE_NAME] {
        let file = directory.join(file_name);
        if file.exists() {
            fs::remove_file(file).map_err(|e| format!("无法清空日志：{e}"))?;
        }
    }
    Ok(())
}
pub fn list_providers(app: &Core) -> Result<Vec<ProviderSummary>, CommandError> {
    let _guard = app.read_transaction()?;
    load_data(app)?
        .providers
        .iter()
        .map(|provider| summary(app, provider).map_err(CommandError::from))
        .collect()
}
pub fn create_provider(
    app: &Core,
    input: CreateProviderInput,
) -> Result<ProviderSummary, CommandError> {
    let _guard = app.transaction()?;
    if input.name.trim().is_empty() {
        return Err("供应商名称不能为空".into());
    }
    let mut data = load_data(app)?;
    if data
        .providers
        .iter()
        .any(|p| p.id == input.id || p.name == input.name.trim())
    {
        return Err("供应商已存在".into());
    }
    validate_provider_input(&input.platform_url, &input.logo)?;
    if input.kind == "custom" {
        validate_custom_logo(input.logo.as_deref())?;
    }
    let validation = normalize_validation_config(&input.kind, input.validation)?;
    let provider = ProviderRecord {
        id: input.id,
        name: input.name.trim().into(),
        abbreviation: input.abbreviation,
        tone: input.tone,
        logo: input.logo,
        kind: input.kind,
        platform_url: input.platform_url.filter(|u| !u.trim().is_empty()),
        validation,
        keys: vec![],
    };
    let result = summary(app, &provider)?;
    data.providers.push(provider);
    save_data(app, &data)?;
    let _ = append_log(app, "INFO", "provider_created", "success");
    Ok(result)
}
pub fn update_provider(
    app: &Core,
    input: UpdateProviderInput,
) -> Result<ProviderSummary, CommandError> {
    let _guard = app.transaction()?;
    validate_provider_input(&input.platform_url, &None)?;
    let mut data = load_data(app)?;
    if input.name.trim().is_empty() {
        return Err("供应商名称不能为空".into());
    }
    if data
        .providers
        .iter()
        .any(|p| p.id != input.id && p.name == input.name.trim())
    {
        return Err("供应商名称已存在".into());
    }
    let provider = data
        .providers
        .iter_mut()
        .find(|p| p.id == input.id)
        .ok_or("未找到供应商")?;
    let next_validation = normalize_validation_config(&provider.kind, input.validation)?;
    let validation_changed = provider.validation != next_validation;
    provider.name = input.name.trim().into();
    provider.platform_url = input.platform_url.filter(|u| !u.trim().is_empty());
    provider.validation = next_validation;
    if validation_changed {
        app.invalidate_provider(&provider.id)?;
        let validation_supported = key_validation_spec(provider).is_some();
        for key in &mut provider.keys {
            key.status = if validation_supported {
                "untested".into()
            } else {
                "unsupported".into()
            };
            key.check_error_code = None;
        }
    }
    let result = summary(app, provider)?;
    save_data(app, &data)?;
    let _ = append_log(app, "INFO", "provider_updated", "success");
    Ok(result)
}

pub fn reorder_providers(app: &Core, provider_ids: Vec<String>) -> Result<(), CommandError> {
    let _guard = app.transaction()?;
    let mut data = load_data(app)?;
    if provider_ids.len() != data.providers.len()
        || provider_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != provider_ids.len()
        || data.providers.iter().any(|p| !provider_ids.contains(&p.id))
    {
        return Err("供应商排序数据不完整".into());
    }
    data.providers.sort_by_key(|p| {
        provider_ids
            .iter()
            .position(|id| id == &p.id)
            .unwrap_or(usize::MAX)
    });
    save_data(app, &data)?;
    let _ = append_log(app, "INFO", "providers_reordered", "success");
    Ok(())
}

pub fn copy_api_key(
    app: &Core,
    key_id: String,
    clipboard: &dyn Clipboard,
) -> Result<(), CommandError> {
    let _guard = app.transaction()?;
    let data = load_data(app)?;
    let key = data
        .providers
        .iter()
        .flat_map(|p| &p.keys)
        .find(|k| k.id == key_id)
        .ok_or("未找到 API Key")?;
    let value = key_value(app, key)?;
    clipboard
        .write(&value)
        .map_err(|_| CommandError::from("无法写入剪贴板"))?;
    let _ = append_log(app, "INFO", "api_key_copied", "success");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_validation_request, classify_validation_status, command_error_code,
        load_settings_from_file, normalize_validation_config, parse_safe_validation_url,
        sanitize_client_log_detail, save_settings_to_file, valid_client_log_event,
        valid_client_log_level, valid_header_name, valid_locale_preference, valid_theme_preference,
        AppData, AppSettings, ClientLogInput, KeyValidationSpec, ValidationConfig,
        ValidationOutcome, SETTINGS_BACKUP_FILE_NAME, SETTINGS_FILE_NAME, SETTINGS_SCHEMA_VERSION,
        SETTINGS_TEMP_FILE_NAME,
    };
    use reqwest::StatusCode;
    use std::{fs, process};

    #[test]
    fn classifies_key_validation_responses() {
        assert_eq!(
            classify_validation_status(StatusCode::OK),
            ValidationOutcome {
                status: "valid",
                error_code: None
            }
        );
        assert_eq!(
            classify_validation_status(StatusCode::UNAUTHORIZED),
            ValidationOutcome {
                status: "invalid",
                error_code: None
            }
        );
        assert_eq!(
            classify_validation_status(StatusCode::FORBIDDEN),
            ValidationOutcome {
                status: "invalid",
                error_code: None
            }
        );
        assert_eq!(
            classify_validation_status(StatusCode::TOO_MANY_REQUESTS),
            ValidationOutcome {
                status: "error",
                error_code: Some("rateLimited")
            }
        );
        assert_eq!(
            classify_validation_status(StatusCode::INTERNAL_SERVER_ERROR),
            ValidationOutcome {
                status: "error",
                error_code: Some("serverUnavailable")
            }
        );
    }

    #[test]
    fn keeps_old_provider_data_compatible() {
        let data: AppData = serde_json::from_str(
            r#"{"providers":[{"id":"custom-old","name":"Old","abbreviation":"OL","tone":"gray","logo":null,"kind":"custom","platformUrl":"https://example.com","keys":[{"id":"key-1","providerId":"custom-old","remark":"","secretId":"key-1","status":"untested","lastCheckedAt":null}]}]}"#,
        )
        .unwrap();
        assert_eq!(data.providers[0].validation, ValidationConfig::None);
        assert_eq!(data.providers[0].keys[0].check_error_code, None);
    }

    #[test]
    fn validates_custom_provider_check_configuration() {
        assert!(parse_safe_validation_url("https://api.example.com/v1/models").is_ok());
        for unsafe_url in [
            "http://api.example.com/v1/models",
            "https://localhost/v1/models",
            "https://127.0.0.1/v1/models",
            "https://10.0.0.1/v1/models",
            "https://user:pass@example.com/v1/models",
            "https://api.example.com/v1/models?key=value",
        ] {
            assert!(
                parse_safe_validation_url(unsafe_url).is_err(),
                "{unsafe_url}"
            );
        }
        assert!(valid_header_name("x-api-key"));
        assert!(!valid_header_name("Authorization"));
        assert!(!valid_header_name("bad header"));

        assert_eq!(
            normalize_validation_config(
                "custom",
                ValidationConfig::OpenAiCompatible {
                    base_url: " https://api.example.com/v1/ ".into(),
                },
            )
            .unwrap(),
            ValidationConfig::OpenAiCompatible {
                base_url: "https://api.example.com/v1".into(),
            }
        );
    }

    #[test]
    fn builds_supported_key_authentication_requests() {
        let client = reqwest::Client::new();
        let bearer = build_validation_request(
            &client,
            &KeyValidationSpec::Bearer("https://api.example.com/v1/models".into()),
            "test-key",
        )
        .unwrap();
        assert_eq!(bearer.headers()["authorization"], "Bearer test-key");

        let header = build_validation_request(
            &client,
            &KeyValidationSpec::ApiKeyHeader {
                url: "https://api.example.com/v1/models".into(),
                header_name: "x-api-key".into(),
            },
            "test-key",
        )
        .unwrap();
        assert_eq!(header.headers()["x-api-key"], "test-key");
    }

    #[test]
    fn maps_internal_errors_to_stable_command_codes() {
        assert_eq!(
            command_error_code("更新下载超时，请检查网络后重试"),
            "UPDATE_DOWNLOAD_TIMEOUT"
        );
        assert_eq!(
            command_error_code("供应商名称已存在"),
            "PROVIDER_NAME_EXISTS"
        );
        assert_eq!(
            command_error_code("无法保存设置文件：disk full"),
            "SETTINGS_SAVE_FAILED"
        );
        assert_eq!(command_error_code("主题设置无效"), "SETTINGS_INVALID");
        assert_eq!(
            command_error_code("供应商检测配置无效：检测地址必须使用 HTTPS"),
            "PROVIDER_VALIDATION_INVALID"
        );
        assert_eq!(command_error_code("unexpected"), "UNKNOWN");
    }

    #[test]
    fn validates_supported_locale_preferences() {
        for locale in ["system", "zh-CN", "zh-TW", "en-US", "ja-JP"] {
            assert!(valid_locale_preference(locale));
        }
        assert!(!valid_locale_preference("fr-FR"));
    }

    #[test]
    fn fills_missing_settings_fields_with_schema_defaults() {
        let settings: AppSettings =
            serde_json::from_str(r#"{"localePreference":"en-US"}"#).unwrap();
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(settings.locale_preference, "en-US");
        assert_eq!(settings.theme_preference, "system");
    }

    #[test]
    fn validates_supported_theme_preferences() {
        for theme in ["system", "light", "dark"] {
            assert!(valid_theme_preference(theme));
        }
        assert!(!valid_theme_preference("sepia"));
    }

    #[test]
    fn validates_and_sanitizes_client_log_entries() {
        for level in ["INFO", "WARN", "ERROR"] {
            assert!(valid_client_log_level(level));
        }
        assert!(!valid_client_log_level("DEBUG"));
        assert!(valid_client_log_event("vue_error"));
        assert!(valid_client_log_event("tauri.command.failed"));
        assert!(!valid_client_log_event("vue error"));
        assert!(!valid_client_log_event(""));
        assert_eq!(
            sanitize_client_log_detail("first\nsecond\rthird"),
            "firstsecondthird"
        );
        let entry: ClientLogInput =
            serde_json::from_str(r#"{"level":"INFO","event":"frontend_started"}"#).unwrap();
        assert_eq!(entry.detail, None);
    }

    #[test]
    fn persists_settings_and_recovers_an_interrupted_replacement() {
        let directory = std::env::temp_dir().join(format!(
            "key-switch-settings-test-{}-{}",
            process::id(),
            super::now()
        ));
        fs::create_dir_all(&directory).unwrap();
        let file = directory.join(SETTINGS_FILE_NAME);
        let initial = AppSettings {
            schema_version: SETTINGS_SCHEMA_VERSION,
            locale_preference: "en-US".into(),
            theme_preference: "light".into(),
        };
        save_settings_to_file(&file, &initial).unwrap();
        let loaded_initial = load_settings_from_file(&file).unwrap();
        assert_eq!(loaded_initial.locale_preference, "en-US");
        assert_eq!(loaded_initial.theme_preference, "light");

        let replacement = AppSettings {
            schema_version: SETTINGS_SCHEMA_VERSION,
            locale_preference: "ja-JP".into(),
            theme_preference: "dark".into(),
        };
        fs::write(
            directory.join(SETTINGS_TEMP_FILE_NAME),
            serde_json::to_string_pretty(&replacement).unwrap(),
        )
        .unwrap();
        fs::rename(&file, directory.join(SETTINGS_BACKUP_FILE_NAME)).unwrap();
        let loaded_replacement = load_settings_from_file(&file).unwrap();
        assert_eq!(loaded_replacement.locale_preference, "ja-JP");
        assert_eq!(loaded_replacement.theme_preference, "dark");

        fs::remove_dir_all(directory).unwrap();
    }
}
