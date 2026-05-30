// Rust 側の Settings 構造体（serde rename_all = "camelCase"）と対応させる。
// single source of truth は Rust 側。フロントは invoke 経由でのみ読み書きする。
export interface Settings {
  /** 表示する Jira Cloud の URL（例: https://your-domain.atlassian.net） */
  jiraUrl: string;
  /** ユーザーが注入する任意の JS */
  customJs: string;
  /** ユーザーが注入する任意の CSS */
  customCss: string;
  /** アイドル時の自動リロードを有効にするか */
  autoReloadEnabled: boolean;
  /** アイドルと判定するまでの秒数（最後の操作からの経過） */
  idleThresholdSecs: number;
  /** アイドル判定をチェックする間隔（秒） */
  reloadCheckIntervalSecs: number;
}
