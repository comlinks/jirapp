// 列ヘッダ着色機能（issue #21）。カンバン列ヘッダの背景色をステータス名ごとに変更し、
// 列の ⋯（その他の操作）メニューに「色の変更」を追加する。基盤 machinery.js の
// window.JIRAPP プラットフォームに登録し、store / addStyle を共有利用する。
//
// 設計の要点（いずれも実機検証済み。詳細は開発メモ jira-column-color-dom 参照）:
//  - 着色方式: 列ヘッダ既定のグレーは Jira 自身のインライン
//    `background-color: var(--project-color-elevation-surface-sunken)`（非 important）。
//    これを直接書き換えるとクリア時に透明化するため触らない。代わりに注入 <style> の
//    `[data-jirapp-col="<hue>"]{...!important}` と header-container への `data-jirapp-col`
//    属性で上書きする。`!important` は非 important インラインに勝つ。クリアは属性を外すだけで
//    既定グレーが自動復帰する。
//  - 列の識別: 位置（nth-child）は並べ替え・増減で崩れるため、安定した data-testid を辿り、
//    ステータス名をキーにする。
//  - 永続化: JIRAPP.store（iframe 経由 native localStorage）に名前→hue マップを保存する。
//    Jira ウィンドウは IPC を持たず設定ストアへは書けないため、この WebView 内保存を用いる。
//  - 常駐: SPA 遷移や再描画で属性が失われても MutationObserver で貼り直す。マップはメモリに
//    キャッシュし、保存時のみ更新する（再適用のたびに localStorage を読み直さない）。
JIRAPP.registerFeature("columnColor", function (app) {
  var STORE_KEY = "jirapp.columnColors.v1";

  // Jira の accent パレット名（hue）と日本語ラベル。--ds-background-accent-<hue>-subtlest に対応。
  var HUES = [
    ["gray", "グレー"], ["red", "レッド"], ["orange", "オレンジ"], ["yellow", "イエロー"],
    ["lime", "ライム"], ["green", "グリーン"], ["teal", "ティール"], ["blue", "ブルー"],
    ["purple", "パープル"], ["magenta", "マゼンタ"]
  ];

  // カンバン DOM の安定 testid。
  var T_WRAP = "platform-board-kit.ui.column.draggable-column.styled-wrapper";
  var T_HDR = "platform-board-kit.common.ui.column-header.header.column-header-container";
  var T_NAME = "platform-board-kit.common.ui.column-header.editable-title.column-title.column-name";
  var T_TRIG = "software-board.board-container.board.column.header.menu.column-menu-trigger";
  // 列メニュー（.atlaskit-portal）判定に使う既定項目 testid の接頭辞。
  var MENU_ITEM_PREFIX = "software-board.board-container.board.column.header.menu.item-";

  function sel(t) {
    return '[data-testid="' + t + '"]';
  }

  // 名前→hue マップはメモリに保持し、保存時のみ更新する（再適用ごとの読み直しを避ける）。
  var map = app.store.get(STORE_KEY, {}) || {};
  function save() {
    app.store.set(STORE_KEY, map);
  }

  // 着色スタイルシートを一度だけ用意する（hue ごとの !important ルール）。
  var css = "";
  HUES.forEach(function (h) {
    css += sel(T_HDR) + '[data-jirapp-col="' + h[0] + '"]' +
      "{background-color:var(--ds-background-accent-" + h[0] + "-subtlest)!important;}\n";
  });
  app.addStyle("__jirapp_col_style__", css);

  function columnName(wrap) {
    var n = wrap.querySelector(sel(T_NAME));
    return n ? (n.textContent || "").trim() : "";
  }

  // 保存済みマップに従い、全列の header-container へ属性を反映する。
  function applyAll() {
    var wraps = document.querySelectorAll(sel(T_WRAP));
    for (var i = 0; i < wraps.length; i++) {
      var hdr = wraps[i].querySelector(sel(T_HDR));
      if (!hdr) continue;
      var hue = map[columnName(wraps[i])];
      if (hue) hdr.setAttribute("data-jirapp-col", hue);
      else hdr.removeAttribute("data-jirapp-col");
    }
  }

  // ⋯ をクリックした列を控える（メニューはポータルへ分離描画されるため、開いた瞬間に対象列を記録）。
  var lastColName = "";
  var lastTrigger = null;
  document.addEventListener("click", function (ev) {
    var t = ev.target && ev.target.closest ? ev.target.closest(sel(T_TRIG)) : null;
    if (!t) return;
    var wrap = t.closest(sel(T_WRAP));
    lastColName = wrap ? columnName(wrap) : "";
    lastTrigger = t;
  }, true);

  // --- 自前パレットのポップアップ ---
  function closePalette() {
    var p = document.getElementById("__jirapp_col_pop__");
    if (p) p.remove();
    document.removeEventListener("mousedown", onOutside, true);
  }
  function onOutside(ev) {
    var p = document.getElementById("__jirapp_col_pop__");
    if (p && !p.contains(ev.target)) closePalette();
  }
  function openPalette(anchor, name) {
    closePalette();
    if (!name) return;
    var pop = document.createElement("div");
    pop.id = "__jirapp_col_pop__";
    pop.style.cssText =
      "position:fixed;z-index:2147483647;background:var(--ds-surface-overlay,#fff);" +
      "border:1px solid var(--ds-border,rgba(9,30,66,.14));border-radius:6px;" +
      "box-shadow:0 4px 12px rgba(9,30,66,.25);padding:8px;width:184px;box-sizing:border-box;" +
      "display:flex;flex-wrap:wrap;gap:6px;";
    var r = anchor.getBoundingClientRect();
    pop.style.left = Math.max(4, Math.min(r.left, window.innerWidth - 190)) + "px";
    pop.style.top = Math.min(r.bottom + 4, window.innerHeight - 120) + "px";

    HUES.forEach(function (h) {
      var sw = document.createElement("button");
      sw.type = "button";
      sw.title = h[1];
      sw.style.cssText =
        "width:28px;height:28px;border-radius:4px;cursor:pointer;padding:0;border:2px solid " +
        (map[name] === h[0] ? "var(--ds-border-selected,#0c66e4)" : "transparent") +
        ";background-color:var(--ds-background-accent-" + h[0] + "-subtlest);";
      sw.addEventListener("click", function () {
        map[name] = h[0];
        save();
        applyAll();
        closePalette();
      });
      pop.appendChild(sw);
    });

    var clr = document.createElement("button");
    clr.type = "button";
    clr.textContent = "クリア（色なし）";
    clr.style.cssText =
      "width:100%;margin-top:2px;padding:6px;cursor:pointer;font-size:12px;" +
      "border:1px solid var(--ds-border,rgba(9,30,66,.14));border-radius:4px;background:transparent;" +
      "color:var(--ds-text,#172b4d);";
    clr.addEventListener("click", function () {
      delete map[name];
      save();
      applyAll();
      closePalette();
    });
    pop.appendChild(clr);

    document.body.appendChild(pop);
    // 直後の同一クリックで即閉じないよう、リスナ登録は次サイクルへ回す。
    setTimeout(function () {
      document.addEventListener("mousedown", onOutside, true);
    }, 0);
  }

  // --- 列メニュー（.atlaskit-portal）へ「色の変更」項目を注入 ---
  function injectMenuItem(portal) {
    if (!portal || portal.querySelector("[data-jirapp-menuitem]")) return;
    var items = portal.querySelectorAll('[role="menuitem"]');
    if (!items.length) return;
    // 列メニューか（既定項目の testid 接頭辞を持つか）で判定する。誤爆防止。
    var isColMenu = false;
    var tmpl = items[0];
    for (var i = 0; i < items.length; i++) {
      var tid = items[i].getAttribute("data-testid") || "";
      if (tid.indexOf(MENU_ITEM_PREFIX) === 0) {
        isColMenu = true;
        // 削除（危険色の可能性）以外を複製元にする。
        if (tid.indexOf("item-delete") < 0) tmpl = items[i];
      }
    }
    if (!isColMenu) return;

    var name = lastColName;
    var anchor = lastTrigger;
    var mi = tmpl.cloneNode(true); // クローンは React の fiber 外なので既定項目のハンドラは発火しない。
    mi.setAttribute("data-jirapp-menuitem", "1");
    mi.removeAttribute("data-testid");
    // 表示ラベルだけ差し替える（アイコン等の構造は保つ）。
    var walker = document.createTreeWalker(mi, NodeFilter.SHOW_TEXT, null);
    var tn = walker.nextNode();
    if (tn) tn.nodeValue = "色の変更";
    else mi.textContent = "色の変更";
    mi.addEventListener("click", function (ev) {
      ev.preventDefault();
      ev.stopPropagation();
      openPalette(anchor || mi, name);
      // Jira の列メニューを閉じる（パレットは body 直下の自前要素なので影響を受けない）。
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    }, true);
    tmpl.parentNode.appendChild(mi);
  }

  // --- 常駐監視 ---
  // 着色は「列に関係する変化」があったときだけ貼り直す（無関係な SPA 変化で全列再走査しない）。
  // メニュー注入はポータル追加を拾う（ポータルは概ね body 直下に mount される）。
  var applyPending = false;
  function scheduleApply() {
    if (applyPending) return;
    applyPending = true;
    setTimeout(function () {
      applyPending = false;
      applyAll();
    }, 50);
  }
  var mo = new MutationObserver(function (muts) {
    var relevant = false;
    for (var i = 0; i < muts.length; i++) {
      var added = muts[i].addedNodes;
      for (var j = 0; j < added.length; j++) {
        var node = added[j];
        if (!node || node.nodeType !== 1) continue;
        if (node.classList && node.classList.contains("atlaskit-portal")) {
          injectMenuItem(node);
          continue;
        }
        if (relevant || !node.matches) continue;
        // 列そのもの、または列を内包するノードが追加されたときだけ再適用する。
        if (node.matches(sel(T_WRAP)) || node.matches(sel(T_HDR)) || node.querySelector(sel(T_HDR))) {
          relevant = true;
        }
      }
    }
    if (relevant) scheduleApply();
  });

  applyAll();
  mo.observe(document.body, { childList: true, subtree: true });
});
