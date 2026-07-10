# jirapp

[![Release](https://img.shields.io/github/v/release/comlinks/jirapp?sort=semver)](https://github.com/comlinks/jirapp/releases/latest)
[![CI](https://github.com/comlinks/jirapp/actions/workflows/ci.yml/badge.svg)](https://github.com/comlinks/jirapp/actions/workflows/ci.yml)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8D8?logo=tauri&logoColor=white)](https://v2.tauri.app)
[![Vue 3](https://img.shields.io/badge/Vue-3-4FC08D?logo=vuedotjs&logoColor=white)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-2021-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows-0078D6?logo=windows&logoColor=white)](#)

Jira 専用ブラウザ（Site-Specific Browser）。Jira Cloud（`*.atlassian.net`）を
システムブラウザから独立したセッションで表示し、任意の JS/CSS 注入と
アイドル時の自動リロードを行う Windows 向け Tauri v2 アプリ。

設計方針の詳細は [CLAUDE.md](./CLAUDE.md) を参照。

## 特長

- **セッション独立** — WebView2 のユーザーデータフォルダをアプリ専用に固定し、
  システムの Edge/Chrome とは Cookie・認証を分離。
- **JS/CSS 注入** — 任意の JS/CSS を Jira 画面に注入（基盤はネイティブ注入で
  CSP の影響を受けにくい構成）。
- **アイドル時自動リロード** — 最終操作からの経過でアイドル判定し、設定間隔で
  自動リロード（閾値・間隔は設定可能）。
- **GitHub リンク** — 設定画面の octocat アイコンからリポジトリを既定ブラウザで開く。
- **セルフアップデート** — GitHub Releases を参照し、新しい署名済みバージョンを
  ダウンロード＆適用して自動再起動（設定画面の「更新を確認」）。通常起動（Jira
  ウィンドウのみ表示）でも起動時に更新を確認し、あればネイティブの確認ダイアログで
  実行可否を尋ねる。
- **前回の表示を復元** — Jira ウィンドウを閉じたときの URL を保存し、次回起動時に
  同一テナントなら復元。フィルター（`?jql=...`）は URL に載るため、起動ごとに
  リセットされず前回の続きから開ける。
- **設定永続化 / ウィンドウ状態復元** — `tauri-plugin-store` /
  `tauri-plugin-window-state`。

## インストール

[GitHub Releases](https://github.com/comlinks/jirapp/releases/latest) から
NSIS インストーラ（`jirapp_*_x64-setup.exe`）を取得してインストールします。
未署名（コード署名なし）のため初回起動時に SmartScreen 警告が出る場合があります
（詳細情報 → 実行）。以降の更新はアプリ内のセルフアップデートで取得できます。

## 開発

```powershell
npm install            # 依存インストール（初回のみ）
npm run tauri:dev      # 開発起動（dev サーバ + Tauri、ホットリロード）
npm run tauri build    # リリースビルド（NSIS インストーラ生成）
```

- フロントのみの型チェック/ビルド: `npm run build`
- Rust のみのコンパイル確認: `cd src-tauri; cargo check`
- Rust テスト / lint: `cd src-tauri; cargo test` / `cargo clippy -- -D warnings`
- dev サーバは Vite `1430` / HMR `1431`（他の Tauri アプリの既定 `1420` と衝突回避）。
- リリースビルドは `rc.exe`（Windows SDK）が必要で、`vcvars64` を読み込んでから
  `--bundles nsis` で実行する。CI（`v*` タグ push）でも自動ビルドされる。

## 構成

| パス | 役割 |
| --- | --- |
| `src/` | 設定ウィンドウ (main) の Vue 3 + TS SPA |
| `src/api.ts` | Rust コマンドの薄いラッパ（設定・ウィンドウ操作は必ず Rust 経由） |
| `src/composables/useUpdater.ts` | セルフアップデートの状態管理 |
| `src-tauri/src/settings.rs` | 設定の型定義・URL 検証・store 読み書き |
| `src-tauri/src/jira.rs` | Jira ウィンドウ生成・注入・アイドル/リロード基盤・システムメニュー |
| `src-tauri/src/commands.rs` | フロントから呼ぶ Tauri コマンド |
| `src-tauri/src/lib.rs` | アプリ初期化・プラグイン登録・状態管理・起動分岐 |

## ウィンドウ構成

- **設定ウィンドウ (`main`)**: Vue SPA。Jira URL・注入 JS/CSS・アイドル閾値などを編集。
  操作行は「保存して閉じる」「キャンセル」＋バージョン/GitHub/更新確認。
- **Jira ウィンドウ (`jira`)**: Jira の web 画面を直接ロードする専用 webview。
  実行時に `WebviewWindowBuilder` で生成する。

## 設計上のポイント

- **セッション独立**: `lib.rs` 冒頭で環境変数 `WEBVIEW2_USER_DATA_FOLDER`
  （`%LOCALAPPDATA%\com.kanfu.jirapp\webview-data`）をアプリ専用パスに固定する。
  WebView2 の制約で 1 プロセス内では UDF を webview ごとに分けられないため、
  個別ウィンドウの `data_directory()` ではなく**全 webview 共通**の env で指定する。
- **設定の single source of truth**: `tauri-plugin-store`（Rust 側）。フロントは
  `invoke` 経由でのみ読み書きする。
- **CSP 対策の注入**: 基盤（アイドル検知・自動リロード・CSS 適用土台）と
  **ユーザー JS** は `initialization_script()` でネイティブ注入。CSS・閾値などの
  ライブ更新は `webview.eval()` で `<style>` 差し替え＋設定反映。
- **メインスレッドを塞がない**: `open_jira_window` は `async`。WebView2 生成は
  `run_on_main_thread` でイベントループ上にスケジュールする（同期生成は白画面化する）。
- **IPC 境界**: capability は `main` のみにスコープし、Jira ウィンドウ（リモート
  コンテンツ）には Tauri API/IPC を与えない。「設定を開く」導線は Win32 の
  システムメニューで実装する。
- **ドラッグ&ドロップ**: Jira ウィンドウは `disable_drag_drop_handler()` を付け、
  OS の drag-drop ハンドラが Jira ボードの HTML5 D&D を横取りしないようにする。

## 既知の制約 / 注意点

- ユーザー **JS の編集**はネイティブ注入のため、完全反映は Jira ウィンドウの
  再オープン時。CSS・アイドル閾値は「保存して閉じる」で即時反映される。
- WebView2 は Chromium ベースだが Chrome 拡張は使えない（機能は JS/CSS 注入で代替）。
- SSO / 外部 IdP の初回ログインフローが webview 内で完結するかは要検証
  （`id.atlassian.com` 経由の SSO→ボード表示までは確認済み）。
- セルフアップデートは GitHub Releases の publish 済みリリースを参照する
  （下書きのままだと更新検知されない）。
