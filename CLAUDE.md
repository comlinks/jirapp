# CLAUDE.md（jirapp）

Jira 専用ブラウザ（Site-Specific Browser）。Jira Cloud（`*.atlassian.net`）の web 画面を、システムブラウザから独立したセッションで表示し、任意の JS/CSS 注入とアイドル時自動リロードを行う Windows 向け Tauri v2 アプリ。

## 領域別ルール（`.claude/rules/`）

領域ごとの実装ルールは `.claude/rules/` に分けてある。`paths` frontmatter を付けてあるので、該当ファイルを読んだときに読み込まれる。

| ファイル | 内容 | 読み込まれる条件 |
| --- | --- | --- |
| `rust.md` | Rust 実装ルール、Tauri コマンド一覧、`sysmenu` の実装 | `src-tauri/**/*.rs` |
| `frontend.md` | 設定ウィンドウの Vue/TS、注入 JS の構成と足し方、自動リロード | `src/**`、`src-tauri/src/inject/` |
| `testing.md` | 動作確認の項目、SSO の確認 | ソースファイル全般 |

このファイル（CLAUDE.md）には、領域をまたぐ設計と、外すと壊れる不変条件だけを置く。

## 技術スタック

- **フレームワーク**：Tauri v2。バックエンドは Rust（`src-tauri/`）、フロントは Vue 3 + TypeScript（Vite）。
- **WebView**：WebView2（Windows / Chromium ベース）。
- **対象 OS**：Windows のみ（クロスプラットフォーム不要）。
- **プラグイン**：`tauri-plugin-store`（設定永続化）、`tauri-plugin-window-state`（Jira ウィンドウの位置・サイズ・最大化）、`tauri-plugin-updater` ＋ `tauri-plugin-process`（セルフアップデートと再起動）、`tauri-plugin-dialog`（起動時の更新確認）。いずれも権限は設定ウィンドウ（`main`）のみ。
- **Win32 連携**：`windows` クレート（Jira ウィンドウのシステムメニュー）。

## ソース構成

### Rust（`src-tauri/src/`）

- **`lib.rs`**：`run()`。起動時に `WEBVIEW2_USER_DATA_FOLDER` を設定 → プラグイン登録 → `setup`（設定読込・`AppState` 管理・main ウィンドウのクローズ挙動・起動時分岐）→ `invoke_handler`。
- **`commands.rs`**：Tauri コマンド群と `reveal_settings`（メニューから設定を表示する共通関数）。
- **`jira.rs`**：Jira ウィンドウの生成・適用・クローズ挙動、URL/テナント解決、`sysmenu` モジュール。注入 JS 自体は持たず `inject` に委譲する。
- **`inject.rs` ＋ `inject/*.js`**：Jira へ注入する JS 資産と配線。`inject/*.js` を `include_str!` で取り込み、`DOC_START_SCRIPTS` に並べて document-start 注入する。
- **`settings.rs`**：`Settings` 構造体、store の読み書き（`load_settings` / `persist_settings`）。

### フロント（`src/`）

`App.vue`（設定 UI）/ `composables/useUpdater.ts`（セルフアップデート）/ `api.ts`（`invoke` ラッパ）/ `types.ts` / `styles.css`。各ファイルの役割は `.claude/rules/frontend.md`。

## ウィンドウ構成と状態遷移

ウィンドウは 2 系統。

- **設定ウィンドウ (`main`)**：Vue SPA。`tauri.conf.json` で生成（`visible:false` / `maximizable:false`）。表示は Rust が制御する。
- **Jira ウィンドウ (`jira`)**：Jira を直接ロードする専用 webview。`jira::build_jira_window` で動的生成。

設定は **Rust 側（`tauri-plugin-store`）が single source of truth**。`AppState(Mutex<Settings>)` はそのメモリキャッシュ。フロントは `invoke` 経由でのみ読み書きする。なお store には `Settings`（キー `settings`）とは別に **`lastUrl`**（前回終了時の Jira URL＝起動時の復元先）も持つ。これは設定 UI に出さない実行時状態で、`Settings` には含めない。ホーム URL（`jira_url`）を変更保存すると `lastUrl` は破棄される（`save_settings`）。

### 起動・表示フロー（`lib.rs` setup ＋ commands）

- 起動時、保存 URL が **空 → 設定ウィンドウを表示** / **設定済み → Jira を自動オープン**（設定ウィンドウは非表示のまま。自動オープン失敗時は設定ウィンドウを表示）。自動オープンの URL は `jira::resolve_startup_url` が解決し、**前回終了時に保存した URL（`lastUrl`）が同一テナント（https + 登録ホスト一致）なら復元**して「前回の続き」から開く（フィルター `?jql=...` は URL に載るためこれで維持される）。無い／別テナント／不正なら設定のホーム URL（`jira_url`）を開く。
- フロントの「保存して閉じる」→ 保存後、Jira が開いていれば `apply_to_jira_window`＋`hide_settings_window`、未オープンなら `open_jira_window`（Jira を開いたら設定ウィンドウを `hide`）。
- 「キャンセル」→ 編集を破棄して `close_settings_window`（Jira があれば `hide`、無ければ `app.exit(0)`＝main ✕ と同じ挙動）。
- Jira のシステムメニュー「設定を開く」→ `reveal_settings`（main を `show`＋`set_focus`＋`settings:refresh` 発火）。「再読み込み」→ `reload_jira`。F5 と左下のフローティングボタンでも同じリロードができる（注入 JS）。
- フロントは `is_jira_open` ＋ `settings:refresh` で状態追従する（ボタン自体は常に表示）。
- **クローズ挙動**：
  - `main` の ✕：Jira が開いていれば閉じず `hide`（＝設定を閉じる扱い）。Jira が無ければ閉じて終了。
  - `jira` の ✕：`CloseRequested` で現在の表示 URL を `webview.url()` で取得し `lastUrl` として保存（次回起動の復元用）。その後、設定ウィンドウが非表示なら `app.exit(0)`（アプリ終了）。`Destroyed` で `settings:refresh` を発火しフロントを更新。
  - **URL の随時保存（issue #24）**：クローズ時保存だけだと、jirapp を終了せず Windows をシャットダウンした場合などに最新 URL を取りこぼす。フィルター変更は SPA の pushState で URL に載る（フルロードを伴わない）ため `on_page_load` でも拾えない。そこで `build_jira_window` が `spawn_last_url_poll` で表示 URL を 10 秒間隔でポーリングし、**変化したときだけ** `lastUrl` を永続化する（`webview.url()` は UI スレッド必須なので読み取りは `run_on_main_thread` に載せ、ウィンドウが無くなったら監視を終える）。
- Jira ウィンドウの位置・サイズ・最大化は `tauri-plugin-window-state` が保存／復元（`main` は denylist で除外）。生成は `visible:false` → `restore_state` → `show` の順で初期位置のちらつきを防ぐ。

## 重要な設計方針・ハマりどころ

新しく触る際にここを外すと壊れやすい。順守すること。

### セッション独立（単一 UDF）

WebView2 のユーザーデータフォルダを `lib.rs` 冒頭の環境変数 `WEBVIEW2_USER_DATA_FOLDER`（`%LOCALAPPDATA%\com.kanfu.jirapp\webview-data`）でアプリ専用に固定し、システムの Edge/Chrome と Cookie・認証を分離する。

- **1 プロセス内で webview ごとに異なる UDF は使えない**（WebView2 の制約）。個別ウィンドウの `data_directory()` を指定すると 2 つ目の webview 生成が失敗し白画面化する。webview を増やしても **UDF は全 webview 共通**にすること。

### メインスレッドを塞いだまま webview を生成しない

`open_jira_window` は **`async fn`** にし、生成は `jira::open` 内の `run_on_main_thread` でイベントループ上にスケジュールしてチャネルで結果を待つ。

- 同期コマンドはメインスレッドでイベントループを止める。WebView2 生成はメッセージループが回ることを要するため、**同期のまま `build()` を呼ぶと生成が完了せず白画面・無反応**になる（過去の主要バグ）。
- 起動時の自動オープン（`setup` 内）は既にメインスレッド上なので `build_jira_window` を直接呼んでよい。逆に **`setup` から `run_on_main_thread`＋`recv` で待つとデッドロック**するので使い分ける。

### Jira ウィンドウにはリモート IPC を与えない（セキュリティ境界）

`capabilities/default.json` の capability は **`main` のみ**にスコープし、Jira ウィンドウ（リモートコンテンツ）には Tauri API/IPC を一切与えない。`updater:default` / `process:default`（セルフアップデート）、`dialog:allow-ask` / `dialog:allow-message`（起動時の更新確認ダイアログ）も同様に `main` 限定で、Jira 側からは更新 API もダイアログも呼べない。

- 「再読み込み」「設定を開く」導線は IPC ではなく **Win32 のシステムメニュー**（`jira.rs` の `sysmenu`）で実装している。実装の詳細は `.claude/rules/rust.md`。
- この境界は維持すること。Jira 側に新しい導線を足す場合も、IPC ではなくネイティブ機構（メニュー等）で。

### JS/CSS 注入

Jira 画面への機能追加は Chrome 拡張が使えない（WebView2 は Chromium ベースだが拡張非対応）ため、すべて JS/CSS 注入で行う。基盤 `inject/machinery.js` が `window.JIRAPP` プラットフォームを提供し、各機能は `registerFeature` で登録する。**新しい JS 拡張は `.js` を 1 枚足して `DOC_START_SCRIPTS` に 1 行足すだけ**で、`jira.rs` は触らない。詳細は `.claude/rules/frontend.md`。

## ドキュメント校正ルール

- **対象**：`README.md` / `CHANGELOG.md`（リリース時に足す節）
- **対象外**：`CLAUDE.md`（読み手が開発者の密な技術メモ）、英語で書く `SECURITY.md`
- **手順**
  1. `just lint-docs`（textlint。preset-ai-writing + preset-ja-technical-writing を npx で取る。リポジトリには入れない）
  2. `japanese-tech-writing` スキルで、textlint が拾えない空句・冗長・演出・論証を点検する
- **守る表記規約**
  - 箇条書きの太字ラベルの区切りは全角コロン（`**用語**：説明`）。半角コロンは `no-ai-list-formatting` に触れる
  - 地の文と見出しで em ダッシュ（`—`）を使わない
  - 誇張語（「大幅に」等）と LLM 空句（「重要なのは」「正面から」「多角的」等）を使わない
- **据え置いてよい指摘**
  - `no-mix-dearu-desumasu` と、列挙が主因の `sentence-length`
  - CHANGELOG の過去セクション（出荷済みの記録なので、表記の一括正規化以外は触らない）
  - 誤検出の常連：UI 名やエスケープシーケンス中のリテラルの `?`、行を折り返した括弧

据え置き分が常に残るため `lint-docs` は `just check` に入れていない。exit 1 をそのまま失敗とみなさず、指摘の中身で判断する。

## 開発ワークフロー

開発タスクの入口は `justfile` に集約してある（`just` でレシピ一覧）。CI（`ci.yml` / `security.yml`）も同じレシピを呼ぶので、コマンドを変えるときは justfile 側だけを直せば両方に効く。レシピの実体は npm scripts や cargo に委ねた薄いファサードで、`--manifest-path` は `working-directory` 属性で不要にしてある。

- 起動：`just dev`（`! just dev` でこのセッションのログに出せる）。Vite の dev サーバだけなら `just dev-web`。
- ビルド確認：`just build`（vue-tsc の型検査 + vite build）。警告ゼロで通ることが基線。

### コミット前チェック

変更の規模で段を変える。

| 変更の規模 | 実行するもの |
| --- | --- |
| ある程度の規模の実装・修正 | `/code-review` → `simplify` → `just check` |
| 軽微なコード修正 | `simplify` → `just check` |
| ドキュメントのみ | 「ドキュメント校正ルール」の手順 |
| バージョン bump のみ | 何も要らない |

順序が要点で、`/code-review`（バグ探索）で挙がったものを直してから `simplify`（再利用・単純化・効率・抽象度の整理）を回す。simplify はバグを探さないので、先に回しても直すべきコードを整えるだけになる。どちらもコードを書き換えるため、**動作確認より前**に実行する（確認するのは適用後のコード）。`/code-review` はユーザーがコマンドを実行する。

`just check` の中身は CI の `check` ジョブ + `lint-inject` ジョブと同じ内容・同じ順で、`build` → `fmt-check` → `clippy` → `test` → `lint-inject`。

- `build` を先に置くのは `tauri-build` が `frontendDist`（`../dist`）の存在を要求するため。
- `npm run build` が通っても **`cargo fmt --check` は別物**で、整形漏れがあると CI だけ赤くなる（実害あり: v0.4.0 で発生）。整形の適用は `just fmt`。
- vcvars の読み込みは要らない（2026-08-22 に clippy / `tauri build` とも素の Git Bash で通ることを確認済み）。

### 環境上の注意（過去に実害あり）

- 重要な Write/Edit は **1 つずつ**実行し、長時間のビルドコマンドと同一バッチに混ぜない（並列バッチで書き込み競合・ファイル破損が起きた実績あり）。
- ビルド結果はファイルに落として読む（端末出力が時系列で錯綜する）。判断は必ず最新ログで。
- `tauri dev` のファイル監視は逐次編集の合間に再コンパイルを走らせるため、**途中の一時エラーは無視**してよい。最終ビルド結果で判断する。
- dev 停止後に `vite(node)` / `jirapp.exe` が孤児化し **ポート 1430 を掴み続ける**ことがある（dev サーバは Vite `1430` / HMR `1431`。pike 等の既定 `1420` との衝突回避のため変更済み）。`Get-NetTCPConnection -LocalPort 1430` で PID 特定 → **PowerShell の `Stop-Process -Id <PID> -Force`** で倒す（bash の `kill` は Windows ネイティブ PID に効かないことがある）。
- 日本語を含むファイルを sed / perl で一括置換するとき、置換文字列に全角文字を使うなら `-C` 系フラグを付けない（perl の `-CSD` で二重エンコードして文字化けした実績あり）。バイト単位で置換し、置換後に化けが無いことを確認する。

## リリース手順

exe 配布は **CI Release（`.github/workflows/release.yml` ／ tauri-action）が主**。`v*` タグの push で起動し、NSIS インストーラ＋セルフアップデート用 `latest.json` をビルドして **下書き** リリースを作成する（`bundle.targets` は `["nsis"]` 固定）。手順:

1. **`CHANGELOG.md` に該当バージョンの項目を追加**（Keep a Changelog 形式。最新版を先頭に、Features / Internal 等で分類し、末尾のリンク定義も追加）。**`chore(release)` コミットには含めず**、機能コミット側または `docs:` コミットで入れる（`chore(release)` は bump のみに保つ）。
2. **バージョン bump**（`chore(release)` は bump のみで、範囲外の変更を抱き合わせない）。`just bump X.Y.Z` が 5 ファイルを順に更新する:
   - `scripts/bump-version.mjs` が `package.json` / `src-tauri/tauri.conf.json` / `src-tauri/Cargo.toml` の version 行を書き換える（各ファイルで該当行が 1 箇所だけであることを確認してから書き、違えば中止する）
   - `npm install --package-lock-only` が `package-lock.json` を更新する（ルートと `packages[""]` の 2 箇所）
   - `cargo check` が `src-tauri/Cargo.lock` の `jirapp` の version 行を更新する
   - lockfile 2 つは手で書かない。特に `Cargo.lock` は依存側にも同名バージョンが並ぶため、全置換すると壊れる。
   - 実行後 `git diff` で、バージョン行以外の lock ドリフトが出ていないことを確認する。
3. `git commit`（メッセージ `chore(release): X.Y.Z`）→ `git push origin main`（このリポは PR 運用なし＝直接 push）。
4. `git tag -a vX.Y.Z -m "..."` → `git push origin vX.Y.Z`。これで `release.yml` が起動しビルド → **下書き** リリースが作られる。
5. **ビルド完了を確認して下書きを publish する**: `gh run watch` で Release ワークフローの成功を待ち、アセット（`jirapp_<ver>_x64-setup.exe` と `latest.json`）が添付されていることを確認してから `gh release edit vX.Y.Z --draft=false --latest` で公開する。`updater` エンドポイント `…/releases/latest/download/latest.json` は **publish 済みの最新リリースしか解決しない**ため、publish しないとセルフアップデートが配信されない。publish は外部公開のため、通常は事前承認を得てから実行する。
6. **再リリース**（アセット不足等）: タグを付け替えて `git push --force` でタグ更新すると同一下書きにアセットが追補される（リリース削除は不要）。

補足・落とし穴:
- **CI 不調時のローカルビルド（フォールバック）**: `just build-installer`（＝`tauri build --bundles nsis`。`targets:"all"` は MSI が WiX 依存で失敗しやすいので nsis に絞る）。成果物は `src-tauri/target/release/bundle/nsis/jirapp_<ver>_x64-setup.exe`。`latest.json` まで要るときは `TAURI_SIGNING_PRIVATE_KEY` を渡す（無いとインストーラは出来るが署名段でエラーになる）。詳細は開発メモ `jirapp-release-build` 参照。
- **updater 署名鍵**はコード署名とは別物。秘密鍵は `C:\Users\kanfu\.tauri\jirapp-updater.key`（リポ外・要バックアップ）、CI は Secret `TAURI_SIGNING_PRIVATE_KEY`。`tauri.conf.json` の `bundle.createUpdaterArtifacts: true` が無いと `latest.json` が生成されず tauri-action がスキップする（Release ジョブは success になるので気づきにくい）。
