<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { Settings } from "./types";
import {
  applyToJiraWindow,
  getSettings,
  hideSettingsWindow,
  isJiraOpen,
  openJiraWindow,
  saveSettings,
} from "./api";

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

function setStatus(msg: string, error = false) {
  status.value = msg;
  isError.value = error;
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
  await listen("settings:refresh", () => {
    setStatus("");
    refreshJiraOpen();
  });
});

async function save() {
  busy.value = true;
  try {
    await saveSettings({ ...settings });
    await applyToJiraWindow();
    setStatus("保存しました（Jira ウィンドウへ適用済み）");
  } catch (e) {
    setStatus(`保存に失敗: ${e}`, true);
  } finally {
    busy.value = false;
  }
}

async function openJira() {
  busy.value = true;
  try {
    await saveSettings({ ...settings });
    await openJiraWindow();
    jiraOpen.value = true;
    setStatus("Jira ウィンドウを開きました");
  } catch (e) {
    setStatus(`Jira ウィンドウを開けません: ${e}`, true);
  } finally {
    busy.value = false;
  }
}

async function closeSettings() {
  busy.value = true;
  try {
    await hideSettingsWindow();
  } catch (e) {
    setStatus(`設定を閉じられません: ${e}`, true);
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
          <span class="hint">例: https://your-domain.atlassian.net</span>
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
          <span class="hint">各ページロード後に実行</span>
        </label>
        <textarea
          id="js"
          v-model="settings.customJs"
          spellcheck="false"
          placeholder="// 例: console.log('jirapp injected');"
        ></textarea>
      </div>

      <div class="actions">
        <button v-if="!jiraOpen" :disabled="busy" @click="openJira">
          Jira を開く
        </button>
        <button v-else :disabled="busy" @click="closeSettings">
          設定を閉じる
        </button>
        <button class="secondary" :disabled="busy" @click="save">
          保存して適用
        </button>
        <span class="status" :class="{ error: isError }">{{ status }}</span>
      </div>
    </template>

    <p v-else class="status">読み込み中…</p>
  </main>
</template>
