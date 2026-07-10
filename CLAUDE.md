# CLAUDE.md — jirapp

Jira 専用ブラウザ（Site-Specific Browser）。Jira Cloud（`*.atlassian.net`）の web 画面を、システムブラウザから独立したセッションで表示し、任意の JS/CSS 注入とアイドル時自動リロードを行う Windows 向け Tauri v2 アプリ。

## 技術スタック

- **フレームワーク**: Tauri v2
- **バックエンド**: Rust（`src-tauri/`）
- **フロントエンド**: Vue 3 + TypeScript（Vite）、Composition API + `<script setup>`
- **WebView**: WebView2（Windows / Chromium ベース）
- **設定永続化**: `tauri-plugin-store`
- **ウィンドウ状態**: `tauri-plugin-window-state`（Jira ウィンドウの位置・サイズ・最大化）
- **セルフアップデート**: `tauri-plugin-updater`（GitHub Releases の `latest.json` を参照）＋ `tauri-plugin-process`（適用後の再起動）。権限は設定ウィンドウ(`main`)のみ。
- **Win32 連携**: `windows` クレート（Jira ウィンドウのシステムメニュー）
- **対象 OS**: Windows のみ（クロスプラットフォーム不要）

## ソース構成

### Rust（`src-tauri/src/`）

- **`lib.rs`** — `run()`。起動時に `WEBVIEW2_USER_DATA_FOLDER` を設定 → プラグイン登録 → `setup`（設定読込・`AppState` 管理・main ウィンドウのクローズ挙動・起動時分岐）→ `invoke_handler`。
- **`commands.rs`** — Tauri コマンド群と `reveal_settings`（メニューから設定を表示する共通関数）。
- **`jira.rs`** — Jira ウィンドウの生成・適用・基盤 JS、`sysmenu` モジュール（システムメニュー）。
- **`settings.rs`** — `Settings` 構造体、store の読み書き（`load_settings` / `persist_settings`）。

### フロント（`src/`）

- **`App.vue`** — 設定 UI。操作行は「保存して閉じる」(primary) / 「キャンセル」、続けてバージョン表記・GitHub(octocat) リンク・セルフアップデートの「更新を確認」を同じ行に右寄せで並べる。`settings:refresh` イベントで状態追従。
- **`composables/useUpdater.ts`** — セルフアップデートの状態管理（`check` / `downloadAndInstall` → `relaunch`）。
- **`api.ts`** — `invoke` ラッパ。設定の読み書き・ウィンドウ操作はすべてここ経由。
- **`types.ts`** — Rust の `Settings`（camelCase）に対応する型。
- **`styles.css`** — テーマ変数 `--bg` を定義（ライト/ダーク）。フッターのグラデもこれに追従。

## ウィンドウ構成と状態遷移

ウィンドウは 2 系統。

- **設定ウィンドウ (`main`)**: Vue SPA。`tauri.conf.json` で生成（`visible:false` / `maximizable:false`）。表示は Rust が制御する。
- **Jira ウィンドウ (`jira`)**: Jira を直接ロードする専用 webview。`jira::build_jira_window` で動的生成。

設定は **Rust 側（`tauri-plugin-store`）が single source of truth**。`AppState(Mutex<Settings>)` はそのメモリキャッシュ。フロントは `invoke` 経由でのみ読み書きする。なお store には `Settings`（キー `settings`）とは別に **`lastUrl`**（前回終了時の Jira URL＝起動時の復元先）も持つ。これは設定 UI に出さない実行時状態で、`Settings` には含めない。ホーム URL（`jira_url`）を変更保存すると `lastUrl` は破棄される（`save_settings`）。

### 起動・表示フロー（`lib.rs` setup ＋ commands）

- 起動時、保存 URL が **空 → 設定ウィンドウを表示** / **設定済み → Jira を自動オープン**（設定ウィンドウは非表示のまま。自動オープン失敗時は設定ウィンドウを表示）。自動オープンの URL は `jira::resolve_startup_url` が解決し、**前回終了時に保存した URL（`lastUrl`）が同一テナント（https + 登録ホスト一致）なら復元**して「前回の続き」から開く（フィルター `?jql=...` は URL に載るためこれで維持される）。無い／別テナント／不正なら設定のホーム URL（`jira_url`）を開く。
- フロントの「保存して閉じる」→ 保存後、Jira が開いていれば `apply_to_jira_window`＋`hide_settings_window`、未オープンなら `open_jira_window`（Jira を開いたら設定ウィンドウを `hide`）。
- 「キャンセル」→ 編集を破棄して `close_settings_window`（Jira があれば `hide`、無ければ `app.exit(0)`＝main ✕ と同じ挙動）。
- Jira のシステムメニュー「設定を開く」→ `reveal_settings`（main を `show`＋`set_focus`＋`settings:refresh` 発火）。
- フロントは `is_jira_open` ＋ `settings:refresh` で状態追従する（ボタン自体は常に表示）。
- **クローズ挙動**:
  - `main` の ✕: Jira が開いていれば閉じず `hide`（＝設定を閉じる扱い）。Jira が無ければ閉じて終了。
  - `jira` の ✕: `CloseRequested` で現在の表示 URL を `webview.url()` で取得し `lastUrl` として保存（次回起動の復元用）。その後、設定ウィンドウが非表示なら `app.exit(0)`（アプリ終了）。`Destroyed` で `settings:refresh` を発火しフロントを更新。
- Jira ウィンドウの位置・サイズ・最大化は `tauri-plugin-window-state` が保存／復元（`main` は denylist で除外）。生成は `visible:false` → `restore_state` → `show` の順で初期位置のちらつきを防ぐ。

### Tauri コマンド（`generate_handler!`）

`get_settings` / `save_settings` / `open_jira_window`（**async**）/ `apply_to_jira_window` / `hide_settings_window` / `close_settings_window` / `open_url` / `is_jira_open`。`reveal_settings` はコマンドではなくメニューイベント用の共通関数。

- `open_url` — 既定ブラウザで URL を開く（設定画面の GitHub リンク用）。**http/https のみ許可**し、`explorer.exe` に URL を**引数として**渡す（シェル非経由でインジェクション回避）。

## 重要な設計方針・ハマりどころ

新しく触る際にここを外すと壊れやすい。順守すること。

### セッション独立（単一 UDF）

WebView2 のユーザーデータフォルダを `lib.rs` 冒頭の環境変数 `WEBVIEW2_USER_DATA_FOLDER`（`%LOCALAPPDATA%\com.kanfu.jirapp\webview-data`）でアプリ専用に固定し、システムの Edge/Chrome と Cookie・認証を分離する。

- **1 プロセス内で webview ごとに異なる UDF は使えない**（WebView2 の制約）。個別ウィンドウの `data_directory()` を指定すると 2 つ目の webview 生成が失敗し白画面化する。webview を増やしても **UDF は全 webview 共通**にすること。

### メインスレッドを塞いだまま webview を生成しない

`open_jira_window` は **`async fn`** にし、生成は `jira::open` 内の `run_on_main_thread` でイベントループ上にスケジュールしてチャネルで結果を待つ。

- 同期コマンドはメインスレッドでイベントループを止める。WebView2 生成はメッセージループが回ることを要するため、**同期のまま `build()` を呼ぶと生成が完了せず白画面・無反応**になる（過去の主要バグ）。
- 起動時の自動オープン（`setup` 内）は既にメインスレッド上なので `build_jira_window` を直接呼んでよい。逆に **`setup` から `run_on_main_thread`＋`recv` で待つとデッドロック**するので使い分ける。
- 切り分け用の `eprintln!` ログ（`building` / `built ok` / `page_load`）は残してある。挙動が変なときはまずこのログを見る。

### JS/CSS 注入

- **基盤処理（アイドル検知・自動リロード・CSS 適用土台）** = `jira.rs` の `MACHINERY_JS` を `initialization_script` でネイティブ注入。CSP の影響を受けにくく、各フルロードの document-start で毎回走る。
- **ユーザー JS** = 別の `initialization_script` として `try/catch` でラップしネイティブ注入（構文エラーを基盤処理に波及させない）。
- **ユーザー CSS と設定値** = `push_config_script` を `webview.eval` で流し込む。`on_page_load` の `Finished` 時、および保存時のライブ適用（`apply`）で再注入される。page 側の `window.__JIRAPP_APPLY__` が CSS 適用とリロード再スケジュールを行う。
- フロントは DOM を直接触らない。注入はすべて Rust 経由。
- **SPA の注意**: `initialization_script` はフルナビゲーション時のみ再実行され、クライアント側のルート遷移では走らない。遷移に追従させたい JS は `MutationObserver` / `setInterval` で常駐させ、`if (window.__flag__) return;` で多重実行を防ぐ。

### Jira ウィンドウにはリモート IPC を与えない（セキュリティ境界）

`capabilities/default.json` の capability は **`main` のみ**にスコープし、Jira ウィンドウ（リモートコンテンツ）には Tauri API/IPC を一切与えない。`updater:default` / `process:default`（セルフアップデート）も同様に `main` 限定で、Jira 側からは更新 API を呼べない。

- 「設定を開く」導線は IPC ではなく **Win32 のシステムメニュー**（`jira.rs` の `sysmenu` モジュール）で実装している。`GetSystemMenu` に項目を追加し、`SetWindowSubclass` で `WM_SYSCOMMAND` を拾って `reveal_settings` を呼ぶ。WM_NCDESTROY でサブクラス解除＋コールバック回収（リークなし）。
- この境界は維持すること。Jira 側に新しい導線を足す場合も、IPC ではなくネイティブ機構（メニュー等）で。

### 自動リロード（アイドル時）

`MACHINERY_JS` 内で `mousemove`/`keydown`/`scroll` 等の最終操作時刻を記録し、設定間隔ごとにアイドル閾値超過を判定して `location.reload()` する。閾値・チェック間隔は設定可能。連続リロード防止に最終操作時刻をリセットする。Jira は SPA なのでフルリロードが重ければ将来内部ビュー更新で代替を検討（現状はフルリロード）。

## 制約・注意点

- WebView2 は Chromium ベースだが **Chrome 拡張は使えない**。機能は JS/CSS 注入で代替する前提。
- 対象は Jira Cloud（`*.atlassian.net`）。Atlassian ログイン / 2FA / 外部 IdP SSO の初回フローが webview 内で完結するか要確認（外部 IdP のポップアップ挙動に注意）。動作確認時、`id.atlassian.com` 経由の SSO→ボード表示まで通ることは確認済み。
- CSP により DOM 経由の script 注入が弾かれうる。CSP 制約のある処理は `initialization_script` に寄せる。

## コーディング規約

- Rust: 標準的な rustfmt / clippy 準拠。エラーは型で表現し握りつぶさない。`unsafe`（`sysmenu`）はコメントで不変条件を明示する。
- Vue/TS: Composition API + `<script setup>`。型を明示する。
- Tauri コマンドは Rust 側に集約し、フロントからは `invoke`（`api.ts`）で呼ぶ。**設定の読み書きは必ず Rust 経由**。

## 開発ワークフロー

- ビルド確認（基線＝どちらも警告ゼロで通ること）:
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run build`
- **コミット／リリース前は CI 相当チェックをローカルでも回す**（CI = `.github/workflows/ci.yml` が main への push / PR で `cargo fmt --check` → `cargo clippy --all-targets -- -D warnings` → `cargo test` を順に実行）。`cargo check` / `npm run build` が通っても **`cargo fmt --check` は別物**で、整形漏れがあると CI（lint）だけ赤くなる（実害あり: v0.4.0 で発生）。
  - `cargo fmt --manifest-path src-tauri/Cargo.toml --check`（整形だけなら `--check` を外して適用）
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `cargo test --manifest-path src-tauri/Cargo.toml`
  - 注意: `clippy` / `tauri build` は `tauri-winres` が `rc.exe` を要求するため、vcvars を読み込んでから実行する（`fmt --check` / `cargo test` は不要）。詳細はリリース手順のメモ参照。
- 起動: `npm run tauri dev`（`! npm run tauri dev` でこのセッションのログに出せる）。
- **環境上の注意（過去に実害あり）**:
  - 重要な Write/Edit は **1 つずつ**実行し、長時間のビルドコマンドと同一バッチに混ぜない（並列バッチで書き込み競合・ファイル破損が起きた実績あり）。
  - ビルド結果はファイルに落として読む（端末出力が時系列で錯綜する）。判断は必ず最新ログで。
  - `tauri dev` のファイル監視は逐次編集の合間に再コンパイルを走らせるため、**途中の一時エラーは無視**してよい。最終ビルド結果で判断する。
  - dev 停止後に `vite(node)` / `jirapp.exe` が孤児化し **ポート 1430 を掴み続ける**ことがある（dev サーバは Vite `1430` / HMR `1431`。pike 等の既定 `1420` との衝突回避のため変更済み）。`Get-NetTCPConnection -LocalPort 1430` で PID 特定 → **PowerShell の `Stop-Process -Id <PID> -Force`** で倒す（bash の `kill` は Windows ネイティブ PID に効かないことがある）。

## 動作確認時のチェック

- セッションがシステムブラウザと分離されているか（別アカウントでログインしても干渉しないか）。
- 注入した JS/CSS が CSP で弾かれていないか（Jira ウィンドウの devtools コンソールでエラー確認。`F12` / `Ctrl+Shift+I` で開ける）。
- アイドル判定が誤発火していないか（操作中にリロードされない）。
- 起動分岐・設定の表示/非表示・「保存して閉じる」/「キャンセル」の挙動・Jira クローズ時の終了・ウィンドウ位置復元が想定どおりか。
- チケットのドラッグ&ドロップが効くか（`disable_drag_drop_handler()` が前提）。「更新を確認」「GitHub リンク」が動くか。
