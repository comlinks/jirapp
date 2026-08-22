# jirapp のタスクランナー。`just` で一覧、`just check` でコミット前チェック一式。
#
# レシピの実体は package.json の scripts に置いたまま、just はその薄いファサードに
# してある（tauri CLI の慣習で npm 経由が要るものを壊さないため）。cargo 系は
# working-directory 属性で src-tauri に降りるので --manifest-path は要らない。

# レシピは Git Bash で走らせる。just の Windows 既定シェル（sh -c）は PATH に存在
# せず、PATH 上の bash は WSL ランチャ（C:\Windows\System32\bash.exe）で、そちらには
# Windows 側の node / cargo が無い。Git を別の場所に入れている場合は
# `just --shell <bash へのパス>` で上書きする（GitHub の windows runner はこのパスで合っている）。
set windows-shell := ["C:/Program Files/Git/bin/bash.exe", "-cu"]

# Biome は CI（lint-inject ジョブ）と同じ版に固定する。片方だけ上げないこと。
biome_version := "2.4.10"

# レシピ一覧を出す
default:
    @just --list --unsorted

# --- 開発 ---

# Jira ウィンドウ込みで開発版を起動する
dev:
    npm run tauri:dev

# Vite の dev サーバだけを起動する
dev-web:
    npm run dev

# フロントをビルドする（vue-tsc の型検査 + vite build）
build:
    npm run build

# NSIS インストーラまで作る（CI Release が主。これはフォールバック）
build-installer:
    npm run tauri build -- --bundles nsis

# --- コミット前チェック ---

# build を先に置くのは tauri-build が frontendDist（../dist）の存在を要求するため。
# コミット前チェック一式（CI の check ジョブ + lint-inject ジョブと同じ内容・同じ順）
check: build fmt-check clippy test lint-inject

# rustfmt を適用する
[working-directory('src-tauri')]
fmt:
    cargo fmt

# 整形漏れを検出する（cargo check や npm run build が通っても、これは別物）
[working-directory('src-tauri')]
fmt-check:
    cargo fmt --check

# clippy（警告もエラー扱い）
[working-directory('src-tauri')]
clippy:
    cargo clippy --all-targets -- -D warnings

# Rust のユニットテスト
[working-directory('src-tauri')]
test:
    cargo test

# Jira へ注入する静的 JS（src-tauri/src/inject/*.js）の Biome lint
lint-inject:
    npx --yes @biomejs/biome@{{biome_version}} lint --error-on-warnings

# 型検査だけを回す（build まで待ちたくないとき）
typecheck:
    npx vue-tsc --noEmit

# 据え置いてよい指摘が常に残るため check には入れない（CLAUDE.md「ドキュメント校正
# ルール」参照）。textlint はリポジトリには入れず npx で都度取る。
# 日本語ドキュメント（README / CHANGELOG）の textlint
lint-docs:
    npx --yes --package textlint \
      --package textlint-rule-preset-ai-writing \
      --package textlint-rule-preset-ja-technical-writing \
      -- textlint --rule preset-ai-writing --rule preset-ja-technical-writing \
      README.md CHANGELOG.md

# --- 監査（security ワークフローと同じ内容） ---

# 依存の脆弱性を見る（cargo install cargo-audit が要る）
[working-directory('src-tauri')]
audit-cargo:
    cargo audit

# npm の依存を見る（本番依存のみ・critical だけ落とす）
audit-npm:
    npm audit --omit=dev --audit-level=critical

# --- リリース ---

# バージョンを上げる（3 ファイル + lockfile 2 つ）。CHANGELOG.md は手で書く
bump VERSION:
    node scripts/bump-version.mjs {{VERSION}}
    npm install --package-lock-only
    cd src-tauri && cargo check --quiet
