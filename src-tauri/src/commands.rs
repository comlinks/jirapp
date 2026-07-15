use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::jira;
use crate::settings::{self, Settings};
use crate::AppState;

/// 現在の設定を取得する。
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.snapshot()
}

/// 設定を保存する（store へ永続化し、メモリ上の状態も更新）。
/// Jira URL が入力されている場合は https + `*.atlassian.net` を満たすことを検証する。
#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<(), String> {
    settings::validate_jira_url(&settings.jira_url)?;
    // ホーム URL が変わったら、古い復元先（前回 URL）は破棄する。
    // 新しく設定したボードではなく前回のボードに戻ってしまうのを防ぐ。
    let prev = state.snapshot();
    if prev.jira_url.trim() != settings.jira_url.trim() {
        settings::clear_last_url(&app);
    }
    settings::persist_settings(&app, &settings)?;
    state.replace(settings);
    Ok(())
}

/// Jira ウィンドウを開く（既に開いていればフォーカス）。開けたら設定ウィンドウは隠す。
///
/// `async fn` にすることで、このコマンドはメインスレッドではなく非同期ランタイム上で
/// 実行される。メインスレッドのイベントループを解放した状態で `jira::open` 内の
/// `run_on_main_thread` から WebView2 生成をスケジュールするため、build() がハングせず
/// 白画面化しない（同期コマンドのままだとメインスレッドを塞いで生成が完了しない）。
#[tauri::command]
pub async fn open_jira_window(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let s = state.snapshot();
    jira::open(&app, &s)?;
    // Jira を開いた時点で設定ウィンドウは非表示にする。
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    Ok(())
}

/// 現在設定を開いている Jira ウィンドウへライブ適用する（CSS / アイドル閾値）。
/// ユーザー JS の変更はウィンドウ再オープン時に反映される（initialization_script のため）。
#[tauri::command]
pub fn apply_to_jira_window(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let s = state.snapshot();
    jira::apply(&app, &s)
}

/// 設定ウィンドウを隠す（フロントの「設定を閉じる」導線から呼ぶ）。
#[tauri::command]
pub fn hide_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(main) = app.get_webview_window("main") {
        main.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 設定ウィンドウを閉じる（フロントの「キャンセル」導線から呼ぶ）。
/// 設定ウィンドウ ✕ と同じ挙動: Jira が開いていれば隠すだけ、無ければアプリ終了。
#[tauri::command]
pub fn close_settings_window(app: AppHandle) -> Result<(), String> {
    if app.get_webview_window(jira::JIRA_LABEL).is_some() {
        if let Some(main) = app.get_webview_window("main") {
            main.hide().map_err(|e| e.to_string())?;
        }
    } else {
        // Jira が無ければ最後のウィンドウなのでアプリを終了する。
        app.exit(0);
    }
    Ok(())
}

/// 既定ブラウザで URL を開く（設定画面の GitHub リンク等）。
///
/// セキュリティ: http/https のスキームのみ許可し、`explorer.exe` に URL を
/// **引数として**渡す（シェルを介さずインジェクションを避ける）。任意プロトコルや
/// `file:` 等は弾く。
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let parsed = tauri::Url::parse(&url).map_err(|e| format!("URL が不正です: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("http/https の URL のみ開けます".into());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg(parsed.as_str())
            .spawn()
            .map_err(|e| format!("URL を開けません: {e}"))?;
    }
    Ok(())
}

/// Jira ウィンドウが開いているか。フロントのボタン表示切替（Jiraを開く⇄設定を閉じる）に使う。
#[tauri::command]
pub fn is_jira_open(app: AppHandle) -> bool {
    app.get_webview_window(jira::JIRA_LABEL).is_some()
}

/// 設定ウィンドウ(main)の高さを、フロントが測った実コンテンツ高(CSS px)に合わせる。
/// 詳細設定の折り畳み開閉やバナー表示などで「丁度良い」高さに自動調整するために使う。
/// 幅は現在値を維持し、高さのみ論理サイズで設定する。
#[tauri::command]
pub fn set_settings_height(app: AppHandle, height: f64) -> Result<(), String> {
    if let Some(main) = app.get_webview_window("main") {
        // 最大化中はユーザーの意図を尊重して触らない。
        if main.is_maximized().unwrap_or(false) {
            return Ok(());
        }
        let scale = main.scale_factor().map_err(|e| e.to_string())?;
        let cur = main.inner_size().map_err(|e| e.to_string())?;
        // 幅は現在の見た目（論理px）を維持する。
        let logical_w = cur.width as f64 / scale;
        // 下限を設けて潰れすぎを防ぐ（tauri.conf.json の minHeight と整合）。
        let h = height.max(240.0);
        main.set_size(tauri::LogicalSize::new(logical_w, h))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 設定ウィンドウを表示してフォーカスする（イベント通知はしない）。
pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
}

/// 設定ウィンドウを表示し、フロントへ状態更新を通知する。
/// Jira ウィンドウのシステムメニュー（設定を開く導線）から呼ぶ。
pub fn reveal_settings<R: Runtime>(app: &AppHandle<R>) {
    show_main(app);
    // フロントにボタン表示（設定を閉じる）へ切替えさせる。
    let _ = app.emit("settings:refresh", ());
}

/// 開いている Jira ウィンドウを再読み込みする（システムメニュー「再読み込み」から呼ぶ）。
/// リモートコンテンツに IPC を与えない方針を保つため、ホストからの `eval` で
/// `location.reload()` を実行する（アイドル自動リロード・F5 と同じ経路）。
pub fn reload_jira<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window(jira::JIRA_LABEL) {
        let _ = win.eval("window.location.reload()");
    }
}
