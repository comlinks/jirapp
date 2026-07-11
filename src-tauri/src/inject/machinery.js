// 基盤 JS ＝ 注入機能の共通プラットフォーム。
//
// Jira ウィンドウへ document-start でネイティブ注入する（initialization_script）。
// CSP の影響を受けにくく、各フルロードのたびに他のページスクリプトより先に走る。
//
// 役割:
//   - アイドル検知＋アイドル時の自動リロード
//   - ユーザー CSS 適用と、Rust からの設定反映（window.__JIRAPP_APPLY__）
//   - 各注入機能が乗る土台 window.JIRAPP を用意する:
//       JIRAPP.registerFeature(name, fn) … 機能を一度だけ登録し DOM 準備後に fn(JIRAPP) 実行
//       JIRAPP.store.get/set(key, ...)   … native localStorage（iframe 経由）による永続化
//       JIRAPP.addStyle(id, css)         … id 付き <style> の作成/更新
//       JIRAPP.onConfig(cb)              … Rust から届く設定（customCss 等）の購読
//
// 注意（SPA）: initialization_script はフルナビゲーション時のみ再実行され、クライアント側の
// ルート遷移では走らない。遷移に追従したい処理は各機能側で MutationObserver / setInterval で
// 常駐させ、多重実行は登録ガードで防ぐこと。
(function () {
  if (window.__JIRAPP_INSTALLED__) return;
  window.__JIRAPP_INSTALLED__ = true;

  // 既定設定（Rust 側の __JIRAPP_APPLY__ 呼び出しで上書きされる）。
  window.__JIRAPP_CONFIG__ = window.__JIRAPP_CONFIG__ || {
    autoReloadEnabled: false,
    idleThresholdSecs: 300,
    reloadCheckIntervalSecs: 30,
    customCss: ""
  };

  // ============================================================
  //  window.JIRAPP — 注入機能の共通プラットフォーム
  // ============================================================
  var installed = {};       // 機能名 -> true（多重登録防止）
  var configListeners = []; // onConfig 購読者

  // --- native localStorage（about:blank iframe 経由）---
  // top の window.localStorage は Atlassian のライブラリがメモリシムに差し替えるため、
  // 直書きは再読込で蒸発する。同一オリジンの hidden iframe から native ストアへ読み書きすれば
  // 永続化する（Atlassian 自身も早期に捕まえた native 参照へ書いている）。
  var lsFrame = null;
  function nativeLS() {
    try {
      if (lsFrame && lsFrame.contentWindow) return lsFrame.contentWindow.localStorage;
      var f = document.createElement("iframe");
      f.setAttribute("aria-hidden", "true");
      f.style.display = "none";
      (document.body || document.documentElement).appendChild(f);
      lsFrame = f;
      return f.contentWindow.localStorage;
    } catch {
      return null;
    }
  }

  var JIRAPP = {
    // 永続ストア（JSON 値）。get は未保存なら fallback を返す。
    store: {
      get: function (key, fallback) {
        try {
          var ls = nativeLS();
          if (!ls) return fallback;
          var raw = ls.getItem(key);
          return raw == null ? fallback : JSON.parse(raw);
        } catch {
          return fallback;
        }
      },
      set: function (key, val) {
        try {
          var ls = nativeLS();
          if (ls) ls.setItem(key, JSON.stringify(val));
        } catch {}
      }
    },

    // id 付き <style> を作成/更新する。css が null/undefined なら内容は変えず要素だけ返す。
    addStyle: function (id, css) {
      var el = document.getElementById(id);
      if (!el) {
        el = document.createElement("style");
        el.id = id;
        (document.head || document.documentElement).appendChild(el);
      }
      if (css != null) el.textContent = css;
      return el;
    },

    // Rust から届く設定（__JIRAPP_CONFIG__）を購読する。登録時に現在値で即コールバック。
    onConfig: function (cb) {
      configListeners.push(cb);
      try {
        cb(window.__JIRAPP_CONFIG__);
      } catch {}
    },

    // 機能を一度だけ登録し、DOM 準備後に fn(JIRAPP) を実行する。
    registerFeature: function (name, fn) {
      if (installed[name]) return;
      installed[name] = true;
      function run() {
        try {
          fn(JIRAPP);
        } catch (e) {
          console.error("[jirapp] feature '" + name + "' error", e);
        }
      }
      if (document.body) run();
      else document.addEventListener("DOMContentLoaded", run);
    }
  };
  window.JIRAPP = JIRAPP;

  // ============================================================
  //  基盤機能: アイドル検知・自動リロード・ユーザー CSS 適用
  // ============================================================

  // --- アイドル検知: 最後のユーザー操作時刻を記録 ---
  var lastActivity = Date.now();
  function touch() {
    lastActivity = Date.now();
  }
  ["mousemove", "mousedown", "keydown", "scroll", "wheel", "touchstart"].forEach(function (ev) {
    window.addEventListener(ev, touch, { passive: true, capture: true });
  });

  // --- アイドル時の自動リロード ---
  var reloadTimer = null;
  function scheduleReload() {
    if (reloadTimer) {
      clearInterval(reloadTimer);
      reloadTimer = null;
    }
    var cfg = window.__JIRAPP_CONFIG__;
    if (!cfg.autoReloadEnabled) return;
    var intervalMs = Math.max(5, cfg.reloadCheckIntervalSecs | 0) * 1000;
    reloadTimer = setInterval(function () {
      var c = window.__JIRAPP_CONFIG__;
      if (!c.autoReloadEnabled) return;
      if (Date.now() - lastActivity >= Math.max(5, c.idleThresholdSecs | 0) * 1000) {
        lastActivity = Date.now(); // 連続リロード防止
        location.reload();
      }
    }, intervalMs);
  }

  // 設定適用のエントリポイント（Rust が push_config_script 経由で呼ぶ）。
  window.__JIRAPP_APPLY__ = function (cfg) {
    if (cfg) window.__JIRAPP_CONFIG__ = cfg;
    JIRAPP.addStyle("__jirapp_user_css__", window.__JIRAPP_CONFIG__.customCss || "");
    scheduleReload();
    for (var i = 0; i < configListeners.length; i++) {
      try {
        configListeners[i](window.__JIRAPP_CONFIG__);
      } catch {}
    }
  };

  // DOM 準備時に既定設定で一度適用しておく。
  function bootstrap() {
    window.__JIRAPP_APPLY__();
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bootstrap);
  } else {
    bootstrap();
  }
})();
