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

/// Jira URL の検証を一箇所に集約する。
///
/// セッション独立の SSB としての信頼境界を守るため、設定する URL は
/// **https かつ `*.atlassian.net`** に限定する（非 HTTPS の MITM や、
/// Jira ブラウザの体裁で攻撃者ページを開かせるのを防ぐ）。SSO 中の
/// `id.atlassian.com` 等への遷移は webview のナビゲーションで起きるもので、
/// 設定値の検証対象ではない。
fn check_https_atlassian(url: &tauri::Url) -> Result<(), String> {
    if url.scheme() != "https" {
        return Err("Jira URL は https である必要があります".into());
    }
    match url.host_str() {
        Some(h) if h == "atlassian.net" || h.ends_with(".atlassian.net") => Ok(()),
        _ => Err("Jira URL のホストは *.atlassian.net である必要があります".into()),
    }
}

/// 保存時の検証。空（未設定）は許可し、入力があれば https + `*.atlassian.net` を要求する。
pub fn validate_jira_url(raw: &str) -> Result<(), String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(());
    }
    let url = tauri::Url::parse(t).map_err(|e| format!("Jira URL が不正です: {e}"))?;
    check_https_atlassian(&url)
}

/// オープン時の解決。空はエラー。https + `*.atlassian.net` を満たした `Url` を返す。
pub fn require_jira_url(raw: &str) -> Result<tauri::Url, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Err("Jira URL が設定されていません".into());
    }
    let url = tauri::Url::parse(t).map_err(|e| format!("Jira URL が不正です: {e}"))?;
    check_https_atlassian(&url)?;
    Ok(url)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_allows_empty() {
        // 空（未設定）は保存可（起動時に設定ウィンドウを出す分岐に乗る）。
        assert!(validate_jira_url("").is_ok());
        assert!(validate_jira_url("   ").is_ok());
    }

    #[test]
    fn validate_accepts_https_atlassian() {
        assert!(validate_jira_url("https://example.atlassian.net").is_ok());
        // 前後の空白は trim される。
        assert!(validate_jira_url("  https://example.atlassian.net/jira/boards  ").is_ok());
        // apex ドメインそのものも許可する。
        assert!(validate_jira_url("https://atlassian.net").is_ok());
    }

    #[test]
    fn validate_rejects_non_https() {
        assert!(validate_jira_url("http://example.atlassian.net").is_err());
    }

    #[test]
    fn validate_rejects_non_atlassian_host() {
        // 体裁を似せた攻撃者ドメインを弾く（接尾辞の取り違えも含む）。
        assert!(validate_jira_url("https://example.com").is_err());
        assert!(validate_jira_url("https://atlassian.net.evil.com").is_err());
        assert!(validate_jira_url("https://evilatlassian.net").is_err());
    }

    #[test]
    fn require_rejects_empty() {
        // オープン時は未設定をエラーにする。
        assert!(require_jira_url("").is_err());
        assert!(require_jira_url("   ").is_err());
    }

    #[test]
    fn require_returns_parsed_url() {
        let url = require_jira_url("https://example.atlassian.net/path").expect("should parse");
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("example.atlassian.net"));
    }

    #[test]
    fn default_settings_are_sane() {
        let d = Settings::default();
        assert!(d.jira_url.is_empty());
        assert!(d.auto_reload_enabled);
        assert!(d.idle_threshold_secs >= 5);
        assert!(d.reload_check_interval_secs >= 5);
    }
}
