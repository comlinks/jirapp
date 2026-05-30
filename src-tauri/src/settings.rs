use serde::{Deserialize, Serialize};

/// アプリ設定。tauri-plugin-store が single source of truth。
/// フロント（TS）の `Settings` 型と camelCase で対応させる。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// 表示する Jira Cloud の URL（例: https://your-domain.atlassian.net）
    pub jira_url: String,
    /// ユーザーが注入する任意の JS
    pub custom_js: String,
    /// ユーザーが注入する任意の CSS
    pub custom_css: String,
    /// アイドル時の自動リロードを有効にするか
    pub auto_reload_enabled: bool,
    /// アイドルと判定するまでの秒数
    pub idle_threshold_secs: u64,
    /// アイドル判定をチェックする間隔（秒）
    pub reload_check_interval_secs: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            jira_url: String::new(),
            custom_js: String::new(),
            custom_css: String::new(),
            auto_reload_enabled: true,
            idle_threshold_secs: 300,
            reload_check_interval_secs: 30,
        }
    }
}

/// store 上で設定を保存するキー。
pub const STORE_PATH: &str = "settings.json";
pub const SETTINGS_KEY: &str = "settings";

use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

/// store から設定を読み込む。未保存・破損時は既定値。
pub fn load_settings<R: Runtime>(app: &AppHandle<R>) -> Settings {
    match app.store(STORE_PATH) {
        Ok(store) => store
            .get(SETTINGS_KEY)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

/// store へ設定を永続化する。
pub fn persist_settings<R: Runtime>(app: &AppHandle<R>, s: &Settings) -> Result<(), String> {
    let store = app.store(STORE_PATH).map_err(|e| e.to_string())?;
    let value = serde_json::to_value(s).map_err(|e| e.to_string())?;
    store.set(SETTINGS_KEY, value);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}
