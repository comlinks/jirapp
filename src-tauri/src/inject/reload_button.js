// 左下フローティングのリロードボタン（issue #26）。画面左下に小さな円形ボタンを常駐させ、
// クリックで location.reload() する（F5 リロード reload_shortcut.js／システムメニュー「再読み込み」
// と同じフルロード経路）。マウス操作だけで手早くリロードしたい用途向け。基盤 machinery.js の
// window.JIRAPP プラットフォームに registerFeature で登録し、addStyle を共有利用する（状態を
// 持たないので store は使わない）。
//
// 設計の要点:
//  - 配置: position:fixed で左下角（left/bottom 固定）。z-index を高くして Jira の UI より前面へ。
//    SPA 遷移で消えないよう、ボタンは <body> 直下に一度だけ置き、MutationObserver で消えたら貼り直す。
//  - 邪魔にならない配慮: 既定 opacity を下げ、hover/focus でのみ不透明にする。色は Atlassian の
//    デザイントークンに追従（card_key_copy.js と同方針）。ライト/ダーク両テーマで馴染む。
//  - リロード: click で location.reload()。フルロードなので document-start 注入も再実行される。
JIRAPP.registerFeature("reloadButton", function (app) {
  var BTN_ID = "__jirapp-reload-btn";

  // 円形の更新アイコン（Atlassian トークン色に追従）。
  var RELOAD_SVG =
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" ' +
    'stroke-linecap="round" stroke-linejoin="round">' +
    '<path d="M21 12a9 9 0 1 1-2.64-6.36"/><polyline points="21 3 21 9 15 9"/></svg>';

  app.addStyle(
    "__jirapp_reload_btn_style__",
    "#" + BTN_ID + "{position:fixed;left:16px;bottom:16px;z-index:2147483000;" +
    "width:48px;height:48px;display:inline-flex;align-items:center;justify-content:center;" +
    "box-sizing:border-box;border:1px solid var(--ds-border,rgba(9,30,66,.14));padding:0;" +
    "border-radius:50%;cursor:pointer;opacity:.6;transition:opacity .12s,background .12s;" +
    "background:var(--ds-surface-overlay,#fff);color:var(--ds-text-subtle,#626f86);" +
    "box-shadow:var(--ds-shadow-overlay,0 1px 4px rgba(9,30,66,.25));}\n" +
    "#" + BTN_ID + ":hover{opacity:1;background:var(--ds-background-neutral-hovered,rgba(9,30,66,.06));" +
    "color:var(--ds-text,#172b4d);}\n" +
    "#" + BTN_ID + ":focus-visible{opacity:1;outline:2px solid var(--ds-border-focused,#388bff);" +
    "outline-offset:2px;}\n" +
    "#" + BTN_ID + " svg{width:26px;height:26px;pointer-events:none;}"
  );

  function ensureButton() {
    if (document.getElementById(BTN_ID)) return;
    var btn = document.createElement("button");
    btn.type = "button";
    btn.id = BTN_ID;
    btn.setAttribute("aria-label", "再読み込み");
    btn.title = "再読み込み";
    btn.innerHTML = RELOAD_SVG;
    btn.addEventListener("click", function (ev) {
      ev.preventDefault();
      location.reload();
    });
    document.body.appendChild(btn);
  }

  // 常駐監視: 万一ボタンが取り除かれても貼り直す。ボタンは <body> 直下に置くので、body の
  // 直接の子の変化だけ見れば十分（subtree 監視は不要）。ensureButton は id ガードで冪等なので
  // コールバックは直接呼んでよい（O(1)。debounce も不要）。
  var mo = new MutationObserver(ensureButton);

  ensureButton();
  mo.observe(document.body, { childList: true });
});
