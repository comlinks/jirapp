mod commands;
mod jira;
mod settings;

use std::sync::Mutex;

use tauri::{Manager, WindowEvent};

use settings::Settings;

/// メモリ上の設定状態。store と同期させる single source of truth のキャッシュ。
pub struct AppState(pub Mutex<Settings>);

impl AppState {
    /// 現在の設定のスナップショットを取得する。
    /// ロックがポイズニングしていても回復して読み出す（1 箇所の panic で
    /// 設定系コマンド全体が連鎖 panic するのを防ぐ）。
    pub fn snapshot(&self) -> Settings {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 設定を置き換える（ポイズニング回復つき）。
    pub fn replace(&self, settings: Settings) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = settings;
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // セッション独立: WebView2 のユーザーデータフォルダをアプリ専用パスに固定する。
    // 設定ウィンドウ(main)・Jira ウィンドウ(jira) すべての webview で同一フォルダを共有し、
    // システムの Edge/Chrome とは Cookie・認証状態を分離する。
    // 注意: 1 プロセス内で webview ごとに異なる UDF は使えない（WebView2 の制約）ため、
    // 個別ウィンドウの data_directory ではなく、起動時の環境変数で一括指定する。
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let dir = std::path::Path::new(&local)
            .join("com.kanfu.jirapp")
            .join("webview-data");
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &dir);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        // セルフアップデート（更新確認・ダウンロード）と適用後の再起動。
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Jira ウィンドウの位置・サイズ・最大化状態を保存／復元する。
        // 設定(main)ウィンドウは固定サイズ・最大化禁止のため状態管理から除外する。
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&["main"])
                .build(),
        )
        .setup(|app| {
            // 起動時に store から設定を読み込み、状態として管理する。
            let initial = settings::load_settings(app.handle());
            app.manage(AppState(Mutex::new(initial.clone())));

            let handle = app.handle().clone();

            // 設定(main)ウィンドウのクローズ挙動:
            //  - Jira が開いていれば閉じずに隠す（＝「設定を閉じる」と同じ扱い）。
            //  - Jira が無ければそのまま閉じる → 最後のウィンドウなのでアプリ終了。
            if let Some(main) = handle.get_webview_window("main") {
                let h = handle.clone();
                main.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        if h.get_webview_window(jira::JIRA_LABEL).is_some() {
                            api.prevent_close();
                            if let Some(m) = h.get_webview_window("main") {
                                let _ = m.hide();
                            }
                        }
                    }
                });
            }

            // 起動時の分岐:
            //  - URL 未設定 → 設定ウィンドウを表示（ユーザーに URL を入力させる）。
            //  - URL 設定済み → 保存済み設定で Jira を自動オープン（設定ウィンドウは非表示のまま）。
            if initial.jira_url.trim().is_empty() {
                commands::show_main(&handle);
            } else if let Err(e) = jira::build_jira_window(&handle, &initial) {
                // 自動オープンに失敗（URL 不正など）したら設定ウィンドウを出して修正させる。
                eprintln!("[jirapp] Jira 自動オープンに失敗: {e}. 設定ウィンドウを表示します");
                commands::show_main(&handle);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::open_jira_window,
            commands::apply_to_jira_window,
            commands::hide_settings_window,
            commands::close_settings_window,
            commands::open_url,
            commands::is_jira_open,
            commands::set_settings_height,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
