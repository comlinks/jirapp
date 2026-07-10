<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, message } from "@tauri-apps/plugin-dialog";
import type { Settings } from "./types";
import {
  applyToJiraWindow,
  closeSettingsWindow,
  getSettings,
  hideSettingsWindow,
  isJiraOpen,
  openJiraWindow,
  openUrl,
  saveSettings,
  setSettingsHeight,
} from "./api";
import { useUpdater } from "./composables/useUpdater";

// このアプリの GitHub リポジトリ。octocat アイコンから開く。
const REPO_URL = "https://github.com/comlinks/jirapp";

function openRepo() {
  // setStatus は関数宣言のため巻き上げられ、ここから呼べる。
  openUrl(REPO_URL).catch((e) => {
    setStatus(`GitHub を開けません: ${e}`, true);
  });
}

// セルフアップデート（更新確認・ダウンロード・再起動）。設定ウィンドウ内でのみ使う。
const updater = useUpdater();

// 更新フローの状態を日本語メッセージにする。
const updaterStatusText = computed(() => {
  switch (updater.state.value) {
    case "checking":
      return "更新を確認しています…";
    case "available":
      return `新しいバージョン v${updater.updateVersion.value} があります`;
    case "downloading":
      return "ダウンロードして適用中…（完了後に自動で再起動します）";
    case "upToDate":
      return "最新版です";
    case "error":
      return `更新に失敗: ${updater.errorMessage.value}`;
    default:
      return "";
  }
});

const settings = reactive<Settings>({
  jiraUrl: "",
  customJs: "",
  customCss: "",
  autoReloadEnabled: true,
  idleThresholdSecs: 300,
  reloadCheckIntervalSecs: 30,
});

const loading = ref(true);
const busy = ref(false);
const status = ref("");
const isError = ref(false);
// Jira ウィンドウが開いているか。開いていれば主ボタンは「設定を閉じる」になる。
const jiraOpen = ref(false);
// settings:refresh リスナーの解除関数（アンマウント時に多重登録/リークを防ぐ）。
let unlisten: UnlistenFn | null = null;

// 実コンテンツ高に追従してウィンドウ高を調整する（詳細設定の開閉・バナー・テキストエリア
// リサイズなどで「丁度良い」高さにする）。直近送信値を保持して無駄な resize を抑える。
let contentObserver: ResizeObserver | null = null;
let lastSentHeight = 0;

// #app の自然な高さ（padding 込み, CSS px）を測ってウィンドウ高に反映する。
// #app は viewport 連動の高さ指定を持たないため offsetHeight = 実コンテンツ高で、
// ウィンドウ高を変えても再測定値は変わらない（リサイズの無限ループは起きない）。
function syncWindowHeight() {
  const el = document.getElementById("app");
  if (!el) return;
  const content = Math.ceil(el.offsetHeight);
  // 上限は画面内に収める（はみ出し防止）。下限は Rust の set_settings_height と
  // ウィンドウ minHeight が担保するため、ここでは課さない（240px の一元管理）。
  const maxH = Math.floor(window.screen.availHeight * 0.95);
  const target = Math.min(content, maxH);
  // 1px 程度の揺れでは送らない（スクロールバー出現等のチャタリング防止）。
  if (Math.abs(target - lastSentHeight) < 2) return;
  lastSentHeight = target;
  setSettingsHeight(target).catch(() => {
    /* 失敗時は据え置き */
  });
}

function setStatus(msg: string, error = false) {
  status.value = msg;
  isError.value = error;
}

// 数値フィールドが空/非数値のまま保存されると Rust 側（u64）でデシリアライズに失敗するため、
// 保存前に整数へ丸め、最小値 5 を下回らないようにする。
function sanitizeNumbers() {
  const fix = (v: unknown) => {
    const n = Math.floor(Number(v));
    return Number.isFinite(n) && n >= 5 ? n : 5;
  };
  settings.idleThresholdSecs = fix(settings.idleThresholdSecs);
  settings.reloadCheckIntervalSecs = fix(settings.reloadCheckIntervalSecs);
}

async function refreshJiraOpen() {
  try {
    jiraOpen.value = await isJiraOpen();
  } catch {
    /* 取得失敗時は据え置き */
  }
}

// 起動時の更新チェック。設定ウィンドウが隠れている（＝Jira 自動オープンでの通常起動）
// ときは、更新があってもバナーに気づけないため、ネイティブの確認ダイアログを出して
// 実行可否を尋ねる。表示中のときはバナー任せ（silent チェックのみ）。
async function maybeCheckUpdateOnStartup() {
  let hidden = false;
  try {
    hidden = !(await getCurrentWindow().isVisible());
  } catch {
    /* 可視状態の取得に失敗したら表示扱いにしてバナーに委ねる */
  }
  await updater.checkForUpdate({ silent: true });
  // 表示中、または更新が無い/確認失敗なら何もしない（従来どおりバナーで扱う）。
  // ここで state.value を直接ナローイングすると後段の "error" 比較が潰れるため boolean 経由。
  const updateAvailable = updater.state.value === "available";
  if (!hidden || !updateAvailable) return;

  const yes = await ask(
    `新しいバージョン v${updater.updateVersion.value} があります。今すぐ更新して再起動しますか？`,
    {
      title: "jirapp の更新",
      kind: "info",
      okLabel: "更新して再起動",
      cancelLabel: "後で",
    },
  ).catch(() => false);
  if (!yes) return;

  // downloadAndInstall は成功時に再起動して戻らない。失敗時は state を error にして
  // 戻る（例外は投げない）ため、隠れた設定ウィンドウの代わりにダイアログで知らせる。
  await updater.downloadAndInstall();
  if (updater.state.value === "error") {
    await message(`更新に失敗しました: ${updater.errorMessage.value}`, {
      title: "jirapp の更新",
      kind: "error",
    }).catch(() => {});
  }
}

onMounted(async () => {
  try {
    Object.assign(settings, await getSettings());
  } catch (e) {
    setStatus(`設定の読み込みに失敗: ${e}`, true);
  } finally {
    loading.value = false;
  }
  await refreshJiraOpen();
  // 起動時の更新チェック。設定ウィンドウが表示されている場合（URL 未設定での起動や
  // メニューからの再表示）はバナーで気づけるので silent チェックのみ。2 回目以降の通常
  // 起動では Jira ウィンドウだけが開き設定ウィンドウは隠れているため、更新があっても
  // バナーに気づけない。その場合はネイティブの確認ダイアログで更新の実行可否を尋ねる。
  await maybeCheckUpdateOnStartup();
  // Rust 側（メニューからの再表示・Jira クローズ）からの状態更新通知でボタン表示を追従させる。
  unlisten = await listen("settings:refresh", () => {
    setStatus("");
    refreshJiraOpen();
    // メニュー「設定を開く」等で設定が再表示されるたびに更新も確認する。
    updater.checkForUpdate({ silent: true });
    syncWindowHeight();
  });

  // コンテンツ描画後に初回フィットし、以降は #app の高さ変化（折り畳み開閉・
  // バナー・テキストエリアのリサイズ）に追従してウィンドウ高を調整する。
  await nextTick();
  syncWindowHeight();
  const appEl = document.getElementById("app");
  if (appEl && typeof ResizeObserver !== "undefined") {
    contentObserver = new ResizeObserver(() => syncWindowHeight());
    contentObserver.observe(appEl);
  }
});

onUnmounted(() => {
  unlisten?.();
  unlisten = null;
  contentObserver?.disconnect();
  contentObserver = null;
});

// 保存して閉じる（primary）。設定を保存し、Jira が開いていれば適用して設定を隠す。
// Jira が未オープンなら保存した設定で Jira を開く（open_jira_window 側で設定は隠れる）。
async function saveAndClose() {
  busy.value = true;
  try {
    sanitizeNumbers();
    await saveSettings({ ...settings });
    if (jiraOpen.value) {
      await applyToJiraWindow();
      await hideSettingsWindow();
    } else {
      await openJiraWindow();
      jiraOpen.value = true;
    }
  } catch (e) {
    setStatus(`保存に失敗: ${e}`, true);
  } finally {
    busy.value = false;
  }
}

// キャンセル（単に閉じるだけ）。未保存の編集を破棄して閉じる。
// Jira が開いていれば設定を隠すだけ、無ければアプリを終了する（main ✕ と同じ挙動）。
async function cancel() {
  busy.value = true;
  try {
    // 保存済みの値へ戻して編集を破棄する（次回表示時に確定値が出るように）。
    try {
      Object.assign(settings, await getSettings());
    } catch {
      /* 取得失敗時はそのまま閉じる */
    }
    await closeSettingsWindow();
  } catch (e) {
    setStatus(`閉じられません: ${e}`, true);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <main>
    <h1>jirapp 設定</h1>
    <p class="subtitle">
      Jira Cloud を独立セッションで表示する専用ブラウザの設定。
    </p>

    <template v-if="!loading">
      <!-- 更新が見つかったら目立つバナーで案内する（自動チェック/手動チェック共通）。 -->
      <div v-if="updater.state.value === 'available'" class="update-banner">
        <span class="update-banner__text">
          新しいバージョン v{{ updater.updateVersion.value }} があります。
        </span>
        <button
          class="update-banner__btn"
          :disabled="updater.isBusy.value"
          @click="updater.downloadAndInstall"
        >
          今すぐ更新して再起動
        </button>
      </div>

      <div class="field">
        <label for="jiraUrl">
          Jira URL
          <span class="hint">https の *.atlassian.net のみ（例: https://your-domain.atlassian.net）</span>
        </label>
        <input
          id="jiraUrl"
          v-model="settings.jiraUrl"
          type="text"
          placeholder="https://your-domain.atlassian.net"
          spellcheck="false"
        />
      </div>

      <!-- 詳細設定: リロード設定と CSS/JS 注入は通常はいじらないため既定で折り畳む。 -->
      <details class="advanced">
        <summary>詳細設定</summary>

        <div class="field checkbox">
          <input
            id="autoReload"
            v-model="settings.autoReloadEnabled"
            type="checkbox"
          />
          <label for="autoReload" style="margin: 0">
            アイドル時に自動リロードする
          </label>
        </div>

        <div class="row">
          <div class="field">
            <label for="idle">
              アイドル閾値（秒）
              <span class="hint">最後の操作からこの秒数で「アイドル」</span>
            </label>
            <input
              id="idle"
              v-model.number="settings.idleThresholdSecs"
              type="number"
              min="5"
            />
          </div>
          <div class="field">
            <label for="interval">
              チェック間隔（秒）
              <span class="hint">アイドル判定の確認頻度</span>
            </label>
            <input
              id="interval"
              v-model.number="settings.reloadCheckIntervalSecs"
              type="number"
              min="5"
            />
          </div>
        </div>

        <div class="field">
          <label for="css">
            注入する CSS
            <span class="hint">ロード後に &lt;style&gt; として適用</span>
          </label>
          <textarea
            id="css"
            v-model="settings.customCss"
            spellcheck="false"
            placeholder="/* 例: ヘッダーを隠す */&#10;header { display: none; }"
          ></textarea>
        </div>

        <div class="field">
          <label for="js">
            注入する JS
            <span class="hint">各ページロード後に実行（変更は Jira を開き直すと反映）</span>
          </label>
          <textarea
            id="js"
            v-model="settings.customJs"
            spellcheck="false"
            placeholder="// 例: console.log('jirapp injected');"
          ></textarea>
        </div>
      </details>

      <div class="actions">
        <button :disabled="busy" @click="saveAndClose">
          保存して閉じる
        </button>
        <button class="secondary" :disabled="busy" @click="cancel">
          キャンセル
        </button>
        <span class="status" :class="{ error: isError }">{{ status }}</span>

        <!-- バージョン表記以降（更新確認系）は margin-left:auto で右寄せ -->
        <span class="version">バージョン {{ updater.appVersion.value || "—" }}</span>
        <button
          class="icon-btn"
          title="GitHub リポジトリを開く"
          aria-label="GitHub リポジトリを開く"
          @click="openRepo"
        >
          <svg viewBox="0 0 16 16" width="18" height="18" aria-hidden="true">
            <path
              fill="currentColor"
              d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
            />
          </svg>
        </button>
        <button
          class="secondary"
          :disabled="updater.isBusy.value"
          @click="() => updater.checkForUpdate()"
        >
          更新を確認
        </button>
        <button
          v-if="updater.state.value === 'available'"
          :disabled="updater.isBusy.value"
          @click="updater.downloadAndInstall"
        >
          v{{ updater.updateVersion.value }} に更新して再起動
        </button>
        <span
          class="status"
          :class="{ error: updater.state.value === 'error' }"
          >{{ updaterStatusText }}</span
        >
      </div>
    </template>

    <p v-else class="status">読み込み中…</p>
  </main>
</template>
