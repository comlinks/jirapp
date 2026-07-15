# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.8.0] - 2026-07-15

### Features

- **リロード機能 (#25)**: Jira ウィンドウを手動で再読み込みできるようにした。設定を開く導線と同様にタイトルバー左上のシステムメニューへ「再読み込み」を追加（`WM_SYSCOMMAND` をコマンド ID で分岐し `location.reload()` を eval する。IPC は使わない）。加えてブラウザ系のキー操作に揃えて **F5** でも再読み込みできる（注入 JS `inject/reload_shortcut.js` が keydown を捕捉。WebView2 のアクセラレータキー有効/無効に依らず一貫して効くよう自前で `location.reload()`）。
- **表示 URL の随時保存 (#24)**: これまで `lastUrl`（次回起動の復元先）は Jira ウィンドウを閉じる時にだけ保存していたため、jirapp を終了せず Windows をシャットダウンした場合などにフィルター変更後の URL を取りこぼしていた。Jira ウィンドウ生成時にバックグラウンドの監視（`spawn_last_url_poll`）を開始し、表示 URL を 10 秒間隔でポーリングして**変化したときだけ** `lastUrl` を永続化するようにした。フィルター変更は SPA の `pushState` で URL に載る（フルロードを伴わない）ため `on_page_load` では拾えないが、ポーリング＋`webview.url()` で追従できる。`webview.url()` は UI スレッド必須のため読み取りは `run_on_main_thread` に載せ、ウィンドウが無くなったら監視を終える。
- **チケットキーのコピー (#22)**: カンバンカードのチケットキー（`COM-123` 等）の隣に、カードをホバーしたとき現れる小さなコピーボタンを追加した。クリックでキー文字列をクリップボードへコピーし、成功を一瞬チェックマークで示す。既存のキーのリンク（クリックでチケットを開く）は残し、その隣に足すだけにしている。ボタンは `navigator.clipboard`（失敗時は `execCommand` フォールバック）でコピーし、SPA の再描画には `MutationObserver` で追従する。注入 JS は `inject/card_key_copy.js` として基盤 `machinery.js` の `JIRAPP.registerFeature` に登録する（`jira.rs` は変更なし）。

## [0.7.0] - 2026-07-11

### Features

- **カンバン列ヘッダの色変更 (#21)**: Jira ボードの列（ステータス）ヘッダの背景色を変更できるようにした。各列の ⋯（その他の操作）メニューに「色の変更」を追加し、Jira の accent パレット（`--ds-background-accent-*-subtlest`）から選択／クリアできる。色は**ステータス名をキー**に `about:blank` の hidden iframe 経由で native localStorage へ保存し、Jira 側に IPC を与えず永続化する（Atlassian が `window.localStorage` をメモリシムに差し替えるため、シム対象外の native ストアへ iframe 経由で書く）。着色は `data-jirapp-col` 属性＋注入 `<style>` の `!important` で行い、Jira 既定のインライン背景（グレー）には触れないため「クリア」で元に戻る。SPA 遷移・再描画には `MutationObserver` で追従する。

### Internal

- **注入 JS のモジュール分離・プラットフォーム化**: 注入 JS を Rust の生文字列から `src-tauri/src/inject/*.js` に切り出し、`inject.rs` が `include_str!` で取り込む構成に変更（エディタ支援・lint が効く）。基盤 `machinery.js` を `window.JIRAPP` プラットフォーム（`registerFeature` / `store` / `addStyle` / `onConfig`）に整理し、各機能はこれに登録する。document-start 注入は `DOC_START_SCRIPTS` レジストリで一括登録し、新しい JS 拡張は `.js` を 1 枚足して 1 行登録するだけでよい。`jira.rs` はウィンドウ生命周期・URL 解決・システムメニューに専念する構成に整理。
- **注入 JS の Biome lint を CI に追加**: `biome.json` で `src-tauri/src/inject/*.js` を対象に lint（formatter は無効、lint のみ）。CI に ubuntu の `lint-inject` ジョブ（`biomejs/setup-biome` + `biome lint`）を追加した。フロント（Vue/TS）は従来どおり `npm run build` の型チェックで担保する。

## [0.6.0] - 2026-07-11

### Features

- **起動時の更新確認ダイアログ**: 2 回目以降の通常起動では Jira ウィンドウだけが開き設定ウィンドウは隠れるため、これまで更新に気づけなかった。起動時に更新チェックを行い、設定ウィンドウが非表示のとき更新があれば `tauri-plugin-dialog` のネイティブ確認ダイアログ（「更新して再起動」/「後で」）で実行可否を尋ねるようにした。設定ウィンドウ表示中（URL 未設定起動・メニューからの再表示）は従来どおりバナーで扱い、二重には出さない。更新の check / downloadAndInstall は既存の `useUpdater` を再利用。dialog の権限（`dialog:allow-ask` / `dialog:allow-message`）は capability の `main` スコープのみで、Jira ウィンドウには与えない（IPC 境界を維持）。

## [0.5.0] - 2026-07-10

### Bug Fixes

- **フィルター設定が起動の度にリセットされる (#20)**: Jira ボードのフィルターは URL（`?jql=...`）に載るが、起動時に常に設定の「ホーム」URL（クエリなし）を開いていたため、前回付けたフィルターが毎回失われていた。Jira ウィンドウを閉じる際に現在表示していた URL を `lastUrl` として保存し、次回起動時に**同一テナント（https + 登録ホストと同一）に限り**復元するようにした（前回の続きから開く）。フィルターは SPA の `pushState` で URL に反映されるが、WebView2 の `Source` はこれにも追従するため、閉じる時に `webview.url()` で現在値を拾える。ホーム URL を変更したときは `lastUrl` を破棄して新しいボードを開く。`lastUrl` は設定 UI に出さない実行時状態として `tauri-plugin-store` に保存する。

### Internal

- 起動時に開く URL の解決を `jira::resolve_startup_url` に集約し、`build_jira_window` は開く URL を引数で受け取る形に変更（ホーム／復元の両方に対応）。

## [0.4.0] - 2026-06-08

### Features

- **同一テナントのリンクを別ウィンドウで開く** (#10): 登録した Jira と同一ホスト（Confluence 等の同一 `*.atlassian.net` テナント）への `target=_blank` / `window.open` を、同一セッションのまま別ウィンドウ（WebView2 ポップアップ）で開く。別テナント・非 https・外部ドメインは従来どおり抑制し、SSO 等のポップアップ挙動は変えない。ポップアップは Tauri 管理外のため IPC は渡らず境界を維持（`jira.rs` の `on_new_window`）。
- **設定ウィンドウを開いたタイミングで更新を自動チェック** (#8): 設定表示時に更新を確認し、更新があればフォーム上部のバナーで案内。自動チェックは silent（最新・失敗時は静かに idle へ、更新時のみ表示）。手動「更新を確認」は従来どおり最新/エラーを表示。
- **詳細設定の折り畳み** (#9): リロード設定（自動リロード・アイドル閾値・チェック間隔）と CSS/JS 注入を「詳細設定」(`<details>`) として既定で折り畳む。折り畳み時は Jira URL のみ表示。

### Improvements

- **操作ボタンを下端に固定し、ウィンドウ高を自動調整**: 詳細設定の開閉・更新バナー・テキストエリアのリサイズに応じて、設定ウィンドウの高さを実コンテンツ高へ自動フィット（`set_settings_height` コマンド + `ResizeObserver`）。

## [0.3.0] - 2026-06-02

### Features

- **設定画面の GitHub リンク**: バージョン表記の右に octocat アイコンを追加し、クリックで GitHub リポジトリを既定ブラウザで開く（`open_url` コマンド: http/https のみ許可し `explorer.exe` に引数渡しでシェルインジェクションを回避）。
- **操作行の整理**: 「保存して閉じる」「キャンセル」とバージョン/GitHub/「更新を確認」を同じ行に統合し、更新系を右寄せに（`flex-wrap` で狭い幅では折り返し）。

### Internal

- **フロントのビルドツールをメジャー更新**: vite 6→8、@vitejs/plugin-vue 5→6、vue-tsc 2→3、typescript 5.7→6（いずれも dev 依存、`npm run build` で検証）。
- **Dependabot**: `windows` クレートを自動更新の対象外に（tauri/tao が使う 0.61 に手動追従するため）。
- **.gitignore 整備**: 秘密情報の安全網（`*.key`/`*.pem`/`.env`/`*.p12`/`*.pfx`/`credentials.json`/`secrets.json`）を追加し、生成物 `src-tauri/gen/schemas` を追跡解除。
- **`tauri:dev` npm スクリプト**を追加。
- ドキュメント（CLAUDE.md / README.md）を実装準拠に更新。

## [0.2.0] - 2026-06-01

### Features

- **セルフアップデート**: 設定画面に「更新を確認」を追加。GitHub Releases の `latest.json` を参照し、新しい署名済みバージョンがあればダウンロード＆適用して自動再起動する（`tauri-plugin-updater` + `tauri-plugin-process`）。updater/process の権限は設定ウィンドウ (`main`) のみにスコープし、Jira ウィンドウには付与しない。

### Bug Fixes

- **チケットのドラッグが出来ない (#1)**: Tauri がデフォルトで登録する OS レベルの drag-drop ハンドラが、Jira ボードの HTML5 ドラッグ&ドロップ（カード移動など）を横取りして無効化していた。Jira ウィンドウ生成時に `disable_drag_drop_handler()` を呼び、WebView 内のネイティブな D&D を有効化。

### Changed

- **設定画面のボタン刷新 (#2)**: 「Jira を開く / 設定を閉じる」のトグル＋「保存して適用」を、**「保存して閉じる」(primary)** と **「キャンセル」** の 2 ボタンに変更。
  - 保存して閉じる: 設定を保存し、Jira が開いていれば適用して設定を隠す。未オープンなら保存した設定で Jira を開く。
  - キャンセル: 未保存の編集を破棄して閉じる（Jira があれば隠す、無ければアプリ終了。設定ウィンドウの ✕ と同じ挙動）。`close_settings_window` コマンドを追加。

### Internal

- **CI（GitHub Actions）**: `npm run build`（vue-tsc 型チェック + Vite ビルド）、`cargo fmt --check`、`cargo clippy -D warnings`、`cargo test`（ユニットテスト）を Windows ランナーで実行。
- **ユニットテスト**: `settings` の Jira URL 検証（`validate_jira_url` / `require_jira_url`）と既定値の単体テストを追加。
- **Security Check**: `cargo audit` と `npm audit`（本番依存・critical）を週次 + push/PR で実行。
- **CodeQL**: GitHub の CodeQL コードスキャン（Default setup）を有効化（リポジトリ設定側で構成。`javascript-typescript` / `rust`）。
- **Dependabot**: npm / cargo / github-actions を週次更新。サプライチェーン対策に cooldown、Tauri 関連はグループ化。
- **SECURITY.md**: 脆弱性報告フローと対象スコープを明文化。
- **Release ワークフロー**: `v*` タグ push で NSIS インストーラをビルドして下書きリリースを作成。バンドル対象を `nsis` に固定（MSI/WiX 依存による失敗を回避）。
- 既存コードの clippy 指摘（needless borrow 3 件）の解消と rustfmt 整形。

## [0.1.0] - 2026-05-30

Initial release.

### Features

- **Jira 専用ブラウザ** — Jira Cloud（`*.atlassian.net`）を、システムブラウザから独立したセッションで表示する Windows 向け Tauri v2 アプリ。
- **セッション独立** — WebView2 のユーザーデータフォルダをアプリ専用に固定し、システムの Edge/Chrome とは Cookie・認証を分離。
- **JS/CSS 注入** — 任意の JS/CSS を WebView に注入（基盤処理はネイティブ注入で CSP の影響を受けにくい構成）。
- **アイドル時自動リロード** — 最終操作からの経過時間でアイドルを判定し、設定間隔で自動リロード。閾値・チェック間隔は設定可能。
- **設定の永続化** — `tauri-plugin-store` で設定を保存。Jira ウィンドウの位置・サイズ・最大化は `tauri-plugin-window-state` で復元。
- **設定導線** — リモートコンテンツに IPC を与えないため、Jira ウィンドウのシステムメニュー（Win32）から設定を開く。

[Unreleased]: https://github.com/comlinks/jirapp/compare/v0.8.0...HEAD
[0.8.0]: https://github.com/comlinks/jirapp/releases/tag/v0.8.0
[0.7.0]: https://github.com/comlinks/jirapp/releases/tag/v0.7.0
[0.6.0]: https://github.com/comlinks/jirapp/releases/tag/v0.6.0
[0.5.0]: https://github.com/comlinks/jirapp/releases/tag/v0.5.0
[0.4.0]: https://github.com/comlinks/jirapp/releases/tag/v0.4.0
[0.3.0]: https://github.com/comlinks/jirapp/releases/tag/v0.3.0
[0.2.0]: https://github.com/comlinks/jirapp/releases/tag/v0.2.0
[0.1.0]: https://github.com/comlinks/jirapp/releases/tag/v0.1.0
