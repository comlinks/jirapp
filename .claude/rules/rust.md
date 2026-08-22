---
paths:
  - "src-tauri/**/*.rs"
---

# Rust 実装ルール

`src-tauri/` の Rust を触るときのルール。全体の設計方針（単一 UDF、メインスレッドを塞がない、IPC 境界）は CLAUDE.md 側にある。

## 基本方針

- 標準的な rustfmt / clippy 準拠。整形は `just fmt`、検査は `just fmt-check` / `just clippy`。
- エラーは型で表現し握りつぶさない。
- `unsafe`（`jira.rs` の `sysmenu`）はコメントで不変条件を明示する。
- Tauri コマンドは Rust 側に集約し、フロントからは `invoke`（`src/api.ts`）で呼ぶ。**設定の読み書きは必ず Rust 経由**。

## Tauri コマンド（`generate_handler!`）

`get_settings` / `save_settings` / `open_jira_window`（**async**）/ `apply_to_jira_window` / `hide_settings_window` / `close_settings_window` / `open_url` / `is_jira_open` / `set_settings_height`。`reveal_settings` はコマンドではなくメニューイベント用の共通関数。

- `open_url`：既定ブラウザで URL を開く（設定画面の GitHub リンク用）。**http/https のみ許可**し、`explorer.exe` に URL を**引数として**渡す（シェル非経由でインジェクション回避）。この 2 つの条件は外さない。
- `open_jira_window` を同期 `fn` に戻さない。理由は CLAUDE.md「メインスレッドを塞いだまま webview を生成しない」。

## システムメニュー（`jira.rs` の `sysmenu`）

Jira ウィンドウに IPC を与えない代わりの導線。`GetSystemMenu` に項目を追加し、`SetWindowSubclass` で `WM_SYSCOMMAND` を拾い、コマンド ID で `reload_jira`（`location.reload()` を eval）と `reveal_settings` に分岐する。`WM_NCDESTROY` でサブクラス解除とコールバック回収を行う（リークなし）。

Jira 側に新しい導線を足すときも、IPC ではなくこのネイティブ機構に乗せる。

## 切り分け用ログ

`eprintln!` の `building` / `built ok` / `page_load` は残してある。Jira ウィンドウの挙動が変なときはまずこのログを見る。

## テスト

ユニットテストは純粋なロジックに書く（現状は `settings` の URL 検証と `jira` のテナント判定）。`just test` で走る。
