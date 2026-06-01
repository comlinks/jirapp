import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { computed, markRaw, type Raw, ref } from "vue";

// 更新フローの状態。
//  idle: 未確認 / checking: 確認中 / available: 更新あり /
//  downloading: ダウンロード+適用中 / upToDate: 最新 / error: 失敗
type UpdateState =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "upToDate"
  | "error";

// モジュールスコープに置き、設定ウィンドウ内で状態を共有する（複数箇所から使っても一貫させる）。
const state = ref<UpdateState>("idle");
// Update オブジェクトは Vue のリアクティブ Proxy で包むと内部メソッドが壊れるため markRaw で保持する。
const pendingUpdate = ref<Raw<Update> | null>(null);
const updateVersion = ref("");
const appVersion = ref("");
const errorMessage = ref("");

let versionLoaded = false;

export function useUpdater() {
  if (!versionLoaded) {
    versionLoaded = true;
    getVersion()
      .then((v) => (appVersion.value = v))
      .catch(() => {});
  }

  const isBusy = computed(
    () => state.value === "checking" || state.value === "downloading",
  );

  async function checkForUpdate() {
    state.value = "checking";
    errorMessage.value = "";
    try {
      const update = await check();
      if (update) {
        pendingUpdate.value = markRaw(update);
        updateVersion.value = update.version;
        state.value = "available";
      } else {
        state.value = "upToDate";
      }
    } catch (e) {
      errorMessage.value = String(e);
      state.value = "error";
    }
  }

  async function downloadAndInstall() {
    if (!pendingUpdate.value) return;
    state.value = "downloading";
    errorMessage.value = "";
    try {
      await pendingUpdate.value.downloadAndInstall();
      // インストール後に再起動して新バージョンを起動する。
      await relaunch();
    } catch (e) {
      errorMessage.value = String(e);
      state.value = "error";
    }
  }

  return {
    state,
    appVersion,
    updateVersion,
    errorMessage,
    isBusy,
    checkForUpdate,
    downloadAndInstall,
  };
}
