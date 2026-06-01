<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Settings } from "./types";
import {
  applyToJiraWindow,
  closeSettingsWindow,
  getSettings,
  hideSettingsWindow,
  isJiraOpen,
  openJiraWindow,
  saveSettings,
} from "./api";
import { useUpdater } from "./composables/useUpdater";

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

onMounted(async () => {
  try {
    Object.assign(settings, await getSettings());
  } catch (e) {
    setStatus(`設定の読み込みに失敗: ${e}`, true);
  } finally {
    loading.value = false;
  }
  await refreshJiraOpen();
  // Rust 側（メニューからの再表示・Jira クローズ）からの状態更新通知でボタン表示を追従させる。
  unlisten = await listen("settings:refresh", () => {
    setStatus("");
    refreshJiraOpen();
  });
});

onUnmounted(() => {
  unlisten?.();
  unlisten = null;
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

      <div class="actions">
        <button :disabled="busy" @click="saveAndClose">
          保存して閉じる
        </button>
        <button class="secondary" :disabled="busy" @click="cancel">
          キャンセル
        </button>
        <span class="status" :class="{ error: isError }">{{ status }}</span>
      </div>

      <div class="updater">
        <span class="version">バージョン {{ updater.appVersion.value || "—" }}</span>
        <button
          class="secondary"
          :disabled="updater.isBusy.value"
          @click="updater.checkForUpdate"
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
