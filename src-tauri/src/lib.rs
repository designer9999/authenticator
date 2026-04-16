use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

type HmacSha1 = Hmac<Sha1>;

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
    pub code: String,
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
    format!("{}{:04}", chrono::Utc::now().timestamp_millis(), rand::random::<u16>() % 10000)
}

fn default_import_issuer() -> String {
    "Google".into()
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

fn lock_state<'a>(state: &'a tauri::State<'_, Mutex<AppState>>) -> Result<std::sync::MutexGuard<'a, AppState>, String> {
    state.lock().map_err(|_| "State lock poisoned".to_string())
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
    Ok(st.accounts.iter().map(|a| {
        let period = a.period as u64;
        AccountCode {
            id: a.id.clone(),
            issuer: a.issuer.clone(),
            name: a.name.clone(),
            code: totp(&a.secret, a.digits, a.period).unwrap_or_else(|_| "------".into()),
            digits: a.digits,
            period: a.period,
            remaining: (a.period - (now % period) as u32),
            created_at: a.id[..13.min(a.id.len())].parse::<i64>().unwrap_or(0),
        }
    }).collect())
}

#[tauri::command]
fn add_account(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    issuer: String,
    name: String,
    password: Option<String>,
    secret: String,
) -> Result<(), String> {
    let clean = clean_secret(&secret)?;
    let account = Account {
        id: unique_id(),
        issuer,
        name,
        password: password.unwrap_or_default(),
        secret: clean,
        digits: 6,
        period: 30,
    };
    let mut st = lock_state(&state)?;
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
    let acc = st.accounts.iter_mut().find(|a| a.id == id).ok_or("Account not found")?;
    acc.issuer = issuer;
    acc.name = name;
    if let Some(p) = password {
        acc.password = p;
    }
    if let Some(s) = secret {
        if !s.trim().is_empty() {
            acc.secret = clean_secret(&s)?;
        }
    }
    save(&app, &st.config, &st.accounts)
}

#[tauri::command]
fn get_account_details(
    state: tauri::State<'_, Mutex<AppState>>,
    id: String,
) -> Result<AccountDetails, String> {
    let st = lock_state(&state)?;
    let acc = st.accounts.iter().find(|a| a.id == id).ok_or("Account not found")?;
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

/// Bulk import: each line is "name:password:secret" or "name:secret"
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
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split(':').collect();
        let (name, password, secret_raw) = if parts.len() >= 3 {
            (
                parts[0].trim().to_string(),
                parts[1..parts.len() - 1].join(":").trim().to_string(),
                parts[parts.len() - 1].trim().to_string(),
            )
        } else if parts.len() == 2 {
            (
                parts[0].trim().to_string(),
                String::new(),
                parts[1].trim().to_string(),
            )
        } else {
            continue;
        };
        let clean = match clean_secret(&secret_raw) {
            Ok(c) => c,
            Err(_) => continue,
        };
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
fn export_accounts(
    state: tauri::State<'_, Mutex<AppState>>,
    path: String,
) -> Result<u32, String> {
    let st = lock_state(&state)?;
    let mut lines = Vec::new();
    for acc in &st.accounts {
        if acc.password.trim().is_empty() {
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
        Some("txt" | "csv") => {},
        _ => return Err("Only .txt and .csv files are supported".into()),
    }
    fs::read_to_string(&path).map_err(|e| format!("Cannot read file: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config = load_config(app.handle());
            let accounts = load(app.handle(), &config);
            app.manage(Mutex::new(AppState { accounts, config }));
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
            open_data_folder,
            change_data_path,
            read_text_file,
            export_accounts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
