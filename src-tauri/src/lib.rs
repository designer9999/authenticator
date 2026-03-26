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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub issuer: String,
    pub name: String,
    pub secret: String,
    pub digits: u32,
    pub period: u32,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    custom_data_path: Option<String>,
}

pub struct AppState {
    pub accounts: Vec<Account>,
    pub config: AppConfig,
}

fn config_path(app: &tauri::AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().expect("no app data dir");
    fs::create_dir_all(&dir).ok();
    dir.join("config.json")
}

fn load_config(app: &tauri::AppHandle) -> AppConfig {
    let path = config_path(app);
    fs::read_to_string(&path)
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

fn save_config(app: &tauri::AppHandle, config: &AppConfig) {
    let path = config_path(app);
    if let Ok(json) = serde_json::to_string_pretty(config) {
        fs::write(path, json).ok();
    }
}

fn store_path(app: &tauri::AppHandle, config: &AppConfig) -> PathBuf {
    if let Some(ref custom) = config.custom_data_path {
        let p = PathBuf::from(custom);
        if p.exists() || fs::create_dir_all(&p).is_ok() {
            return p.join("accounts.json");
        }
    }
    let dir = app.path().app_data_dir().expect("no app data dir");
    fs::create_dir_all(&dir).ok();
    dir.join("accounts.json")
}

fn load(app: &tauri::AppHandle, config: &AppConfig) -> Vec<Account> {
    let path = store_path(app, config);
    fs::read_to_string(&path)
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

fn save(app: &tauri::AppHandle, config: &AppConfig, accounts: &[Account]) {
    let path = store_path(app, config);
    if let Ok(json) = serde_json::to_string_pretty(accounts) {
        fs::write(path, json).ok();
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn totp(secret_b32: &str, digits: u32, period: u32) -> Result<String, String> {
    let clean: String = secret_b32
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();

    let key = BASE32_NOPAD
        .decode(clean.as_bytes())
        .map_err(|e| format!("Bad base32: {e}"))?;

    let step = (now_secs() / period as u64).to_be_bytes();

    let mut mac =
        HmacSha1::new_from_slice(&key).map_err(|e| format!("HMAC error: {e}"))?;
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

#[tauri::command]
fn get_accounts(state: tauri::State<'_, Mutex<AppState>>) -> Vec<AccountCode> {
    let st = state.lock().unwrap();
    st.accounts
        .iter()
        .map(|a| AccountCode {
            id: a.id.clone(),
            issuer: a.issuer.clone(),
            name: a.name.clone(),
            code: totp(&a.secret, a.digits, a.period).unwrap_or_else(|_| "------".into()),
            digits: a.digits,
            period: a.period,
            remaining: a.period - (now_secs() % a.period as u64) as u32,
            created_at: a.id.parse::<i64>().unwrap_or(0),
        })
        .collect()
}

#[tauri::command]
fn add_account(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    issuer: String,
    name: String,
    secret: String,
) -> Result<(), String> {
    let clean: String = secret
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();

    BASE32_NOPAD
        .decode(clean.as_bytes())
        .map_err(|e| format!("Invalid secret: {e}"))?;

    let account = Account {
        id: format!("{}", chrono::Utc::now().timestamp_millis()),
        issuer,
        name,
        secret: clean,
        digits: 6,
        period: 30,
    };

    let mut st = state.lock().unwrap();
    st.accounts.push(account);
    save(&app, &st.config, &st.accounts);
    Ok(())
}

#[tauri::command]
fn remove_account(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), String> {
    let mut st = state.lock().unwrap();
    st.accounts.retain(|a| a.id != id);
    save(&app, &st.config, &st.accounts);
    Ok(())
}

/// Bulk import: each line is "name:password:secret" or "name:secret"
#[tauri::command]
fn bulk_import(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    text: String,
) -> Result<u32, String> {
    let mut st = state.lock().unwrap();
    let mut count = 0u32;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split(':').collect();
        let (name, secret_raw) = if parts.len() >= 3 {
            (parts[0].trim(), parts[2..].join(""))
        } else if parts.len() == 2 {
            (parts[0].trim(), parts[1].to_string())
        } else {
            continue;
        };
        let clean: String = secret_raw.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
        if clean.is_empty() { continue; }
        if BASE32_NOPAD.decode(clean.as_bytes()).is_err() { continue; }
        let account = Account {
            id: format!("{}", chrono::Utc::now().timestamp_millis() + count as i64),
            issuer: name.chars().next().unwrap_or('?').to_uppercase().to_string(),
            name: name.to_string(),
            secret: clean,
            digits: 6,
            period: 30,
        };
        st.accounts.push(account);
        count += 1;
    }
    if count > 0 {
        save(&app, &st.config, &st.accounts);
    }
    Ok(count)
}

#[tauri::command]
fn edit_account(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    id: String,
    issuer: String,
    name: String,
    secret: Option<String>,
) -> Result<(), String> {
    let mut st = state.lock().unwrap();
    if let Some(acc) = st.accounts.iter_mut().find(|a| a.id == id) {
        acc.issuer = issuer;
        acc.name = name;
        if let Some(s) = secret {
            let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
            if !clean.is_empty() {
                BASE32_NOPAD.decode(clean.as_bytes()).map_err(|e| format!("Invalid secret: {e}"))?;
                acc.secret = clean;
            }
        }
        save(&app, &st.config, &st.accounts);
        Ok(())
    } else {
        Err("Account not found".into())
    }
}

#[tauri::command]
fn reorder_accounts(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    ids: Vec<String>,
) -> Result<(), String> {
    let mut st = state.lock().unwrap();
    let mut reordered = Vec::with_capacity(ids.len());
    for id in &ids {
        if let Some(acc) = st.accounts.iter().find(|a| &a.id == id) {
            reordered.push(acc.clone());
        }
    }
    st.accounts = reordered;
    save(&app, &st.config, &st.accounts);
    Ok(())
}

#[tauri::command]
fn get_remaining() -> u32 {
    30 - (now_secs() % 30) as u32
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
) -> AppInfo {
    let st = state.lock().unwrap();
    let path = store_path(&app, &st.config);
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        account_count: st.accounts.len(),
        data_path: path.parent().unwrap_or(&path).display().to_string(),
    }
}

#[tauri::command]
fn open_data_folder(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let st = state.lock().unwrap();
    let path = store_path(&app, &st.config);
    let dir = path.parent().unwrap_or(&path);
    fs::create_dir_all(dir).ok();

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
fn read_text_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("Cannot read file: {e}"))
}

/// Change data storage path: moves existing accounts.json to new location
#[tauri::command]
fn change_data_path(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    new_path: String,
) -> Result<String, String> {
    let mut st = state.lock().unwrap();

    let new_dir = PathBuf::from(&new_path);
    fs::create_dir_all(&new_dir).map_err(|e| format!("Cannot create directory: {e}"))?;

    // Current file location
    let old_file = store_path(&app, &st.config);
    let new_file = new_dir.join("accounts.json");

    // Move existing file if it exists and destinations differ
    if old_file.exists() && old_file != new_file {
        fs::copy(&old_file, &new_file)
            .map_err(|e| format!("Failed to copy data: {e}"))?;
        fs::remove_file(&old_file).ok(); // best effort cleanup
    }

    // Save config
    st.config.custom_data_path = Some(new_path);
    save_config(&app, &st.config);

    Ok(new_file.parent().unwrap_or(&new_file).display().to_string())
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
            reorder_accounts,
            get_remaining,
            get_app_info,
            open_data_folder,
            change_data_path,
            bulk_import,
            read_text_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
