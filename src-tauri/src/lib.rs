use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;
use tauri_plugin_updater::{Update, UpdaterExt};

type HmacSha1 = Hmac<Sha1>;
const DEFAULT_UPDATER_PUBKEY: &str =
    "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEM0OEQ0NzA0OUIzREMyRTUKUldUbHdqMmJCRWVOeEUzbEZodFhrZXRXVEJKblRqOVRtNVEwcWdTY2NvRGs1eVYyVkdXYUpiYmIK";
const DEFAULT_UPDATER_ENDPOINT: &str =
    "https://github.com/designer9999/authenticator/releases/latest/download/latest.json";

/// Forward-compatible: #[serde(default)] ensures old JSON files load
/// even if future versions add new fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Account {
    pub id: String,
    pub issuer: String,
    pub name: String,
    pub password: String,
    pub secret: String,
    pub digits: u32,
    pub period: u32,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            id: String::new(),
            issuer: String::new(),
            name: String::new(),
            password: String::new(),
            secret: String::new(),
            digits: 6,
            period: 30,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AccountCode {
    pub id: String,
    pub issuer: String,
    pub name: String,
    pub password: String,
    pub code: String,
    pub has_code: bool,
    pub digits: u32,
    pub period: u32,
    pub remaining: u32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountDetails {
    pub id: String,
    pub issuer: String,
    pub name: String,
    pub password: String,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    custom_data_path: Option<String>,
}

pub struct AppState {
    pub accounts: Vec<Account>,
    pub config: AppConfig,
}

#[derive(Default)]
struct PendingUpdate(Mutex<Option<Update>>);

// ── Helpers ──

/// Strip whitespace and uppercase a base32 secret. Returns Err if invalid.
fn clean_secret(raw: &str) -> Result<String, String> {
    let clean: String = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    if clean.is_empty() {
        return Err("Secret is empty".into());
    }
    BASE32_NOPAD
        .decode(clean.as_bytes())
        .map_err(|e| format!("Invalid base32 secret: {e}"))?;
    Ok(clean)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn unique_id() -> String {
    format!(
        "{}{:04}",
        chrono::Utc::now().timestamp_millis(),
        rand::random::<u16>() % 10000
    )
}

fn default_import_issuer() -> String {
    "Google".into()
}

fn looks_like_secret(raw: &str) -> bool {
    let clean: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() < 16 {
        return false;
    }
    clean
        .chars()
        .all(|c| matches!(c.to_ascii_uppercase(), 'A'..='Z' | '2'..='7'))
}

fn account_exists(
    accounts: &[Account],
    issuer: &str,
    name: &str,
    password: &str,
    secret: &str,
) -> bool {
    accounts.iter().any(|account| {
        account.issuer.eq_ignore_ascii_case(issuer)
            && account.name.eq_ignore_ascii_case(name)
            && account.password == password
            && account.secret == secret
    })
}

// ── Storage ──

fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create config dir: {e}"))?;
    Ok(dir.join("config.json"))
}

fn load_config(app: &tauri::AppHandle) -> AppConfig {
    config_path(app)
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

fn save_config(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = config_path(app)?;
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| format!("Failed to save config: {e}"))
}

fn store_path(app: &tauri::AppHandle, config: &AppConfig) -> Result<PathBuf, String> {
    if let Some(ref custom) = config.custom_data_path {
        let p = PathBuf::from(custom);
        fs::create_dir_all(&p).map_err(|e| format!("Cannot create data dir: {e}"))?;
        return Ok(p.join("accounts.json"));
    }
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create data dir: {e}"))?;
    Ok(dir.join("accounts.json"))
}

fn load(app: &tauri::AppHandle, config: &AppConfig) -> Vec<Account> {
    store_path(app, config)
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

fn save(app: &tauri::AppHandle, config: &AppConfig, accounts: &[Account]) -> Result<(), String> {
    let path = store_path(app, config)?;
    let json = serde_json::to_string_pretty(accounts).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| format!("Failed to save accounts: {e}"))
}

fn lock_state<'a>(
    state: &'a tauri::State<'_, Mutex<AppState>>,
) -> Result<std::sync::MutexGuard<'a, AppState>, String> {
    state.lock().map_err(|_| "State lock poisoned".to_string())
}

fn updater_pubkey() -> Option<String> {
    option_env!("TAURI_UPDATER_PUBKEY")
        .map(|value| value.replace("\\n", "\n"))
        .or_else(|| Some(DEFAULT_UPDATER_PUBKEY.to_string()))
        .filter(|value| !value.trim().is_empty())
}

fn updater_endpoint() -> &'static str {
    option_env!("TAURI_UPDATER_ENDPOINT")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(DEFAULT_UPDATER_ENDPOINT)
}

// ── TOTP ──

fn totp(secret_b32: &str, digits: u32, period: u32) -> Result<String, String> {
    let key = BASE32_NOPAD
        .decode(secret_b32.as_bytes())
        .map_err(|e| format!("Bad base32: {e}"))?;

    let step = (now_secs() / period as u64).to_be_bytes();

    let mut mac = HmacSha1::new_from_slice(&key).map_err(|e| format!("HMAC error: {e}"))?;
    mac.update(&step);
    let hash = mac.finalize().into_bytes();

    let off = (hash[19] & 0x0f) as usize;
    let bin = ((hash[off] as u32 & 0x7f) << 24)
        | ((hash[off + 1] as u32) << 16)
        | ((hash[off + 2] as u32) << 8)
        | (hash[off + 3] as u32);

    let otp = bin % 10u32.pow(digits);
    Ok(format!("{:0>w$}", otp, w = digits as usize))
}

// ── Commands ──

#[tauri::command]
fn get_accounts(state: tauri::State<'_, Mutex<AppState>>) -> Result<Vec<AccountCode>, String> {
    let st = lock_state(&state)?;
    let now = now_secs();
    Ok(st
        .accounts
        .iter()
        .map(|a| {
            let has_code = !a.secret.trim().is_empty();
            let period = a.period as u64;
            AccountCode {
                id: a.id.clone(),
                issuer: a.issuer.clone(),
                name: a.name.clone(),
                password: a.password.clone(),
                code: if has_code {
                    totp(&a.secret, a.digits, a.period).unwrap_or_else(|_| "------".into())
                } else {
                    String::new()
                },
                has_code,
                digits: a.digits,
                period: a.period,
                remaining: if has_code {
                    a.period - (now % period) as u32
                } else {
                    0
                },
                created_at: a.id[..13.min(a.id.len())].parse::<i64>().unwrap_or(0),
            }
        })
        .collect())
}

#[tauri::command]
fn add_account(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    issuer: String,
    name: String,
    password: Option<String>,
    secret: Option<String>,
) -> Result<(), String> {
    let account = Account {
        id: unique_id(),
        issuer,
        name,
        password: password.unwrap_or_default(),
        secret: match secret {
            Some(raw) if !raw.trim().is_empty() => clean_secret(&raw)?,
            _ => String::new(),
        },
        digits: 6,
        period: 30,
    };
    let mut st = lock_state(&state)?;
    if account_exists(
        &st.accounts,
        &account.issuer,
        &account.name,
        &account.password,
        &account.secret,
    ) {
        return Ok(());
    }
    st.accounts.push(account);
    save(&app, &st.config, &st.accounts)
}

#[tauri::command]
fn remove_account(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), String> {
    let mut st = lock_state(&state)?;
    st.accounts.retain(|a| a.id != id);
    save(&app, &st.config, &st.accounts)
}

#[tauri::command]
fn edit_account(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    id: String,
    issuer: String,
    name: String,
    password: Option<String>,
    secret: Option<String>,
) -> Result<(), String> {
    let mut st = lock_state(&state)?;
    let acc = st
        .accounts
        .iter_mut()
        .find(|a| a.id == id)
        .ok_or("Account not found")?;
    acc.issuer = issuer;
    acc.name = name;
    if let Some(p) = password {
        acc.password = p;
    }
    if let Some(s) = secret {
        acc.secret = if s.trim().is_empty() {
            String::new()
        } else {
            clean_secret(&s)?
        };
    }
    save(&app, &st.config, &st.accounts)
}

#[tauri::command]
fn get_account_details(
    state: tauri::State<'_, Mutex<AppState>>,
    id: String,
) -> Result<AccountDetails, String> {
    let st = lock_state(&state)?;
    let acc = st
        .accounts
        .iter()
        .find(|a| a.id == id)
        .ok_or("Account not found")?;
    Ok(AccountDetails {
        id: acc.id.clone(),
        issuer: acc.issuer.clone(),
        name: acc.name.clone(),
        password: acc.password.clone(),
        secret: acc.secret.clone(),
    })
}

#[tauri::command]
fn reorder_accounts(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    ids: Vec<String>,
) -> Result<(), String> {
    let mut st = lock_state(&state)?;
    let mut reordered = Vec::with_capacity(ids.len());
    for id in &ids {
        if let Some(acc) = st.accounts.iter().find(|a| &a.id == id) {
            reordered.push(acc.clone());
        }
    }
    st.accounts = reordered;
    save(&app, &st.config, &st.accounts)
}

/// Bulk import: each line is "name:password:secret", "name:password", or "name:secret"
#[tauri::command]
fn bulk_import(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    text: String,
) -> Result<u32, String> {
    let mut st = lock_state(&state)?;
    let mut count = 0u32;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(':').collect();
        let (name, password, secret_raw) = if parts.len() >= 3 {
            (
                parts[0].trim().to_string(),
                parts[1..parts.len() - 1].join(":").trim().to_string(),
                Some(parts[parts.len() - 1].trim().to_string()),
            )
        } else if parts.len() == 2 {
            let second = parts[1].trim().to_string();
            if looks_like_secret(&second) {
                (parts[0].trim().to_string(), String::new(), Some(second))
            } else {
                (parts[0].trim().to_string(), second, None)
            }
        } else {
            continue;
        };
        let clean = match secret_raw {
            Some(secret_raw) => match clean_secret(&secret_raw) {
                Ok(c) => c,
                Err(_) => continue,
            },
            None => String::new(),
        };
        if account_exists(
            &st.accounts,
            &default_import_issuer(),
            &name,
            &password,
            &clean,
        ) {
            continue;
        }
        st.accounts.push(Account {
            id: unique_id(),
            issuer: default_import_issuer(),
            name,
            password,
            secret: clean,
            digits: 6,
            period: 30,
        });
        count += 1;
    }
    if count > 0 {
        save(&app, &st.config, &st.accounts)?;
    }
    Ok(count)
}

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub version: String,
    pub account_count: usize,
    pub data_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub configured: bool,
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    pub message: String,
}

#[tauri::command]
fn get_app_info(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<AppInfo, String> {
    let st = lock_state(&state)?;
    let path = store_path(&app, &st.config)?;
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        account_count: st.accounts.len(),
        data_path: path.parent().unwrap_or(&path).display().to_string(),
    })
}

#[tauri::command]
async fn check_for_updates(
    app: tauri::AppHandle,
    pending: tauri::State<'_, PendingUpdate>,
) -> Result<UpdateCheckResult, String> {
    {
        let mut pending = pending
            .0
            .lock()
            .map_err(|_| "Pending update lock poisoned".to_string())?;
        *pending = None;
    }

    let Some(pubkey) = updater_pubkey() else {
        return Ok(UpdateCheckResult {
            configured: false,
            available: false,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            latest_version: None,
            notes: None,
            published_at: None,
            message: "Automatic updates are not configured in this build".into(),
        });
    };

    let endpoint = updater_endpoint();
    let updater = app
        .updater_builder()
        .pubkey(pubkey)
        .endpoints(vec![endpoint.parse().map_err(|e| {
            format!("Invalid updater endpoint '{endpoint}': {e}")
        })?])
        .map_err(|e| format!("Failed to configure updater endpoint: {e}"))?
        .build()
        .map_err(|e| format!("Failed to build updater: {e}"))?;

    if let Some(update) = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for updates: {e}"))?
    {
        let result = UpdateCheckResult {
            configured: true,
            available: true,
            current_version: update.current_version.clone(),
            latest_version: Some(update.version.clone()),
            notes: update.body.clone(),
            published_at: update.date.as_ref().map(ToString::to_string),
            message: format!("Update available: v{}", update.version),
        };
        let mut pending = pending
            .0
            .lock()
            .map_err(|_| "Pending update lock poisoned".to_string())?;
        *pending = Some(update);
        Ok(result)
    } else {
        Ok(UpdateCheckResult {
            configured: true,
            available: false,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            latest_version: None,
            notes: None,
            published_at: None,
            message: format!(
                "You're on the latest version ({})",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }
}

#[tauri::command]
async fn install_update(
    app: tauri::AppHandle,
    pending: tauri::State<'_, PendingUpdate>,
) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    let _ = &app;

    let update = {
        let mut pending = pending
            .0
            .lock()
            .map_err(|_| "Pending update lock poisoned".to_string())?;
        pending
            .take()
            .ok_or_else(|| "No pending update. Check for updates first.".to_string())?
    };

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("Failed to download and install update: {e}"))?;

    #[cfg(not(target_os = "windows"))]
    app.restart();

    #[cfg(target_os = "windows")]
    return Ok("Update installed. Windows will close the app to finish installation.".into());

    #[cfg(not(target_os = "windows"))]
    Ok("Update installed. Restarting app...".into())
}

#[tauri::command]
fn open_data_folder(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let st = lock_state(&state)?;
    let path = store_path(&app, &st.config)?;
    let dir = path.parent().unwrap_or(&path);

    #[cfg(target_os = "windows")]
    std::process::Command::new("explorer")
        .arg(dir.to_string_lossy().to_string())
        .spawn()
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(dir)
        .spawn()
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open")
        .arg(dir)
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn change_data_path(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    new_path: String,
) -> Result<String, String> {
    let mut st = lock_state(&state)?;
    let new_dir = PathBuf::from(&new_path);
    fs::create_dir_all(&new_dir).map_err(|e| format!("Cannot create directory: {e}"))?;

    let old_file = store_path(&app, &st.config)?;
    let new_file = new_dir.join("accounts.json");

    if old_file.exists() && old_file != new_file {
        fs::copy(&old_file, &new_file).map_err(|e| format!("Failed to copy data: {e}"))?;
        fs::remove_file(&old_file).ok();
    }

    st.config.custom_data_path = Some(new_path);
    save_config(&app, &st.config)?;

    Ok(new_file.parent().unwrap_or(&new_file).display().to_string())
}

/// Export accounts to a text file (name:password:secret or name:secret per line)
#[tauri::command]
fn export_accounts(state: tauri::State<'_, Mutex<AppState>>, path: String) -> Result<u32, String> {
    let st = lock_state(&state)?;
    let mut lines = Vec::new();
    for acc in &st.accounts {
        if acc.secret.trim().is_empty() {
            lines.push(format!("{}:{}", acc.name, acc.password));
        } else if acc.password.trim().is_empty() {
            lines.push(format!("{}:{}", acc.name, acc.secret));
        } else {
            lines.push(format!("{}:{}:{}", acc.name, acc.password, acc.secret));
        }
    }
    let content = lines.join("\n");
    fs::write(&path, content).map_err(|e| format!("Failed to write export: {e}"))?;
    Ok(st.accounts.len() as u32)
}

/// Read a text file — restricted to .txt and .csv extensions
#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let p = PathBuf::from(&path);
    match p.extension().and_then(|e| e.to_str()) {
        Some("txt" | "csv") => {}
        _ => return Err("Only .txt and .csv files are supported".into()),
    }
    fs::read_to_string(&path).map_err(|e| format!("Cannot read file: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let config = load_config(app.handle());
            let accounts = load(app.handle(), &config);
            app.manage(Mutex::new(AppState { accounts, config }));
            app.manage(PendingUpdate::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_accounts,
            add_account,
            remove_account,
            edit_account,
            get_account_details,
            reorder_accounts,
            bulk_import,
            get_app_info,
            check_for_updates,
            install_update,
            open_data_folder,
            change_data_path,
            read_text_file,
            export_accounts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
