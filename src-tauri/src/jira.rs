use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::settings::{self, Settings};
use crate::AppState;

/// Jira ウィンドウのラベル。
pub const JIRA_LABEL: &str = "jira";

// セッション独立（WebView2 のユーザーデータフォルダ分離）は lib.rs の起動時に
// 環境変数 WEBVIEW2_USER_DATA_FOLDER で全 webview 共通に設定している。
// 重要: 1 プロセス内で webview ごとに異なる data_directory を指定すると、
// WebView2 の制約により 2 つ目の webview 生成が失敗し白画面になる。

/// Jira ウィンドウを開く。既に開いていればフォーカスして現在設定を再適用する。
///
/// 重要（Windows / WebView2 のメインスレッドブロック対策）:
/// 同期 `#[tauri::command]` はメインスレッドで実行されイベントループを止める。
/// WebView2 の生成はメインスレッドのメッセージループが回ることを要するため、
/// ループを止めたまま `build()` を呼ぶと webview 生成が完了せず白画面になる。
/// そこで実際の生成は `run_on_main_thread` でイベントループ上にスケジュールし、
/// 呼び出し元（async コマンド）はワーカースレッドで結果を待つ。
pub fn open<R: Runtime>(app: &AppHandle<R>, s: &Settings) -> Result<(), String> {
    // URL の検証（空・スキーム・ホスト）はここ（呼び出し元スレッド）で済ませ、不正なら即座に返す。
    settings::require_jira_url(&s.jira_url)?;

    // 既存ウィンドウがあればフォーカス＋ライブ適用のみ。
    if let Some(win) = app.get_webview_window(JIRA_LABEL) {
        let _ = win.set_focus();
        apply(app, s)?;
        return Ok(());
    }

    // 生成はメインスレッド（イベントループ）上で行う。結果はチャネルで受け取り、
    // build() のエラーをフロントへ伝播できるようにする。
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let app_for_build = app.clone();
    let settings_for_build = s.clone();
    app.run_on_main_thread(move || {
        // ユーザー操作による明示オープンはホーム（設定の jira_url）を開く。
        let url = settings_for_build.jira_url.clone();
        let _ = tx.send(build_jira_window(&app_for_build, &settings_for_build, &url));
    })
    .map_err(|e| format!("メインスレッドへのスケジュールに失敗: {e}"))?;

    // メインスレッドは（async コマンド化により）解放されているのでループが回り、
    // スケジュールしたクロージャが実行されて build() が完了する。
    rx.recv()
        .map_err(|e| format!("Jira ウィンドウ生成結果の受信に失敗: {e}"))?
}

/// 実際の Jira ウィンドウ生成。必ずメインスレッド上で呼ぶこと。
/// 起動時の自動オープン（lib.rs の setup）からも直接呼ぶため crate 公開。
///
/// `open_url` は実際に読み込む URL。通常は設定の `s.jira_url`（ホーム）だが、
/// 起動時は前回終了時の URL を復元するため別 URL が渡されうる。設定値（CSS/JS/
/// アイドル閾値）は `s` から取る。
pub(crate) fn build_jira_window<R: Runtime>(
    app: &AppHandle<R>,
    s: &Settings,
    open_url: &str,
) -> Result<(), String> {
    // 呼び出し前に検証済みのこともあるが、生成スレッド上でも同じ検証を通して Url を得る。
    // 復元 URL も含め、https + *.atlassian.net の境界をここで必ず担保する。
    let parsed = settings::require_jira_url(open_url)?;

    // 新規ウィンドウ判定で「現在の」登録ホストを参照するためのハンドル。
    // 設定で URL を変更した後（ウィンドウ再生成せず）でも最新の登録ドメインに追従させる。
    let app_for_newwin = app.clone();

    let mut builder = WebviewWindowBuilder::new(app, JIRA_LABEL, WebviewUrl::External(parsed))
        .title("Jira")
        // 初期サイズ（保存済み状態があれば後で復元して上書きする）。
        .inner_size(1280.0, 900.0)
        // 復元した位置・サイズを適用してから表示し、初期位置からのちらつきを防ぐ。
        .visible(false)
        // OS レベルの drag-drop ハンドラを無効化する。これを有効のままにすると
        // WebView 内の HTML5 ドラッグ&ドロップ（Jira ボードのカード移動など）が
        // ネイティブ側に横取りされて動作しない（Windows の WebView2 で必須）。
        .disable_drag_drop_handler()
        // 新規ウィンドウ要求（target=_blank / window.open）の扱い。
        //  - 同じ atlassian.net テナント（Confluence 等）→ Allow。WebView2 が
        //    同一環境（＝同一 UDF/セッション）で別ウィンドウのポップアップを開く。
        //    Tauri 管理外の素の WebView2 ウィンドウなので IPC は一切渡らない（境界維持）。
        //  - それ以外 → Deny（従来どおり抑制）。SSO 等のポップアップを外部ブラウザへ
        //    逃がして壊さないよう、現状の挙動を保つ。
        // 既定（ハンドラ未設定）では new-window 要求は wry に抑制され「リンクが効かない」。
        .on_new_window(move |url, _features| {
            if registered_host(&app_for_newwin).is_some_and(|host| is_same_tenant_url(&url, &host))
            {
                NewWindowResponse::Allow
            } else {
                NewWindowResponse::Deny
            }
        });

    // 注入機能（基盤プラットフォーム＋各機能）を document-start でネイティブ注入する。
    // 登録順＝注入順。先頭の MACHINERY_JS が window.JIRAPP を用意し、以降の機能がそれに乗る。
    // 機能追加は inject::DOC_START_SCRIPTS に 1 行足すだけでよい。
    for script in crate::inject::DOC_START_SCRIPTS {
        builder = builder.initialization_script(*script);
    }

    // ユーザー JS は基盤・各機能の後に注入する（CSP 非対象のネイティブ注入）。
    // 構文エラーがあってもこの script 内に閉じ、基盤処理は壊さない。
    if !s.custom_js.trim().is_empty() {
        builder = builder.initialization_script(crate::inject::user_js_wrapper(&s.custom_js));
    }

    // ページロード完了ごとに「現在の」設定（CSS・閾値）を反映する。
    // SPA のナビゲーション後やライブ保存後も最新状態を維持できる。
    let app_for_load = app.clone();
    builder = builder.on_page_load(move |webview, payload| {
        // 切り分け用ログ: ナビゲーションが Started/Finished のどこまで進んだか。
        eprintln!(
            "[jirapp] page_load event={:?} url={}",
            payload.event(),
            payload.url()
        );
        if !matches!(payload.event(), PageLoadEvent::Finished) {
            return;
        }
        if let Some(state) = app_for_load.try_state::<AppState>() {
            let current = state.snapshot();
            let _ = webview.eval(crate::inject::push_config_script(&current));
        }
    });

    eprintln!("[jirapp] building jira window for url={}", open_url.trim());
    let window = builder.build().map_err(|e| e.to_string())?;
    eprintln!("[jirapp] jira window built ok");

    // 前回の位置・サイズ（最大化状態を含む）を復元してから表示する。
    // 状態が未保存の初回は何もしない（既定の中央・初期サイズのまま）。
    {
        use tauri_plugin_window_state::{StateFlags, WindowExt};
        let _ =
            window.restore_state(StateFlags::POSITION | StateFlags::SIZE | StateFlags::MAXIMIZED);
    }
    let _ = window.show();

    // 設定を開く導線は、タイトルバー左上アイコンのシステムメニューに追加する。
    // リモートコンテンツに Tauri API を与えずに済むよう、Win32 のシステムメニュー＋
    // ウィンドウサブクラスで WM_SYSCOMMAND を拾って実装する（IPC を使わない）。
    #[cfg(windows)]
    sysmenu::install(&window, app);

    // クローズ挙動:
    //  - 設定ウィンドウが非表示のまま Jira を閉じたらアプリ終了。
    //  - 設定ウィンドウが表示中なら Jira だけ閉じ、フロントへ状態更新（ボタン表示の切替）を通知。
    let app_for_close = app.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { .. } => {
            // 次回起動時の復元用に、閉じる直前の表示 URL を保存する。
            // フィルターは SPA の pushState で URL に載る（フルロードを伴わない）が、
            // WebView2 の Source はそれにも追従するため webview.url() で現在値を拾える。
            if let Some(win) = app_for_close.get_webview_window(JIRA_LABEL) {
                if let Ok(url) = win.url() {
                    let _ = settings::persist_last_url(&app_for_close, url.as_str());
                }
            }
            let settings_visible = app_for_close
                .get_webview_window("main")
                .and_then(|w| w.is_visible().ok())
                .unwrap_or(false);
            if !settings_visible {
                app_for_close.exit(0);
            }
        }
        WindowEvent::Destroyed => {
            let _ = app_for_close.emit("settings:refresh", ());
        }
        _ => {}
    });

    // 表示中の URL を随時 `lastUrl` として保存する（issue #24）。
    // クローズ時保存だけだと、jirapp を終了せず Windows をシャットダウンした場合などに
    // 最新 URL を取りこぼす。フィルター変更は SPA の pushState で URL に載る（フルロードを
    // 伴わない）ため on_page_load では拾えず、バックグラウンドのポーリングで変化時に永続化する。
    spawn_last_url_poll(app);

    Ok(())
}

/// 表示中の Jira URL を定期的に監視し、変化したら `lastUrl` として保存する。
///
/// フィルター変更は SPA の pushState で URL に載る（フルロードを伴わない）ため
/// `on_page_load` では拾えず、`CloseRequested` の保存だけでは、アプリを終了せず OS を
/// シャットダウンした場合に取りこぼす（issue #24）。そこで軽量なポーリングで
/// **変化したときだけ** 永続化する（未変化なら store へ書き込まない）。
///
/// `webview.url()` は UI スレッド上でのみ安全に呼べるため、読み取りは
/// `run_on_main_thread` に載せる。ウィンドウが無くなったら（＝クローズ）監視を終える。
/// アプリ終了時はプロセスごと落ちるためスレッドの後始末は不要。
fn spawn_last_url_poll<R: Runtime>(app: &AppHandle<R>) {
    /// 監視間隔。変化時のみ書き込むため、この値による disk への負荷は実質ない。
    /// シャットダウン時の取りこぼし窓もこの程度に収まる。
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10);

    let app = app.clone();
    std::thread::spawn(move || {
        let mut last_saved: Option<String> = None;
        loop {
            std::thread::sleep(POLL_INTERVAL);

            // 現在の表示 URL を UI スレッドで読み取る。
            let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
            let app_for_read = app.clone();
            if app
                .run_on_main_thread(move || {
                    let url = app_for_read
                        .get_webview_window(JIRA_LABEL)
                        .and_then(|w| w.url().ok())
                        .map(|u| u.to_string());
                    let _ = tx.send(url);
                })
                .is_err()
            {
                break; // アプリ終了などでスケジュール不可 → 監視終了
            }

            match rx.recv() {
                Ok(Some(url)) => {
                    if last_saved.as_deref() != Some(url.as_str()) {
                        let _ = settings::persist_last_url(&app, &url);
                        last_saved = Some(url);
                    }
                }
                Ok(None) => break, // ウィンドウが無くなった（クローズ）→ 監視終了
                Err(_) => break,   // 送信側が落ちた → 監視終了
            }
        }
    });
}

/// Jira ウィンドウのシステムメニュー（タイトルバー左上アイコンのメニュー）連携。
///
/// Tauri のメニュー API はメニューバーとして表示されてしまうため、Windows の
/// システムメニューへ直接「設定を開く」を追加し、`WM_SYSCOMMAND` をウィンドウ
/// サブクラスで拾って `reveal_settings` を呼ぶ。リモートコンテンツ（Jira）には
/// Tauri API/IPC を一切与えないという方針を保ったまま導線を提供できる。
#[cfg(windows)]
mod sysmenu {
    use tauri::{AppHandle, Runtime};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, GetSystemMenu, MF_SEPARATOR, MF_STRING, WM_NCDESTROY, WM_SYSCOMMAND,
    };

    /// システムメニュー項目のコマンド ID。WM_SYSCOMMAND では下位 4bit が
    /// システム予約のため 0 にしておき、判定時に 0xFFF0 でマスクする。
    const IDM_OPEN_SETTINGS: usize = 0x0010;
    /// サブクラス識別子（このウィンドウに対して一意なら何でもよい）。
    const SUBCLASS_ID: usize = 1;

    /// 設定を開くコールバックの型。UI スレッド上でのみ使うので Send は不要。
    type Callback = Box<dyn Fn()>;

    /// Jira ウィンドウのシステムメニューに「設定を開く」を追加する。
    pub fn install<R: Runtime>(window: &tauri::WebviewWindow<R>, app: &AppHandle<R>) {
        let hwnd = match window.hwnd() {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[jirapp] hwnd 取得失敗、システムメニュー設定をスキップ: {e}");
                return;
            }
        };

        // クロージャを型消去してサブクラスの参照データ（usize）として持たせる。
        // ウィンドウ破棄（WM_NCDESTROY）時に回収して drop する。
        let app_for_cb = app.clone();
        let cb: Callback = Box::new(move || crate::commands::reveal_settings(&app_for_cb));
        let refdata = Box::into_raw(Box::new(cb)) as usize;

        unsafe {
            let hmenu = GetSystemMenu(hwnd, false);
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
            if let Err(e) = AppendMenuW(hmenu, MF_STRING, IDM_OPEN_SETTINGS, w!("設定を開く"))
            {
                eprintln!("[jirapp] システムメニュー項目の追加に失敗: {e}");
            }
            if SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, refdata) == false {
                eprintln!("[jirapp] ウィンドウサブクラス設定に失敗");
                // 失敗時はリークを避けて回収する。
                drop(Box::from_raw(refdata as *mut Callback));
            }
        }
    }

    /// サブクラスのウィンドウプロシージャ。WM_SYSCOMMAND で自前 ID を拾う。
    unsafe extern "system" fn subclass_proc(
        hwnd: HWND,
        umsg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _uid: usize,
        refdata: usize,
    ) -> LRESULT {
        match umsg {
            WM_SYSCOMMAND if (wparam.0 & 0xFFF0) == IDM_OPEN_SETTINGS => {
                if refdata != 0 {
                    let cb = &*(refdata as *const Callback);
                    cb();
                }
                return LRESULT(0);
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
                if refdata != 0 {
                    drop(Box::from_raw(refdata as *mut Callback));
                }
            }
            _ => {}
        }
        DefSubclassProc(hwnd, umsg, wparam, lparam)
    }
}

/// 開いている Jira ウィンドウへ現在設定をライブ適用する（CSS・アイドル閾値）。
/// ユーザー JS の変更はウィンドウ再オープン時に反映される。
pub fn apply<R: Runtime>(app: &AppHandle<R>, s: &Settings) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(JIRA_LABEL) {
        win.eval(crate::inject::push_config_script(s))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 起動時に自動オープンする URL を解決する。
///
/// 前回終了時に保存した URL（`lastUrl`）があり、それが登録 Jira と同一テナント
/// （https + 同一ホスト）なら、その URL を復元して「前回の続き」から開く。
/// フィルター（`?jql=...`）はこの URL に載るため、これで起動ごとのリセットを防ぐ。
/// 保存が無い・別テナント・不正なら、設定のホーム URL（`s.jira_url`）にフォールバックする。
pub(crate) fn resolve_startup_url<R: Runtime>(app: &AppHandle<R>, s: &Settings) -> String {
    if let Some(last) = settings::load_last_url(app) {
        if let (Ok(last_url), Some(host)) = (tauri::Url::parse(last.trim()), registered_host(app)) {
            if is_same_tenant_url(&last_url, &host) {
                return last;
            }
        }
    }
    s.jira_url.clone()
}

/// 新規ウィンドウ要求の URL が、登録した Jira と「同一ホスト（同一テナント）」への
/// https リンクかどうか。これに該当する場合のみポップアップで開かせる（同一セッション）。
/// `*.atlassian.net` 全体ではなく、登録ドメインと完全一致のものだけを対象にする。
fn is_same_tenant_url(target: &tauri::Url, registered_host: &str) -> bool {
    target.scheme() == "https" && target.host_str() == Some(registered_host)
}

/// 現在登録されている Jira URL のホスト名を取り出す（未設定・不正なら None）。
fn registered_host<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    let raw = app.try_state::<AppState>()?.snapshot().jira_url;
    let url = tauri::Url::parse(raw.trim()).ok()?;
    url.host_str().map(|h| h.to_string())
}

#[cfg(test)]
mod tests {
    use super::is_same_tenant_url;

    fn url(s: &str) -> tauri::Url {
        tauri::Url::parse(s).expect("valid url")
    }

    #[test]
    fn allows_only_same_registered_host() {
        let host = "example.atlassian.net";
        // 登録ドメインと同一ホスト（Jira / Confluence いずれのパスでも）→ 許可。
        assert!(is_same_tenant_url(
            &url("https://example.atlassian.net/jira/boards"),
            host
        ));
        assert!(is_same_tenant_url(
            &url("https://example.atlassian.net/wiki/spaces/X"),
            host
        ));
    }

    #[test]
    fn rejects_other_tenants_and_non_https() {
        let host = "example.atlassian.net";
        // 別テナント（同じ atlassian.net でもホストが違う）は拒否。
        assert!(!is_same_tenant_url(
            &url("https://other.atlassian.net/wiki"),
            host
        ));
        // http への降格は拒否。
        assert!(!is_same_tenant_url(
            &url("http://example.atlassian.net"),
            host
        ));
        // 別ドメイン（SSO 等）は拒否。
        assert!(!is_same_tenant_url(
            &url("https://id.atlassian.com/login"),
            host
        ));
        assert!(!is_same_tenant_url(&url("https://example.com"), host));
    }
}
