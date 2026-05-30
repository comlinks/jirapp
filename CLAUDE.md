# CLAUDE.md — jirapp

Jira 専用ブラウザ（Site-Specific Browser）。Jira Cloud（`*.atlassian.net`）の web 画面をシステムブラウザから独立したセッションで表示し、任意の JS/CSS 注入と自動リロードを行う Windows 向け Tauri v2 アプリ。

## 技術スタック

- **フレームワーク**: Tauri v2
- **バックエンド**: Rust
- **フロントエンド**: Vue 3 + TypeScript (Vite)
- **WebView**: WebView2 (Windows / Chromium ベース)
- **設定永続化**: tauri-plugin-store
- **対象 OS**: Windows のみ（クロスプラットフォーム不要）

## アーキテクチャ

ウィンドウは 2 系統で構成する。

- **設定ウィンドウ (main)**: Vue SPA。Jira URL、注入する JS/CSS の編集、リロード間隔、アイドル判定などを設定・編集する UI。
- **Jira ウィンドウ (jira)**: Jira の web 画面を直接ロードする専用 webview。設定ウィンドウで編集した内容を適用する。

設定は Rust 側（tauri-plugin-store）が single source of truth。Jira ウィンドウは起動時および設定変更時に Rust から設定を受け取って適用する。

## 重要な設計方針

### セッション独立

WebView2 のユーザーデータフォルダをこのアプリ専用パスに固定し、システムの Edge/Chrome と Cookie・認証状態を共有させない。`WebviewWindowBuilder::data_directory()`（または起動時の環境変数）で専用ディレクトリを指定する。これが要件の根幹なので、デフォルトプロファイルに混ざらないことを必ず確認する。

### JS/CSS 注入

- **ナビゲーション前に毎回実行したいもの** → `initialization_script()` を使う。WebView2 のネイティブ注入なので Jira の CSP の影響を受けにくい。基盤的な処理（アイドル検知、リロードフック、CSS 適用の土台）はここに置く。
- **ロード後の動的適用・ユーザー編集分** → Rust から `webview.eval()` で DOM に `<style>` / `<script>` を挿入する。ただし DOM 経由の script 挿入は Jira の CSP で弾かれる可能性があるため、CSP 制約を受けやすい処理は initialization_script 側に寄せる方が安全。
- ユーザーが編集した任意 JS/CSS は store に保存し、適用時に注入する。

### 自動リロード（アイドル時）

「ユーザー操作のないタイミング」が要件なのでアイドル検知が必須。

- WebView 内の JS（initialization_script で仕込む）で `mousemove` / `keydown` / `scroll` の最終操作時刻を記録する。
- 設定されたアイドル時間を超えていたらリロードする。リロード間隔・アイドル閾値は設定可能にする。
- Jira は SPA のため、フルリロード（`location.reload()`）が重い場合は Jira 内部のビュー更新で代替できないか検討する。まずはフルリロードで実装し、必要なら最適化する。

## 制約・注意点

- WebView2 は Chromium ベースだが **Chrome 拡張は使えない**。機能は JS/CSS 注入で代替する前提。
- 対象は Jira Cloud（`*.atlassian.net`）。Atlassian アカウントログイン / 2FA / 外部 IdP SSO の初回フローが webview 内で完結するか要確認。外部 IdP のポップアップ挙動に注意。
- CSP により DOM 経由の script 注入が弾かれうる。CSP 制約のある処理は initialization_script に寄せる。

## コーディング規約

- Rust: 標準的な rustfmt / clippy 準拠。エラーは型で表現し握りつぶさない。
- Vue/TS: Composition API + `<script setup>`。型を明示する。
- Tauri コマンドは Rust 側に集約し、フロントからは invoke で呼ぶ。設定の読み書きは必ず Rust 経由にする。

## 開発時の確認事項

- セッションがシステムブラウザと分離されているか（別アカウントでログインしても干渉しないか）。
- 注入した JS/CSS が CSP で弾かれていないか（コンソールエラー確認）。
- アイドル判定が誤発火していないか（操作中にリロードされない）。
