# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

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

[0.4.0]: https://github.com/comlinks/jirapp/releases/tag/v0.4.0
[0.3.0]: https://github.com/comlinks/jirapp/releases/tag/v0.3.0
[0.2.0]: https://github.com/comlinks/jirapp/releases/tag/v0.2.0
[0.1.0]: https://github.com/comlinks/jirapp/releases/tag/v0.1.0
