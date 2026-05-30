use tauri::webview::PageLoadEvent;
use tauri::{
    AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

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
        let _ = tx.send(build_jira_window(&app_for_build, &settings_for_build));
    })
    .map_err(|e| format!("メインスレッドへのスケジュールに失敗: {e}"))?;

    // メインスレッドは（async コマンド化により）解放されているのでループが回り、
    // スケジュールしたクロージャが実行されて build() が完了する。
    rx.recv()
        .map_err(|e| format!("Jira ウィンドウ生成結果の受信に失敗: {e}"))?
}

/// 実際の Jira ウィンドウ生成。必ずメインスレッド上で呼ぶこと。
/// 起動時の自動オープン（lib.rs の setup）からも直接呼ぶため crate 公開。
pub(crate) fn build_jira_window<R: Runtime>(app: &AppHandle<R>, s: &Settings) -> Result<(), String> {
    // 呼び出し前に検証済みのこともあるが、生成スレッド上でも同じ検証を通して Url を得る。
    let parsed = settings::require_jira_url(&s.jira_url)?;

    let mut builder = WebviewWindowBuilder::new(app, JIRA_LABEL, WebviewUrl::External(parsed))
        .title("Jira")
        // 初期サイズ（保存済み状態があれば後で復元して上書きする）。
        .inner_size(1280.0, 900.0)
        // 復元した位置・サイズを適用してから表示し、初期位置からのちらつきを防ぐ。
        .visible(false)
        // 基盤処理（アイドル検知・リロード・CSS 適用土台）はネイティブ注入。CSP の影響を受けにくい。
        .initialization_script(MACHINERY_JS);

    // ユーザー JS は別 initialization_script としてネイティブ注入（CSP 非対象）。
    // 構文エラーがあってもこの script 内に閉じ、基盤処理は壊さない。
    if !s.custom_js.trim().is_empty() {
        builder = builder.initialization_script(&user_js_wrapper(&s.custom_js));
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
            let _ = webview.eval(&push_config_script(&current));
        }
    });

    eprintln!("[jirapp] building jira window for url={}", s.jira_url.trim());
    let window = builder.build().map_err(|e| e.to_string())?;
    eprintln!("[jirapp] jira window built ok");

    // 前回の位置・サイズ（最大化状態を含む）を復元してから表示する。
    // 状態が未保存の初回は何もしない（既定の中央・初期サイズのまま）。
    {
        use tauri_plugin_window_state::{StateFlags, WindowExt};
        let _ = window
            .restore_state(StateFlags::POSITION | StateFlags::SIZE | StateFlags::MAXIMIZED);
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

    Ok(())
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
            if let Err(e) = AppendMenuW(hmenu, MF_STRING, IDM_OPEN_SETTINGS, w!("設定を開く")) {
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
        win.eval(&push_config_script(s)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// ユーザー JS をネイティブ注入用にラップする。
fn user_js_wrapper(js: &str) -> String {
    format!("try {{\n{js}\n}} catch (e) {{ console.error('[jirapp] user JS error', e); }}")
}

/// 現在設定を page 側 `__JIRAPP_CONFIG__` に流し込み、適用関数を呼ぶスクリプト。
fn push_config_script(s: &Settings) -> String {
    // 文字列は JSON エンコードで安全にエスケープする。
    let css = serde_json::to_string(&s.custom_css).unwrap_or_else(|_| "\"\"".into());
    format!(
        "(function(){{ if (!window.__JIRAPP_APPLY__) return; \
         window.__JIRAPP_APPLY__({{\
         autoReloadEnabled:{auto},\
         idleThresholdSecs:{idle},\
         reloadCheckIntervalSecs:{interval},\
         customCss:{css}\
         }}); }})();",
        auto = s.auto_reload_enabled,
        idle = s.idle_threshold_secs,
        interval = s.reload_check_interval_secs,
        css = css,
    )
}

/// 基盤 JS。アイドル検知・アイドル時自動リロード・CSS 適用の土台を仕込む。
/// ナビゲーション前に毎回ネイティブ実行される（initialization_script）。
const MACHINERY_JS: &str = r#"
(function () {
  if (window.__JIRAPP_INSTALLED__) return;
  window.__JIRAPP_INSTALLED__ = true;

  // 既定設定（Rust 側の __JIRAPP_APPLY__ 呼び出しで上書きされる）
  window.__JIRAPP_CONFIG__ = window.__JIRAPP_CONFIG__ || {
    autoReloadEnabled: false,
    idleThresholdSecs: 300,
    reloadCheckIntervalSecs: 30,
    customCss: ""
  };

  // --- アイドル検知: 最後のユーザー操作時刻を記録 ---
  var lastActivity = Date.now();
  function touch() { lastActivity = Date.now(); }
  ["mousemove", "mousedown", "keydown", "scroll", "wheel", "touchstart"].forEach(function (ev) {
    window.addEventListener(ev, touch, { passive: true, capture: true });
  });

  // --- アイドル時の自動リロード ---
  var reloadTimer = null;
  function scheduleReload() {
    if (reloadTimer) { clearInterval(reloadTimer); reloadTimer = null; }
    var cfg = window.__JIRAPP_CONFIG__;
    if (!cfg.autoReloadEnabled) return;
    var intervalMs = Math.max(5, cfg.reloadCheckIntervalSecs | 0) * 1000;
    reloadTimer = setInterval(function () {
      var c = window.__JIRAPP_CONFIG__;
      if (!c.autoReloadEnabled) return;
      var idleMs = Date.now() - lastActivity;
      if (idleMs >= Math.max(5, c.idleThresholdSecs | 0) * 1000) {
        lastActivity = Date.now(); // 連続リロード防止
        location.reload();
      }
    }, intervalMs);
  }

  // --- ユーザー CSS 適用 ---
  function applyCss(css) {
    var id = "__jirapp_user_css__";
    var el = document.getElementById(id);
    if (!el) {
      el = document.createElement("style");
      el.id = id;
      (document.head || document.documentElement).appendChild(el);
    }
    el.textContent = css || "";
  }

  // 設定適用のエントリポイント（Rust から呼ぶ）
  window.__JIRAPP_APPLY__ = function (cfg) {
    if (cfg) window.__JIRAPP_CONFIG__ = cfg;
    applyCss(window.__JIRAPP_CONFIG__.customCss);
    scheduleReload();
  };

  // DOM 準備時に既定設定で一度適用しておく
  function bootstrap() { window.__JIRAPP_APPLY__(); }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bootstrap);
  } else {
    bootstrap();
  }
})();
"#;
