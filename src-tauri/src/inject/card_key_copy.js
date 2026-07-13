// チケットキーのコピー機能（issue #22）。カンバンカードのキー "COM-123" の隣に、
// カードホバーで現れる小さなコピーボタンを足し、クリックでキー文字列をクリップボードへ
// コピーする。基盤 machinery.js の window.JIRAPP プラットフォームに registerFeature で登録し、
// addStyle を共有利用する（永続状態は持たないので store は使わない）。
//
// 設計の要点（実機 DOM を devtools で確認済み。詳細は開発メモ jira-card-key-copy-dom 参照）:
//  - キー要素: data-testid="platform-card.common.ui.key.key"（中に /browse/KEY への <a target=_blank>）。
//    既存リンク（クリックでチケットを開く）は壊さず、その隣にボタンを足すだけにする。
//  - 配置: キーの grid 列は content 幅に固定されるため、ボタンを inline で足すと折り返す。
//    キー div を position:relative にし、ボタンを position:absolute; left:100% でテキスト右へ
//    浮かせる（レイアウト非破壊）。ボタンは <a> の外に置くのでリンク遷移は誘発しない。
//  - クリップ回避（重要）: キー div は Jira 側スタイルで overflow:hidden。left:100% のボタンは
//    キーの箱の外側に出るため、そのままだと opacity に関係なく切り取られて不可視になる（実機で
//    「ホバーしても出ない」の真因）。`overflow:visible !important` で上書きして表示させる（非
//    important では Jira 側に負けるため !important 必須。キー列は content 幅なので副作用はない）。
//  - 表示: 既定 opacity:0。カードラッパ card-with-icc の :hover でのみ表示（キーボード focus でも）。
//  - コピー: navigator.clipboard.writeText（secure context で可）。失敗時は textarea + execCommand。
//    クリックはユーザージェスチャなので clipboard API のフォーカス要件を満たす。
//  - 常駐: SPA 再描画でカード（＝キー）が再生成されても MutationObserver で貼り直す。多重付与は
//    キー内の既存ボタン有無で防ぐ。
JIRAPP.registerFeature("cardKeyCopy", function (app) {
  // カンバン DOM の安定 testid。
  var T_KEY = "platform-card.common.ui.key.key";
  var T_CARD = "software-board.board-container.board.card-container.card-with-icc";

  function sel(t) {
    return '[data-testid="' + t + '"]';
  }

  // アイコン（Atlassian のトークン色に追従。copy = 2枚重ねの矩形 / check = チェックマーク）。
  var COPY_SVG =
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" ' +
    'stroke-linecap="round" stroke-linejoin="round">' +
    '<rect x="9" y="9" width="13" height="13" rx="2"/>' +
    '<path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>';
  var CHECK_SVG =
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" ' +
    'stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>';

  app.addStyle(
    "__jirapp_card_copy_style__",
    sel(T_KEY) + "{position:relative;overflow:visible !important;}\n" +
    ".__jirapp-copybtn{position:absolute;left:100%;top:50%;transform:translateY(-50%);" +
    "margin-left:2px;display:inline-flex;align-items:center;justify-content:center;" +
    "width:18px;height:18px;box-sizing:border-box;border:0;padding:0;background:transparent;" +
    "cursor:pointer;border-radius:3px;opacity:0;transition:opacity .1s;" +
    "color:var(--ds-text-subtle,#626f86);}\n" +
    sel(T_CARD) + ":hover .__jirapp-copybtn{opacity:1;}\n" +
    ".__jirapp-copybtn:focus-visible{opacity:1;outline:2px solid var(--ds-border-focused,#388bff);}\n" +
    ".__jirapp-copybtn:hover{background:var(--ds-background-neutral-hovered,rgba(9,30,66,.08));" +
    "color:var(--ds-text,#172b4d);}\n" +
    ".__jirapp-copybtn.is-copied{color:var(--ds-icon-success,#22a06b);opacity:1;}\n" +
    ".__jirapp-copybtn svg{width:13px;height:13px;pointer-events:none;}"
  );

  // キーの表示文字列。ボタンは <a> の外に append するので、リンク文字列だけを読む。
  function keyText(key) {
    var a = key.querySelector("a");
    return ((a || key).textContent || "").trim();
  }

  function fallbackCopy(text) {
    try {
      var ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.top = "-1000px";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.focus();
      ta.select();
      document.execCommand("copy");
      ta.remove();
    } catch {}
  }

  function copyText(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text).catch(function () {
        fallbackCopy(text);
      });
    }
    fallbackCopy(text);
    return Promise.resolve();
  }

  // コピー成功の一時フィードバック（チェックマークへ変えて約1.2秒後に戻す）。
  function flash(btn) {
    btn.classList.add("is-copied");
    btn.innerHTML = CHECK_SVG;
    clearTimeout(btn.__jirappTimer);
    btn.__jirappTimer = setTimeout(function () {
      btn.classList.remove("is-copied");
      btn.innerHTML = COPY_SVG;
    }, 1200);
  }

  function addButton(key) {
    if (key.querySelector(".__jirapp-copybtn")) return;
    var initial = keyText(key);
    if (!initial) return;
    var btn = document.createElement("button");
    btn.type = "button";
    btn.className = "__jirapp-copybtn";
    btn.setAttribute("aria-label", "チケットキーをコピー");
    btn.title = "キーをコピー: " + initial;
    btn.innerHTML = COPY_SVG;
    btn.addEventListener(
      "click",
      function (ev) {
        ev.preventDefault();
        ev.stopPropagation();
        // カード再利用で文字列が変わりうるので、クリック時に最新のキーを読む。
        var text = keyText(key);
        if (!text) return;
        Promise.resolve(copyText(text)).then(function () {
          flash(btn);
        });
      },
      true
    );
    // 押下がカードのドラッグ開始やクリック（詳細を開く）に伝播しないようにする。
    btn.addEventListener("pointerdown", function (ev) {
      ev.stopPropagation();
    }, true);
    btn.addEventListener("mousedown", function (ev) {
      ev.stopPropagation();
    }, true);
    key.appendChild(btn);
  }

  function addAll() {
    var keys = document.querySelectorAll(sel(T_KEY));
    for (var i = 0; i < keys.length; i++) addButton(keys[i]);
  }

  // 常駐監視: カード（＝キー）の追加・再描画があったときだけ貼り直す。
  var pending = false;
  function schedule() {
    if (pending) return;
    pending = true;
    setTimeout(function () {
      pending = false;
      addAll();
    }, 50);
  }
  var mo = new MutationObserver(function (muts) {
    for (var i = 0; i < muts.length; i++) {
      var added = muts[i].addedNodes;
      for (var j = 0; j < added.length; j++) {
        var node = added[j];
        if (!node || node.nodeType !== 1 || !node.matches) continue;
        if (node.matches(sel(T_KEY)) || (node.querySelector && node.querySelector(sel(T_KEY)))) {
          schedule();
          return;
        }
      }
    }
  });

  addAll();
  mo.observe(document.body, { childList: true, subtree: true });
});
