# Security Policy

## Supported Versions

セキュリティ修正は最新リリースに対してのみ提供されます。

| Version  | Supported          |
| -------- | ------------------ |
| latest   | :white_check_mark: |
| < latest | :x:                |

## Reporting a Vulnerability

脆弱性を発見した場合は、**公開 issue を作成せず**、以下のいずれかの方法で非公開に報告してください。

### GitHub Security Advisories（推奨）

[Report a vulnerability](https://github.com/comlinks/jirapp/security/advisories/new) からプライベートな脆弱性報告を作成してください。

### メール

GitHub を使えない場合は **kan.fushihara@gmail.com** まで連絡してください。

### 報告に含めてほしい情報

- 脆弱性の種類（例: 注入 JS/CSS 経由の XSS、IPC 特権昇格、セッション分離のバイパス 等）
- 影響範囲（影響を受けるバージョン・機能・前提条件）
- 再現手順
- 想定される攻撃シナリオ・影響度
- 可能であれば修正案・PoC

### 対応方針

- **初動応答**: 報告受領から 7 日以内に確認連絡
- **修正リリース**: 重大度に応じて 30 日以内を目標
- **公開**: 修正リリース後に GitHub Security Advisory で詳細を公開（報告者のクレジット記載、希望があれば匿名）

## Scope

jirapp は Jira Cloud（`*.atlassian.net`）を独立セッションで表示する Site-Specific Browser です。
以下を主な対象とします。

- **注入 JS/CSS 経由の XSS / コードインジェクション** — 設定の custom JS/CSS や基盤注入スクリプト経由の任意コード実行
- **Tauri IPC の特権昇格** — リモートコンテンツ（Jira ウィンドウ）への IPC/Tauri API 露出など、capability 境界の破れ
- **セッション分離のバイパス** — WebView2 ユーザーデータフォルダ分離（システムブラウザとの Cookie/認証の混在）を破る経路
- **Jira URL 検証のバイパス** — `https` + `*.atlassian.net` 制限の回避や、Jira を装った非正規ページへの誘導
- **配布物・CI/CD のサプライチェーン** — NSIS インストーラやビルドパイプラインを介した混入

以下は対象外です。

- サービス拒否（DoS）
- マシンへの物理アクセスを前提とする攻撃
- ソーシャルエンジニアリング
- upstream 依存（Tauri / WebView2 等）自体の脆弱性 — まず upstream に報告してください。jirapp での扱いは [Dependabot alerts](https://github.com/comlinks/jirapp/security/dependabot) で追跡します。
