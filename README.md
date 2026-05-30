# jirapp

Jira 専用ブラウザ（Site-Specific Browser）。Jira Cloud（`*.atlassian.net`）を
システムブラウザから独立したセッションで表示し、任意の JS/CSS 注入と
アイドル時の自動リロードを行う Windows 向け Tauri v2 アプリ。

設計方針は [CLAUDE.md](./CLAUDE.md) を参照。

## 開発

```powershell
npm install            # 依存インストール（初回のみ）
npm run tauri dev      # 開発起動（設定ウィンドウ + ホットリロード）
npm run tauri build    # リリースビルド（インストーラ生成）
```

- フロントのみの型チェック/ビルド: `npm run build`
- Rust のみのコンパイル確認: `cd src-tauri; cargo check`
- アイコン再生成: `cargo tauri icon ./app-icon.png -o ./src-tauri/icons`

## 構成

| パス | 役割 |
| --- | --- |
| `src/` | 設定ウィンドウ (main) の Vue 3 + TS SPA |
| `src/api.ts` | Rust コマンドの薄いラッパ（設定は必ず Rust 経由） |
| `src-tauri/src/settings.rs` | 設定の型定義と store 読み書き |
| `src-tauri/src/jira.rs` | Jira ウィンドウ生成・注入・アイドル/リロード基盤 |
| `src-tauri/src/commands.rs` | フロントから呼ぶ Tauri コマンド |
| `src-tauri/src/lib.rs` | アプリ初期化・状態管理 |

## ウィンドウ構成

- **設定ウィンドウ (`main`)**: Vue SPA。Jira URL・注入 JS/CSS・アイドル閾値などを編集。
- **Jira ウィンドウ (`jira`)**: Jira の web 画面を直接ロードする専用 webview。
  実行時に `WebviewWindowBuilder` で生成する。

## 設計上のポイント

- **セッション独立**: Jira webview の `data_directory()` をアプリ専用パス
  （`app_data_dir()/jira-webview-data`）に固定し、システムの Edge/Chrome と
  Cookie・認証状態を共有しない。
- **設定の single source of truth**: tauri-plugin-store（Rust 側）。フロントは
  `invoke` 経由でのみ読み書きする。
- **CSP 対策の注入**:
  - 基盤（アイドル検知・自動リロード・CSS 適用土台）と**ユーザー JS** は
    `initialization_script()` でネイティブ注入（CSP の影響を受けにくい）。
  - CSS・閾値などのライブ更新は `webview.eval()` で `<style>` 差し替え＋設定反映。
- **アイドル自動リロード**: webview 内 JS が `mousemove`/`keydown`/`scroll` 等の
  最終操作時刻を記録し、設定間隔ごとにアイドル超過を判定して `location.reload()`。

## 既知の制約 / TODO

- ユーザー **JS の編集**はネイティブ注入のため、完全反映は Jira ウィンドウの
  再オープン時。CSS・アイドル閾値は「保存して適用」で即時反映される。
- CSP により `eval` 経由のライブ JS 実行は弾かれる場合がある（基盤側へ寄せて回避）。
- SSO / 外部 IdP の初回ログインフローが webview 内で完結するかは要検証。
- フルリロードが重い場合の Jira 内部ビュー更新による代替は未実装。
