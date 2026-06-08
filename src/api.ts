import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "./types";

// 設定の読み書きは必ず Rust 経由（tauri-plugin-store が single source of truth）。

export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export function saveSettings(settings: Settings): Promise<void> {
  return invoke("save_settings", { settings });
}

/** Jira ウィンドウを開く（既に開いていればフォーカス）。 */
export function openJiraWindow(): Promise<void> {
  return invoke("open_jira_window");
}

/** 現在の設定を Jira ウィンドウへライブ適用する（CSS/JS の再注入・閾値更新）。 */
export function applyToJiraWindow(): Promise<void> {
  return invoke("apply_to_jira_window");
}

/** 設定ウィンドウを隠す（「設定を閉じる」導線）。 */
export function hideSettingsWindow(): Promise<void> {
  return invoke("hide_settings_window");
}

/** 設定ウィンドウを閉じる（「キャンセル」導線。Jira があれば隠す、無ければ終了）。 */
export function closeSettingsWindow(): Promise<void> {
  return invoke("close_settings_window");
}

/** Jira ウィンドウが開いているか（ボタン表示の切替に使う）。 */
export function isJiraOpen(): Promise<boolean> {
  return invoke<boolean>("is_jira_open");
}

/** 既定ブラウザで URL を開く（http/https のみ。GitHub リンク等）。 */
export function openUrl(url: string): Promise<void> {
  return invoke("open_url", { url });
}

/** 設定ウィンドウの高さを実コンテンツ高(CSS px)に合わせる（折り畳み開閉などで丁度良く）。 */
export function setSettingsHeight(height: number): Promise<void> {
  return invoke("set_settings_height", { height });
}
