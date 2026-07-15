//! Jira ウィンドウへ注入する JS 資産と、その配線・設定反映のまとめ。
//!
//! 注入 JS は Rust の生文字列ではなく `inject/*.js` に置き、`include_str!` で取り込む
//! （エディタ支援・lint が効き、Rust 側はロジックに専念できる）。
//!
//! 新しい注入機能を足す手順は次の 2 ステップだけ。まず `inject/<feature>.js` を作り、
//! `JIRAPP.registerFeature("<name>", function (app) { ... })` の形で基盤プラットフォーム
//! （machinery.js）に登録する。次にその `include_str!` 定数を [`DOC_START_SCRIPTS`] へ 1 行
//! 足す（`MACHINERY_JS` は先頭固定）。これで document-start 注入に乗る。
//!
//! ユーザー CSS/設定値は [`push_config_script`] 経由で `webview.eval` により反映され、
//! page 側の `window.__JIRAPP_APPLY__` が受け取る。

use crate::settings::Settings;

/// 基盤 JS ＝ 共通プラットフォーム（`window.JIRAPP`）。アイドル検知・自動リロード・
/// ユーザー CSS 適用の土台と、各機能が使う store / addStyle / registerFeature を提供する。
const MACHINERY_JS: &str = include_str!("inject/machinery.js");

/// 列ヘッダ着色機能（issue #21）。`JIRAPP.registerFeature` で基盤に登録する。
const COLUMN_COLOR_JS: &str = include_str!("inject/column_color.js");

/// チケットキーのコピー機能（issue #22）。カンバンカードのキー隣にコピーボタンを足す。
const CARD_KEY_COPY_JS: &str = include_str!("inject/card_key_copy.js");

/// F5 リロード機能（issue #25）。ブラウザ系に揃えて F5 で location.reload() する。
const RELOAD_SHORTCUT_JS: &str = include_str!("inject/reload_shortcut.js");

/// document-start でネイティブ注入するスクリプト群（順序どおり登録される）。
/// **先頭は必ず `MACHINERY_JS`**（他機能が乗る `window.JIRAPP` を先に用意する）。
/// 機能追加時はここへ 1 行足すだけでよい。
pub(crate) const DOC_START_SCRIPTS: &[&str] = &[
    MACHINERY_JS,
    COLUMN_COLOR_JS,
    CARD_KEY_COPY_JS,
    RELOAD_SHORTCUT_JS,
];

/// ユーザー JS をネイティブ注入用にラップする。構文エラーがあってもこの script 内に閉じ、
/// 基盤処理へ波及させない。
pub(crate) fn user_js_wrapper(js: &str) -> String {
    format!("try {{\n{js}\n}} catch (e) {{ console.error('[jirapp] user JS error', e); }}")
}

/// 現在設定を page 側 `__JIRAPP_CONFIG__` に流し込み、適用関数 `__JIRAPP_APPLY__` を呼ぶスクリプト。
/// `on_page_load`(Finished) 時と、保存時のライブ適用（`jira::apply`）で `webview.eval` される。
pub(crate) fn push_config_script(s: &Settings) -> String {
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
