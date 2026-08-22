---
paths:
  - "src/**/*.{vue,ts,css}"
  - "src-tauri/src/inject/*.js"
  - "src-tauri/src/inject.rs"
---

# フロント・注入 JS の実装ルール

設定ウィンドウの Vue SPA（`src/`）と、Jira へ注入する JS（`src-tauri/src/inject/`）のルール。

## 設定ウィンドウ（`src/`）

- Composition API + `<script setup>`。型を明示する。
- **`App.vue`**：設定 UI。操作行は「保存して閉じる」(primary) と「キャンセル」、続けてバージョン表記・GitHub(octocat) リンク・「更新を確認」を同じ行に右寄せで並べる。`settings:refresh` イベントで状態追従する。
- **`composables/useUpdater.ts`**：セルフアップデートの状態管理（`check` / `downloadAndInstall` → `relaunch`）。`App.vue` の `onMounted` で起動時チェックを行い、設定ウィンドウが**非表示**なら更新時にネイティブ確認ダイアログ（`ask`）を出す。表示中はバナーで扱う。
- **`api.ts`**：`invoke` ラッパ。設定の読み書き・ウィンドウ操作はすべてここ経由。
- **`types.ts`**：Rust の `Settings`（camelCase）に対応する型。
- **`styles.css`**：テーマ変数 `--bg` を定義（ライト/ダーク）。フッターのグラデもこれに追従。
- フロントは Jira 画面の DOM を直接触らない。注入はすべて Rust（`inject`）経由。

## 注入 JS の構成

注入 JS は Rust の生文字列ではなく `src-tauri/src/inject/*.js` に置き、`inject.rs` が `include_str!` で取り込む（エディタ支援と lint が効く）。`inject.rs` の `DOC_START_SCRIPTS`（`&[&str]`）に並べた順で document-start にネイティブ注入する。

- **基盤プラットフォーム**：`inject/machinery.js`（`DOC_START_SCRIPTS` の**先頭固定**）。アイドル検知・自動リロード・ユーザー CSS 適用の土台に加え、各機能が乗る `window.JIRAPP` を用意する。`registerFeature(name, fn)`（多重登録ガードと DOM 準備後の `fn(JIRAPP)` 実行）/ `store.get/set(key, ...)`（iframe 経由 native localStorage 永続化）/ `addStyle(id, css)`（id 付き `<style>`）/ `onConfig(cb)`（Rust からの設定購読）。
- **個別機能**：`column_color.js`（列ヘッダ着色, #21）、`card_key_copy.js`（キーのコピー, #22）、`reload_shortcut.js`（F5 リロード, #25）、`reload_button.js`（左下フローティングボタン, #26）。`JIRAPP.registerFeature("...", function (app) { ... })` の形で基盤に登録し、`app.store` / `app.addStyle` を共有利用する。DOM は `data-testid` で辿り、SPA 追従は各機能内の `MutationObserver` で行う。
- **新しい JS 拡張機能の足し方**：`inject/<feature>.js` を作って `JIRAPP.registerFeature` で登録し、`inject.rs` の `DOC_START_SCRIPTS` に `include_str!` 定数を 1 行足すだけ。`jira.rs` は触らない。
- **ユーザー JS**：`inject::user_js_wrapper` で `try/catch` ラップし、基盤・各機能の後に注入する（構文エラーを基盤へ波及させない）。
- **ユーザー CSS と設定値**：`inject::push_config_script` を `webview.eval` で流し込む。`on_page_load` の `Finished` 時と、保存時のライブ適用（`jira::apply`）で再注入される。page 側の `window.__JIRAPP_APPLY__` が CSS 適用とリロード再スケジュール、`onConfig` 通知を行う。
- lint は Biome（`just lint-inject`）。設定は `biome.json` で `inject/*.js` に限定してある（formatter は off、lint のみ）。Biome の版は justfile の `biome_version` に一本化してあり、CI も同じレシピを呼ぶ。

## SPA への追従

`initialization_script` はフルナビゲーション時のみ再実行され、クライアント側のルート遷移では走らない。遷移に追従させたい JS は `MutationObserver` / `setInterval` で常駐させ、多重実行は `registerFeature` の登録ガードで防ぐ。

CSP により DOM 経由の script 注入が弾かれうる。CSP 制約のある処理は `initialization_script` に寄せる。

## 自動リロード（アイドル時）

`machinery.js` 内で `mousemove` / `keydown` / `scroll` 等の最終操作時刻を記録し、設定間隔ごとにアイドル閾値超過を判定して `location.reload()` する。閾値とチェック間隔は設定可能。連続リロード防止に最終操作時刻をリセットする。

- **編集中はスキップする**：判定時に `isEditing()`（`document.activeElement` が textarea / テキスト系 input / `isContentEditable`。shadow root は `shadowRoot.activeElement` を辿る）が真ならリロードせず、最終操作時刻を更新して見送る。Jira の説明・コメント欄は input でも textarea でもなく ProseMirror の contenteditable なので、この判定を外すと本来の目的（編集内容を消さない）を果たせない。最終操作時刻を更新するのは、フォーカスを外した直後に即リロードされるのを防ぐため（外してから改めて閾値ぶん放置されたらリロードする）。
