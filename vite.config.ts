import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// @tauri-apps/cli が起動するときに設定する環境変数
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],

  // Tauri はカスタムスキームで動くため相対パスにする
  clearScreen: false,
  server: {
    // pike など他の Tauri アプリ（既定 1420）と同時にデバッグしても衝突しないよう変更。
    port: 1430,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1431,
        }
      : undefined,
    watch: {
      // src-tauri は Vite の監視対象から外す
      ignored: ["**/src-tauri/**"],
    },
  },
}));
