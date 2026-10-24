use key_switch_core::*;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fs, path::PathBuf, process::Command, time::Duration};
use tauri::Manager;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_updater::UpdaterExt;
const AUTOMATIC_UPDATES_ENABLED: bool = true;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    data_directory: String,
    log_directory: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo {
    current_version: String,
    latest_version: String,
    title: String,
    notes: String,
    release_url: String,
    prerelease: bool,
    published_at: Option<String>,
    release_tag: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    html_url: String,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PrereleaseIdentifier {
    Numeric(u64),
    Text(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SemVersion {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Vec<PrereleaseIdentifier>,
}

impl SemVersion {
    fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        let value = value.strip_prefix("app-").unwrap_or(value);
        let value = value
            .strip_prefix('v')
            .or_else(|| value.strip_prefix('V'))
            .unwrap_or(value);
        let value = value
            .split_once('+')
            .map(|(version, _)| version)
            .unwrap_or(value);
        let (core, prerelease) = value
            .split_once('-')
            .map(|(core, prerelease)| (core, Some(prerelease)))
            .unwrap_or((value, None));
        let mut numbers = core.split('.');
        let major = numbers.next()?.parse().ok()?;
        let minor = numbers.next()?.parse().ok()?;
        let patch = numbers.next()?.parse().ok()?;
        if numbers.next().is_some() {
            return None;
        }
        let prerelease = prerelease
            .map(|value| {
                value
                    .split('.')
                    .filter(|item| !item.is_empty())
                    .map(|item| {
                        item.parse::<u64>()
                            .map(PrereleaseIdentifier::Numeric)
                            .unwrap_or_else(|_| {
                                PrereleaseIdentifier::Text(item.to_ascii_lowercase())
                            })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Self {
            major,
            minor,
            patch,
            prerelease,
        })
    }

    fn is_prerelease(&self) -> bool {
        !self.prerelease.is_empty()
    }
}

impl Ord for SemVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
            .then_with(
                || match (self.prerelease.is_empty(), other.prerelease.is_empty()) {
                    (true, true) => Ordering::Equal,
                    (true, false) => Ordering::Greater,
                    (false, true) => Ordering::Less,
                    (false, false) => {
                        for (left, right) in self.prerelease.iter().zip(&other.prerelease) {
                            let ordering = match (left, right) {
                                (
                                    PrereleaseIdentifier::Numeric(left),
                                    PrereleaseIdentifier::Numeric(right),
                                ) => left.cmp(right),
                                (
                                    PrereleaseIdentifier::Numeric(_),
                                    PrereleaseIdentifier::Text(_),
                                ) => Ordering::Less,
                                (
                                    PrereleaseIdentifier::Text(_),
                                    PrereleaseIdentifier::Numeric(_),
                                ) => Ordering::Greater,
                                (
                                    PrereleaseIdentifier::Text(left),
                                    PrereleaseIdentifier::Text(right),
                                ) => left.cmp(right),
                            };
                            if ordering != Ordering::Equal {
                                return ordering;
                            }
                        }
                        self.prerelease.len().cmp(&other.prerelease.len())
                    }
                },
            )
    }
}

impl PartialOrd for SemVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn open_directory(directory: PathBuf) -> Result<(), String> {
    fs::create_dir_all(&directory).map_err(|e| format!("无法创建目录：{e}"))?;
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    command
        .arg(directory)
        .spawn()
        .map_err(|e| format!("无法打开目录：{e}"))?;
    Ok(())
}
fn append_log(
    app: &tauri::AppHandle,
    level: &str,
    event: &str,
    detail: &str,
) -> Result<(), String> {
    key_switch_core::append_log(&app.state::<Core>(), level, event, detail)
}
fn log_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    key_switch_core::log_directory(&app.state::<Core>())
}
#[tauri::command]
fn get_app_info(app: tauri::AppHandle) -> Result<AppInfo, CommandError> {
    let log_directory = log_directory(&app)?;
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        data_directory: app
            .path()
            .app_data_dir()
            .map_err(|e| format!("无法解析应用数据目录：{e}"))?
            .display()
            .to_string(),
        log_directory: log_directory.display().to_string(),
    })
}

#[tauri::command]
async fn check_for_updates(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, CommandError> {
    const RELEASES_API: &str =
        "https://api.github.com/repos/ThirteenAsh/key-switch/releases?per_page=20";
    const RELEASE_URL_PREFIX: &str = "https://github.com/ThirteenAsh/key-switch/releases/";
    const MAX_RESPONSE_SIZE: usize = 512 * 1024;

    let current_version_text = app.package_info().version.to_string();
    let current_version =
        SemVersion::parse(&current_version_text).ok_or("当前应用版本不符合 SemVer 规范")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent(concat!(
            "Key-Switch-Update-Check/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|e| format!("无法初始化更新检测客户端：{e}"))?;
    let response = client
        .get(RELEASES_API)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| format!("无法连接 GitHub Releases：{e}"))?;
    if !response.status().is_success() {
        return Err(format!("GitHub Releases 返回异常状态：{}", response.status()).into());
    }
    if response.content_length().unwrap_or(0) > MAX_RESPONSE_SIZE as u64 {
        return Err("GitHub Releases 响应过大".into());
    }
    let body = response
        .bytes()
        .await
        .map_err(|e| format!("无法读取 GitHub Releases 响应：{e}"))?;
    if body.len() > MAX_RESPONSE_SIZE {
        return Err("GitHub Releases 响应过大".into());
    }
    let releases: Vec<GithubRelease> =
        serde_json::from_slice(&body).map_err(|e| format!("GitHub Releases 数据格式错误：{e}"))?;

    let candidate = releases
        .into_iter()
        .filter(|release| !release.draft && release.html_url.starts_with(RELEASE_URL_PREFIX))
        .filter_map(|release| {
            SemVersion::parse(&release.tag_name).map(|version| (release, version))
        })
        .filter(|(release, version)| {
            version > &current_version && (current_version.is_prerelease() || !release.prerelease)
        })
        .max_by(|(_, left), (_, right)| left.cmp(right));

    let Some((release, _)) = candidate else {
        let _ = append_log(&app, "INFO", "update_checked", "available=false");
        return Ok(None);
    };
    let notes = release
        .body
        .unwrap_or_default()
        .chars()
        .take(4000)
        .collect();
    let update = UpdateInfo {
        current_version: current_version_text,
        latest_version: release
            .tag_name
            .trim_start_matches("app-")
            .trim_start_matches('v')
            .into(),
        title: release.name.unwrap_or_else(|| release.tag_name.clone()),
        notes,
        release_url: release.html_url,
        prerelease: release.prerelease,
        published_at: release.published_at,
        release_tag: release.tag_name,
    };
    let _ = append_log(&app, "INFO", "update_checked", "available=true");
    Ok(Some(update))
}

#[tauri::command]
async fn install_update(app: tauri::AppHandle, release_tag: String) -> Result<(), CommandError> {
    const RELEASE_TAG_PREFIX: &str = "v";
    const LEGACY_RELEASE_TAG_PREFIX: &str = "app-v";
    const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(90);

    let tag_is_safe = release_tag.len() <= 64
        && release_tag
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
        && (release_tag.starts_with(RELEASE_TAG_PREFIX)
            || release_tag.starts_with(LEGACY_RELEASE_TAG_PREFIX));
    if !tag_is_safe {
        return Err("更新版本标签无效".into());
    }
    let expected_version = SemVersion::parse(&release_tag).ok_or("更新版本不符合 SemVer 规范")?;
    let endpoint = reqwest::Url::parse(&format!(
        "https://github.com/ThirteenAsh/key-switch/releases/download/{release_tag}/latest.json"
    ))
    .map_err(|e| format!("无法生成更新清单地址：{e}"))?;
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| format!("无法配置更新端点：{e}"))?
        // 单个 HTTP 请求的上限略高于业务层总超时，由业务层统一返回可识别的超时错误。
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| format!("无法初始化自动更新：{e}"))?;
    let update = tokio::time::timeout(DOWNLOAD_TIMEOUT, updater.check())
        .await
        .map_err(|_| {
            let _ = append_log(
                &app,
                "WARN",
                "update_manifest_timeout",
                "timeout_seconds=90",
            );
            "更新下载超时，请检查网络后重试".to_string()
        })?
        .map_err(|e| format!("无法读取签名更新清单：{e}"))?
        .ok_or("该 Release 没有可安装的更新")?;
    let manifest_version =
        SemVersion::parse(&update.version).ok_or("签名更新清单中的版本不符合 SemVer 规范")?;
    if manifest_version != expected_version {
        return Err("Release 标签与签名更新清单版本不一致".into());
    }

    let _ = append_log(
        &app,
        "INFO",
        "update_install_started",
        "signature_check=pending",
    );
    let update_bytes = tokio::time::timeout(DOWNLOAD_TIMEOUT, update.download(|_, _| {}, || {}))
        .await
        .map_err(|_| {
            let _ = append_log(
                &app,
                "WARN",
                "update_download_timeout",
                "timeout_seconds=90",
            );
            "更新下载超时，请检查网络后重试".to_string()
        })?
        .map_err(|e| format!("更新下载或签名验证失败：{e}"))?;
    update
        .install(update_bytes)
        .map_err(|e| format!("更新安装失败：{e}"))?;
    let _ = append_log(&app, "INFO", "update_installed", "restart=pending");
    app.restart();
}
#[tauri::command]
fn open_data_directory(app: tauri::AppHandle) -> Result<(), CommandError> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法解析应用数据目录：{e}"))?;
    open_directory(directory)?;
    let _ = append_log(&app, "INFO", "data_directory_opened", "success");
    Ok(())
}

#[tauri::command]
fn open_log_directory(app: tauri::AppHandle) -> Result<(), CommandError> {
    open_directory(log_directory(&app)?)?;
    let _ = append_log(&app, "INFO", "log_directory_opened", "success");
    Ok(())
}

#[tauri::command]
fn load_app_settings(
    app: tauri::AppHandle,
    legacy_locale_preference: Option<String>,
) -> Result<AppSettings, CommandError> {
    key_switch_core::load_app_settings(&app.state::<Core>(), legacy_locale_preference)
}
#[tauri::command]
fn save_app_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
) -> Result<AppSettings, CommandError> {
    key_switch_core::save_app_settings(&app.state::<Core>(), settings)
}
#[tauri::command]
fn write_client_log(app: tauri::AppHandle, input: ClientLogInput) -> Result<(), CommandError> {
    key_switch_core::write_client_log(&app.state::<Core>(), input)
}
#[tauri::command]
fn clear_logs(app: tauri::AppHandle) -> Result<(), CommandError> {
    key_switch_core::clear_logs(&app.state::<Core>())
}
#[tauri::command]
fn list_providers(app: tauri::AppHandle) -> Result<Vec<ProviderSummary>, CommandError> {
    key_switch_core::list_providers(&app.state::<Core>())
}
#[tauri::command]
fn create_provider(
    app: tauri::AppHandle,
    input: CreateProviderInput,
) -> Result<ProviderSummary, CommandError> {
    key_switch_core::create_provider(&app.state::<Core>(), input)
}
#[tauri::command]
fn update_provider(
    app: tauri::AppHandle,
    input: UpdateProviderInput,
) -> Result<ProviderSummary, CommandError> {
    key_switch_core::update_provider(&app.state::<Core>(), input)
}
#[tauri::command]
fn delete_provider(app: tauri::AppHandle, provider_id: String) -> Result<(), CommandError> {
    key_switch_core::delete_provider(&app.state::<Core>(), provider_id)
}
#[tauri::command]
fn reorder_providers(app: tauri::AppHandle, provider_ids: Vec<String>) -> Result<(), CommandError> {
    key_switch_core::reorder_providers(&app.state::<Core>(), provider_ids)
}
#[tauri::command]
fn create_api_key(
    app: tauri::AppHandle,
    input: CreateKeyInput,
) -> Result<ApiKeySummary, CommandError> {
    key_switch_core::create_api_key(&app.state::<Core>(), input)
}
#[tauri::command]
fn update_api_key(
    app: tauri::AppHandle,
    input: UpdateKeyInput,
) -> Result<ApiKeySummary, CommandError> {
    key_switch_core::update_api_key(&app.state::<Core>(), input)
}
#[tauri::command]
fn copy_api_key(app: tauri::AppHandle, key_id: String) -> Result<(), CommandError> {
    key_switch_core::copy_api_key(&app.state::<Core>(), key_id, &DesktopClipboard(&app))
}
#[tauri::command]
fn delete_api_key(app: tauri::AppHandle, key_id: String) -> Result<(), CommandError> {
    key_switch_core::delete_api_key(&app.state::<Core>(), key_id)
}
#[tauri::command]
async fn check_provider_keys(
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<Vec<ApiKeySummary>, CommandError> {
    key_switch_core::check_provider_keys(&app.state::<Core>(), provider_id).await
}
#[tauri::command]
async fn check_api_key(
    app: tauri::AppHandle,
    provider_id: String,
    key_id: String,
) -> Result<ApiKeySummary, CommandError> {
    key_switch_core::check_api_key(&app.state::<Core>(), provider_id, key_id).await
}
struct DesktopClipboard<'a>(&'a tauri::AppHandle);
impl Clipboard for DesktopClipboard<'_> {
    fn write(&self, value: &str) -> Result<(), CommandError> {
        self.0
            .clipboard()
            .write_text(value.to_string())
            .map_err(|_| CommandError::from("无法写入剪贴板"))
    }
}

fn show_startup_error(message: &str) {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "Key Switch".encode_utf16().chain(Some(0)).collect();
        // Both buffers are NUL terminated and remain alive for this synchronous call.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        rfd::MessageDialog::new()
            .set_title("Key Switch")
            .set_description(message)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init());
    if AUTOMATIC_UPDATES_ENABLED {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }
    builder
        .setup(|app| {
            let directory = app.path().app_data_dir().unwrap_or_else(|_| {
                show_startup_error(startup_message(system_locale(), StartupError::Unavailable));
                std::process::exit(1);
            });
            let lock = match InstanceLock::acquire(&directory) {
                Ok(lock) => lock,
                Err(error) => {
                    let message = startup_message(system_locale(), error);
                    show_startup_error(message);
                    std::process::exit(1);
                }
            };
            let core =
                Core::new(lock, std::sync::Arc::new(SystemCredentials)).unwrap_or_else(|error| {
                    show_startup_error(&localized_error(system_locale(), error.code));
                    std::process::exit(1);
                });
            app.manage(core);
            tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?.build()?;
            let _ = append_log(app.handle(), "INFO", "application_started", "success");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_app_settings,
            save_app_settings,
            get_app_info,
            check_for_updates,
            install_update,
            open_data_directory,
            open_log_directory,
            write_client_log,
            clear_logs,
            list_providers,
            create_provider,
            update_provider,
            delete_provider,
            reorder_providers,
            create_api_key,
            update_api_key,
            copy_api_key,
            delete_api_key,
            check_api_key,
            check_provider_keys
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_version_precedence() {
        assert!(SemVersion::parse("1.1.0").unwrap() > SemVersion::parse("1.1.0-beta.1").unwrap());
        assert!(SemVersion::parse("invalid").is_none());
    }
}
