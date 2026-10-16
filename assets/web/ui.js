// Terrafolk 채팅 독 — 주민 선택(Rust 픽킹 tf.onPickAgent ⇄ tf.selectAgent,
// src/world/pick.rs) 시 하단에 뜨는 대화 UI(FE ChatBar §17.76 포트).
// 연결은 worldsim.js(tf.ui), i18n·MT는 tf-i18n.js(TF_I18N) — 본 스크립트는
// ui.js → tf-i18n.js 로드 순서(늦음) 때문에 TF_I18N을 사용 시점에 해석한다.
// 대화 표시 규칙(§17.67) — 발화 로케일=내 로케일이면 원문, en 화면의 비-en
// 발화는 영문 정본, 나머지는 열람 시점 MT(디바운스 배치 + 공유 캐시).
(function () {
  'use strict';
  var tf = window.terrafolk;
  if (!tf || !tf.ui) return;

  var MAX_ROWS = 240;

  function esc(s) {
    return String(s == null ? '' : s).replace(/[&<>"]/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c];
    });
  }
  function I() { return window.TF_I18N || null; }
  function t(key, params, fallback) {
    var i = I();
    return i ? i.t(key, params) : (fallback || key);
  }
  function tOr(key, fallback) {
    var i = I();
    return i ? i.tOr(key, fallback) : (fallback || key);
  }
  function locale() {
    var i = I();
    return i ? i.getLocale() : 'en';
  }

  // ── 스타일 — tf-ui.css(tf-chatdock)가 원본. 로드 순서 독립용 최소 첨언:
  // 독 루트 위치·키보드 오프셋만 여기서 보증한다.
  var st = document.createElement('style');
  st.textContent =
    '.tf-chatdock{position:fixed;z-index:30;left:50%;transform:translateX(-50%);' +
    'bottom:calc(.75rem + var(--tf-kb,0px) + env(safe-area-inset-bottom,0px));' +
    'width:min(42.5rem,calc(100% - 1rem));max-height:86dvh;display:none;' +
    'flex-direction:column;gap:.25rem;pointer-events:none;}' +
    '.tf-chatdock.on{display:flex;}' +
    '.tf-chatdock>*{pointer-events:auto;}' +
    'body.tf-chat-on #tf-hud{bottom:calc(6.5rem + env(safe-area-inset-bottom,0px));}' +
    '@media (max-width:600px){.tf-chatdock{width:calc(100% - .75rem);}}';
  document.head.appendChild(st);

  // ── DOM — 로그 패널(접힘) · 헤더 칩(토글) · 입력 바
  var dock = document.createElement('div');
  dock.className = 'tf-chatdock';
  dock.innerHTML =
    '<div class="tf-chatlog-panel">' +
    '<div class="tf-chatlog tf-scroll"></div>' +
    '</div>' +
    '<button class="tf-chathead">' +
    '<span>💬</span><span class="nm">?</span><span class="act"></span>' +
    '<span class="caret">▴</span>' +
    '</button>' +
    '<div class="tf-inputbar">' +
    '<input class="tf-input" maxlength="500" autocomplete="off" enterkeyhint="send">' +
    '<button class="send" title="Send">➤</button>' +
    '<button class="close" title="Close">✕</button>' +
    '</div>';
  document.body.appendChild(dock);

  var c = {
    dock: dock,
    panel: dock.querySelector('.tf-chatlog-panel'),
    log: dock.querySelector('.tf-chatlog'),
    head: dock.querySelector('.tf-chathead'),
    nm: dock.querySelector('.nm'),
    act: dock.querySelector('.act'),
    caret: dock.querySelector('.caret'),
    input: dock.querySelector('.tf-input'),
    send: dock.querySelector('.send'),
    close: dock.querySelector('.close'),
  };

  var sel = null;        // 선택 agent id(null = 독 숨김)
  var rows = [];         // 전체 대화 행(FE chat) — 로그는 selChat 필터 뷰
  var logOpen = false;
  var stuck = true;      // 로그 하단 고정(위로 스크롤 중엔 해제)
  var backlogTried = false;

  function nameOf(id) { return tf.ui.names[id] || id; }

  function agentInfo(id) {
    return (tf.ui.agentInfo && tf.ui.agentInfo(id)) ||
      { name: nameOf(id), actType: '', status: '' };
  }

  /** FE chatText.isAlreadyKorean 포트 — §17.51 한글 원문 재번역 방지 */
  function isAlreadyKorean(s) {
    var h = (s.match(/[가-힣]/g) || []).length;
    var l = (s.match(/[A-Za-z]/g) || []).length;
    return h > 0 && h >= l * 3;
  }

  /** FE isHumanRow 포트 — speaker=이메일 로컬파트(또는 주민명 불일치)로 판정 */
  function isHumanRow(r) {
    if (r.speaker && r.speaker.indexOf('@') >= 0) return true;
    return r.speaker !== agentInfo(r.agent_id).name;
  }

  /** 선택 주민 대화(FE selChat) — 해당 주민 발화 + 사람 발언 전부 */
  function selChat() {
    return rows.filter(function (r) {
      return r.agent_id === sel || (r.speaker && r.speaker.indexOf('@') >= 0);
    });
  }

  // ── 로그 렌더 — 행별 DOM 노드(비동기 MT 패치용 참조 유지)
  function renderLog() {
    if (!sel) return;
    var i18n = I();
    var loc = locale();
    c.log.innerHTML = '';
    var list = selChat();
    list.forEach(function (r) {
      var human = r.mine || isHumanRow(r);
      var row = document.createElement('div');
      row.className = 'tf-chatrow' + (r.pending ? ' pending' : '') + (r.sys ? ' sys' : '');
      var who = document.createElement('span');
      who.className = 'who' + (human ? ' human' : '');
      who.textContent = r.mine ? 'me' : (r.speaker || nameOf(r.agent_id));
      if (!human && i18n) who.style.color = i18n.agentColor(r.agent_id);
      var tx = document.createElement('span');
      tx.className = 'tx';
      row.appendChild(who);
      row.appendChild(tx);
      var when = r.gmin != null ? r.gmin : null;
      if (when != null || r.ts) {
        var ts = document.createElement('span');
        ts.className = 'ts';
        ts.textContent = (i18n && when != null) ? i18n.fmtGmin(when)
          : (r.ts ? String(r.ts).slice(11, 16) : '');
        row.appendChild(ts);
      }
      if (r.sys) {
        tx.textContent = r.text;
      } else {
        // §17.67 — 동기 결과는 즉시, 나머지 로케일 조합은 MT 후 패치
        var lang = r.lang || 'en';
        var src = String(r.text || r.text_en || '');
        if (loc === lang) tx.textContent = src;
        else if (loc === 'en' && r.text_en) tx.textContent = String(r.text_en);
        else if (loc === 'ko' && isAlreadyKorean(src)) tx.textContent = src;
        else {
          tx.textContent = src;
          if (i18n) {
            i18n.translateText(loc, src).then(function (out) {
              if (out !== src && tx.textContent === src) tx.textContent = out;
            });
          }
        }
      }
      c.log.appendChild(row);
    });
    if (!list.length) {
      var empty = document.createElement('div');
      empty.className = 'tf-empty';
      empty.textContent = t('chat.log.empty', null, 'No conversation yet');
      c.log.appendChild(empty);
    }
    if (stuck) c.log.scrollTop = c.log.scrollHeight;
  }

  c.log.addEventListener('scroll', function () {
    stuck = c.log.scrollTop + c.log.clientHeight >= c.log.scrollHeight - 24;
  });

  // ── Rust 픽킹 연결 — 클릭/탭 선택·해제 통보 수신
  tf.onPickAgent = function (id) {
    sel = id || null;
    document.body.classList.toggle('tf-chat-on', !!sel);
    c.dock.classList.toggle('on', !!sel);
    if (!sel) {
      setLogOpen(false);
      c.input.blur();
      return;
    }
    renderHead();
    renderLog();
  };

  function renderHead() {
    var info = agentInfo(sel);
    var i18n = I();
    c.nm.textContent = info.name || nameOf(sel);
    if (i18n) c.nm.style.color = i18n.agentColor(sel);
    var act = info.actType || info.status || '';
    c.act.textContent = i18n && act ? i18n.tOr('act.' + act, act) : act;
    c.input.placeholder = t('chat.placeholder.talk', { name: info.name || sel },
      'Talk to ' + (info.name || sel) + '…');
  }

  function select(id) { if (tf.selectAgent) tf.selectAgent(id || ''); }

  function setLogOpen(open) {
    logOpen = open;
    c.panel.classList.toggle('on', open);
    c.head.classList.toggle('on', open);
    c.caret.textContent = open ? '▾' : '▴';
    tf.ui.logOpenFlag = open;
    if (open) {
      loadBacklog();
      stuck = true;
      renderLog();
    }
  }

  c.head.addEventListener('click', function () { setLogOpen(!logOpen); });
  c.close.addEventListener('click', function () { select(null); });

  // ── 대화 백로그 — 공개 REST, 실시간 수신분과 (대상|원문) 키로 중복 제거
  function loadBacklog() {
    if (backlogTried || !tf.ui.base) return;
    backlogTried = true;
    fetch(tf.ui.base + '/api/worldsim/chat')
      .then(function (res) { return res.ok ? res.json() : []; })
      .then(function (list) {
        if (!Array.isArray(list) || !list.length) return;
        var live = {};
        rows.forEach(function (r) { live[r.agent_id + '|' + r.text] = 1; });
        var back = list.filter(function (r) {
          return !live[r.agent_id + '|' + r.text];
        }).slice(-MAX_ROWS).map(function (r) {
          // 백로그 행 정규화 — 백엔드 game_min을 gmin으로(FE §17.37 규약)
          return { agent_id: r.agent_id, speaker: r.speaker, text: r.text,
            text_en: r.text_en, lang: r.lang,
            gmin: r.game_min != null ? r.game_min : null, ts: r.ts };
        });
        rows = back.concat(rows).slice(-MAX_ROWS);
        renderLog();
      })
      .catch(function () { backlogTried = false; });
  }

  // ── 실시간 수신 — 에코 확정(§17.55): pending 행을 서버 행으로 치환(이중 렌더 방지)
  tf.ui.onChatRow(function (m) {
    var echo = null;
    for (var i = 0; i < rows.length; i++) {
      var p = rows[i];
      if (!echo && p.pending && p.mine && p.agent_id === m.agent_id && p.text === m.text) {
        p.pending = false;
        p.gmin = m.gmin;
        p.ts = new Date().toISOString();
        echo = p;
      }
    }
    if (!echo) {
      rows.push({ agent_id: m.agent_id, speaker: m.speaker, text: m.text,
        text_en: m.text_en, lang: m.lang, gmin: m.gmin,
        ts: new Date().toISOString() });
    }
    if (rows.length > MAX_ROWS) rows.splice(0, rows.length - MAX_ROWS);
    if (logOpen) renderLog();
    if (m.agent_id === sel) renderHead();
  });

  // ── 송신 — 원문 에코 즉시 표시(§17.55), 방송 도착 시 확정, 15s 미수신 제거
  function send() {
    var text = c.input.value.trim().slice(0, 500);
    if (!text || !sel) return;
    if (!tf.ui.live()) {
      rows.push({ agent_id: sel, speaker: 'me', sys: true,
        text: 'Offline — demo world' });
      setLogOpen(true);
      renderLog();
      return;
    }
    tf.ui.sendChat(sel, text, locale());
    var row = { agent_id: sel, speaker: 'me', text: text, lang: locale(),
      mine: true, pending: true, ts: new Date().toISOString() };
    rows.push(row);
    setTimeout(function () {
      var i = rows.indexOf(row);
      if (i >= 0 && rows[i].pending) {
        rows.splice(i, 1);
        if (logOpen) renderLog();
      }
    }, 15000);
    c.input.value = '';
    setLogOpen(true);
    renderLog();
  }

  c.send.addEventListener('click', send);
  c.input.addEventListener('keydown', function (e) {
    if (e.key === 'Enter') { e.preventDefault(); send(); }
    if (e.key === 'Escape') c.input.blur();   // 선택 해제는 캔버스 Escape(Rust)
  });

  // ── 타이핑 플래그 — winit keydown은 window에 붙어 DOM 입력 키도 Bevy에 온다.
  // Rust(camera_control·cam_follow)가 이 값을 읽고 타이핑 중 팬/줌/추적 해제를 막는다
  tf.uiTyping = false;
  c.input.addEventListener('focus', function () { tf.uiTyping = true; });
  c.input.addEventListener('blur', function () { tf.uiTyping = false; });

  // ── 모바일 키보드 — visualViewport로 독을 키보드 위에 고정
  var vv = window.visualViewport;
  if (vv) {
    var applyVv = function () {
      var off = Math.max(0, window.innerHeight - vv.height - vv.offsetTop);
      document.documentElement.style.setProperty('--tf-kb', off + 'px');
    };
    vv.addEventListener('resize', applyVv);
    vv.addEventListener('scroll', applyVv);
    applyVv();
  }

  // 언어 전환 — 채팅 독 문자열 재렌더(TF_I18N이 늦게 로드돼도 다음 전환부터)
  if (window.TF_I18N) {
    window.TF_I18N.onLocale(function () {
      if (sel) {
        renderHead();
        if (logOpen) renderLog();
      }
      sideRebuild();
    });
  }

  // ═══════════ §17.76 우측 세로 정보 레일 — FE ChatSideRail·ChatPanels 포트 ═══════════
  // 상태🫀·소지품🎒·기억🕰️(로그인 전용)·생활기록📜·관계💞 5종 pill 버튼이 화면
  // 우측 가장자리에 붙고, 활성 패널 본문은 채팅 바 최상단에 뜬다(한 번에 하나,
  // 같은 버튼 재클릭으로 닫힘). 상태 소유는 TF_UI(tf-ui.js) — 이 독은 표현만.
  var RAIL_TABS = [
    { key: 'status', labelKey: 'panel.status', emoji: '🫀' },
    { key: 'inv', labelKey: 'panel.inventory', emoji: '🎒' },
    { key: 'memories', labelKey: 'chat.tab.memories', emoji: '🕰️', userOnly: true },
    { key: 'history', labelKey: 'panel.history', emoji: '📜' },
    { key: 'relations', labelKey: 'panel.relations', emoji: '💞' },
  ];
  var NEED_BARS = [
    ['needs.hunger', 'hunger'], ['needs.thirst', 'thirst'],
    ['needs.fatigue', 'fatigue'], ['needs.warmth', 'warmth'],
  ];
  var HIST_COLOR = {
    gain: '#6ee7b7', trade: '#fcd34d', craft: '#7dd3fc', consume: '#fda4af',
    '획득': '#6ee7b7', '거래': '#fcd34d', '제작': '#7dd3fc', '소모': '#fda4af',
  };
  var MEM_KIND_STYLE = {
    reflection: 'background:rgba(168,85,247,.2);color:#d8b4fe;',
    plan: 'background:rgba(59,130,246,.15);color:#93c5fd;',
    action: 'background:var(--ink-700,#212836);color:var(--ink-300,#8d97a8);',
    chat: 'background:rgba(34,197,94,.15);color:#86efac;',
  };
  var CARRY_CAP_UI = 5000;   // §17.81 — 룸 rules CARRY_CAP와 동기

  var rail = document.createElement('div');
  rail.className = 'tf-rail';
  document.body.appendChild(rail);
  var sidePanel = document.createElement('div');
  sidePanel.className = 'tf-sidepanel';
  dock.insertBefore(sidePanel, dock.firstChild);

  var side = null;            // 활성 레일 키(null = 닫힘) — 대화 로그(chat)와 배타
  var sideDom = null;         // {key, sel, locale, refs…} — 뼈대 유지·값 갱신 계약

  function T() { return window.TF_UI || null; }
  function carryWeight(inv) {   // FE parts.carryWeight 포트(§17.81) — 착용품 제외
    var DATA = window.TF_DATA;
    var worn = (DATA && DATA.WORN_ITEMS) || [];
    var wt = (DATA && DATA.ITEM_WT) || {};
    var w = 0;
    Object.keys(inv || {}).forEach(function (id) {
      if (worn.indexOf(id) >= 0) return;
      w += (inv[id] || 0) * (wt[id] != null ? wt[id] : 1);
    });
    return Math.round(w * 10) / 10;
  }

  function sideSel() {
    var S = T() && T().state;
    return (sel && S && S.live[sel]) ? S.live[sel] : null;
  }

  function renderRail() {
    var S = T() && T().state;
    var isUser = !!(S && S.scope && S.scope.kind === 'user');
    var on = !!sel;
    rail.style.display = on ? '' : 'none';
    if (!on) return;
    var html = RAIL_TABS.filter(function (tb) {
      return !tb.userOnly || isUser;
    }).map(function (tb) {
      return '<button class="tf-railbtn' + (side === tb.key ? ' on' : '') +
        '" data-side="' + tb.key + '" title="' + esc(t(tb.labelKey, null, tb.key)) + '">' +
        '<span class="em">' + tb.emoji + '</span><span>' + esc(t(tb.labelKey, null, tb.key)) + '</span></button>';
    }).join('');
    if (rail.__html !== html) {
      rail.__html = html;
      rail.innerHTML = html;
    }
  }

  rail.addEventListener('click', function (e) {
    var b = e.target.closest ? e.target.closest('.tf-railbtn') : null;
    if (!b) return;
    setSide(side === b.getAttribute('data-side') ? null : b.getAttribute('data-side'));
  });

  function setSide(key) {
    side = key;
    if (side) setLogOpen(false);   // §17.76 — 한 번에 한 패널(대화 로그와 배타)
    sideRebuild();
    renderRail();
    renderSide();
  }

  /** 패널 뼈대 재구축 조건 — 대상·탭·로케일이 바뀔 때만(입장 애니메이션·바 transition 보존) */
  function sideRebuild() { sideDom = null; }

  function panelShell(emoji, title, bodyHtml, scrollKey) {
    return '<div class="tf-sidepanel-card">' +
      '<div class="tf-sidepanel-head">' +
        '<span class="tt">' + emoji + ' ' + esc(title) + '</span>' +
        '<button class="x" data-side-close="1" title="' + esc(t('btn.close', null, 'Close')) + '">✕</button>' +
      '</div>' +
      '<div class="tf-sidepanel-body tf-scroll"' + (scrollKey ? ' data-sscroll="' + scrollKey + '"' : '') + '>' +
        bodyHtml + '</div>' +
    '</div>';
  }

  function needRowHtml(pair) {
    return '<div class="tf-srow" data-need="' + pair[1] + '">' +
      '<span class="lb">' + esc(t(pair[0], null, pair[1])) + '</span>' +
      '<div class="tf-sbar"><div class="fill"></div></div>' +
      '<span class="vl"></span></div>';
  }

  function sideSkeleton(key, a) {
    var i18n = I();
    var refs = { key: key, sel: sel, locale: locale(), fills: {}, vals: {} };
    var html;
    if (key === 'status') {
      html = panelShell('🫀', (a ? a.name + ' — ' : '') + t('panel.status', null, 'Status'),
        '<div class="tf-sact"></div>' +
        NEED_BARS.map(needRowHtml).join('') +
        '<div class="tf-sgoal"><span class="g"></span><span class="h"></span></div>');
    } else if (key === 'inv') {
      html = panelShell('🎒', (a ? a.name + ' — ' : '') + t('panel.inventory', null, 'Inventory'),
        '<div class="tf-carry"><span class="em">🎒</span>' +
          '<div class="tf-sbar carry"><div class="fill"></div></div><span class="vl"></span></div>' +
        '<div class="tf-chips"></div>');
    } else if (key === 'memories') {
      html = panelShell('🕰️', t('panel.memories', null, 'Memories'),
        '<div class="tf-list" data-sscroll-inner="mem"></div>', 'mem');
    } else if (key === 'history') {
      html = panelShell('📜', t('panel.history', null, 'History'),
        '<div class="tf-list" data-sscroll-inner="hist"></div>', 'hist');
    } else {
      html = panelShell('💞', (a ? a.name + ' — ' : '') + t('panel.relations', null, 'Relations'),
        '<div class="tf-list" data-sscroll-inner="rel"></div>', 'rel');
    }
    sidePanel.innerHTML = html;
    refs.el = sidePanel.firstElementChild;
    refs.act = sidePanel.querySelector('.tf-sact');
    refs.goal = sidePanel.querySelector('.tf-sgoal .g');
    refs.hint = sidePanel.querySelector('.tf-sgoal .h');
    refs.carryFill = sidePanel.querySelector('.tf-carry .fill');
    refs.carryVal = sidePanel.querySelector('.tf-carry .vl');
    refs.chips = sidePanel.querySelector('.tf-chips');
    NEED_BARS.forEach(function (pair) {
      var row = sidePanel.querySelector('[data-need="' + pair[1] + '"]');
      if (row) {
        refs.fills[pair[1]] = row.querySelector('.fill');
        refs.vals[pair[1]] = row.querySelector('.vl');
      }
    });
    ['mem', 'hist', 'rel'].forEach(function (k) {
      var el = sidePanel.querySelector('[data-sscroll-inner="' + k + '"]');
      if (el) refs[k] = el;
    });
    return refs;
  }

  function fmtTime(ts) {
    var d = new Date(ts);
    if (isNaN(d.getTime())) return '';
    var p = function (n) { return (n < 10 ? '0' : '') + n; };
    return p(d.getHours()) + ':' + p(d.getMinutes());
  }

  /** 몸통 값 갱신 — 노드 유지(바 transition·스크롤 위치 보존), 목록류만 innerHTML */
  function sideApply() {
    var d = sideDom;
    if (!d) return;
    var S = T() && T().state;
    var a = sideSel();
    if (!a) return;
    var i18n = I();
    if (d.key === 'status') {
      var act = a.actType || a.status || '';
      if (d.actText !== act) {
        d.actText = act;
        d.act.textContent = act ? tOr('act.' + act, act) : '';
      }
      NEED_BARS.forEach(function (pair) {
        var v = Number((a.needs || {})[pair[1]] != null ? a.needs[pair[1]] : 0);
        var pct = Math.min(100, Math.max(0, Math.round(v)));
        var f = d.fills[pair[1]];
        if (f) {
          f.style.width = pct + '%';
          f.classList.toggle('hot', v > 70);
        }
        var vl = d.vals[pair[1]];
        if (vl) vl.textContent = String(Math.round(v));
      });
      if (d.goalText !== (a.goalText || '')) {
        d.goalText = a.goalText || '';
        d.goal.textContent = d.goalText ? '🎯 ' + d.goalText : '';
        d.goal.title = d.goalText || '';
      }
      var hint = a.mine ? t('chat.hint.own', null, '') : t('chat.hint.spectator', null, '');
      if (d.hintText !== hint) {
        d.hintText = hint;
        d.hint.textContent = hint;
      }
    } else if (d.key === 'inv') {
      var wt = carryWeight(a.inv);
      var pct = Math.min(100, (wt / CARRY_CAP_UI) * 100);
      var heavy = wt / CARRY_CAP_UI >= 0.85;
      if (d.carryFill) {
        d.carryFill.style.width = pct + '%';
        d.carryFill.classList.toggle('heavy', heavy);
      }
      if (d.carryVal) {
        d.carryVal.textContent = wt + '/' + CARRY_CAP_UI;
        d.carryVal.classList.toggle('heavy', heavy);
      }
      var entries = Object.keys(a.inv || {}).filter(function (k) { return (a.inv[k] || 0) > 0; });
      var chipHtml = entries.length
        ? entries.map(function (item) {
            return '<span class="tf-schip" title="' + esc(tOr('item.' + item, item)) + '">' +
              (T().chipIcon ? T().chipIcon(item, 16) : '<span>🎒</span>') +
              '<span>' + esc(tOr('item.' + item, item)) + '</span>' +
              '<b>×' + a.inv[item] + '</b></span>';
          }).join('')
        : '<span class="tf-sempty">' + esc(t('panel.inventory.empty', null, '—')) + '</span>';
      if (d.chipsHtml !== chipHtml) {
        d.chipsHtml = chipHtml;
        d.chips.innerHTML = chipHtml;
      }
    } else if (d.key === 'memories') {
      var mem = (S && S.memories) || [];
      d.mem.innerHTML = mem.length
        ? mem.map(function (m) {
            return '<div class="tf-memrow"><span class="k" style="' +
              (MEM_KIND_STYLE[m.kind] || MEM_KIND_STYLE.action) + '">' +
              esc(tOr('mem.' + m.kind, m.kind)) + '</span>' +
              '<span class="tx">' + esc(m.text || '') + '</span></div>';
          }).join('')
        : '<span class="tf-sempty">' + esc(t('panel.memories.empty', null, '—')) + '</span>';
    } else if (d.key === 'history') {
      var hist = (S && S.history) || [];
      d.hist.innerHTML = hist.length
        ? hist.map(function (h) {
            return '<div class="tf-histrow"><span class="ts">' + esc(fmtTime(h.ts)) + '</span>' +
              '<span class="ty" style="color:' + (HIST_COLOR[h.type] || 'var(--ink-300,#8d97a8)') + '">' +
              esc(tOr('hist.' + h.type, h.type)) + '</span>' +
              '<span class="tx">' + esc(h.text || '') + '</span></div>';
          }).join('')
        : '<span class="tf-sempty">' + esc(t('panel.history.empty', null, '—')) + '</span>';
    } else if (d.key === 'relations') {
      var rels = ((S && S.relations) || []).filter(function (r) {
        return r.a_id === sel || r.b_id === sel;
      }).sort(function (x, y) { return y.warmth - x.warmth; }).slice(0, 8);
      d.rel.innerHTML = rels.length
        ? rels.map(function (r) {
            var otherId = r.a_id === sel ? r.b_id : r.a_id;
            var nm = (S && S.live[otherId] && S.live[otherId].name) || otherId;
            var bucket = i18n ? i18n.relBucket(r.warmth) : '';
            return '<div class="tf-relrow" title="' +
              esc(t('panel.relations.chats', { n: r.chats, m: r.trades }, '')) + '">' +
              '<span class="nm" style="color:' + (i18n ? i18n.agentColor(otherId) : '#fff') + '">' +
                esc(nm) + '</span>' +
              '<div class="tf-sbar rel"><div class="fill" style="width:' + r.warmth + '%;' +
                'background:hsl(' + Math.round(r.warmth * 1.2) + ',65%,55%)"></div></div>' +
              '<span class="vl">' + esc(t('rel.' + bucket, bucket, null)) + ' ' + r.warmth + '</span></div>';
          }).join('')
        : '<span class="tf-sempty">' + esc(t('panel.relations.empty', null, '—')) + '</span>';
    }
  }

  function renderSide() {
    var on = !!(sel && side);
    sidePanel.style.display = on ? '' : 'none';
    if (!on) { sideDom = null; return; }
    var a = sideSel();
    if (!a) return;
    if (!sideDom || sideDom.key !== side || sideDom.sel !== sel || sideDom.locale !== locale()) {
      sideDom = sideSkeleton(side, a);
    }
    sideApply();
  }

  sidePanel.addEventListener('click', function (e) {
    if (e.target.closest && e.target.closest('[data-side-close]')) setSide(null);
  });

  // 갱신 — TF_UI 스냅숏은 1s, 표현은 500ms 폴(바 transition이 보간)
  setInterval(function () {
    renderRail();
    renderSide();
  }, 500);

  // ── 레일·사이드 패널 스타일(tailwind 룩을 tf-ui.css 팔레트로 직역) ──
  var railSt = document.createElement('style');
  railSt.textContent =
    '.tf-rail{position:fixed;right:12px;top:56px;z-index:28;display:flex;' +
    'flex-direction:column;align-items:flex-end;gap:4px;pointer-events:none;}' +
    '.tf-railbtn{pointer-events:auto;display:flex;align-items:center;gap:6px;' +
    'padding:6px 10px;border-radius:999px;border:1px solid var(--ink-700,#212836);' +
    'background:rgba(11,14,20,.85);color:var(--ink-300,#8d97a8);font-size:10px;' +
    'font-weight:500;box-shadow:0 10px 24px rgba(0,0,0,.4);backdrop-filter:blur(6px);cursor:pointer;transition:border-color .15s,color .15s;}' +
    '.tf-railbtn .em{font-size:13px;line-height:1;}' +
    '.tf-railbtn:hover{border-color:var(--ink-500,#4a5468);color:var(--ink-100,#ccd3de);}' +
    '.tf-railbtn.on{border-color:var(--accent,#7c9cff);background:rgba(124,156,255,.2);color:var(--accent,#7c9cff);}' +
    '.tf-sidepanel{display:none;margin-bottom:4px;}' +
    '.tf-sidepanel-card{display:flex;flex-direction:column;max-height:min(40vh,320px);' +
    'min-height:44px;overflow:hidden;border-radius:12px;border:1px solid var(--ink-700,#212836);' +
    'background:rgba(11,14,20,.85);box-shadow:0 10px 24px rgba(0,0,0,.4);backdrop-filter:blur(6px);width:min(340px,calc(100vw - 5.5rem));}' +
    '.tf-sidepanel-head{display:flex;align-items:center;justify-content:space-between;gap:8px;' +
    'padding:4px 10px;border-bottom:1px solid var(--ink-800,#181d28);flex-shrink:0;}' +
    '.tf-sidepanel-head .tt{font-size:10px;font-weight:500;color:var(--ink-400,#6b7689);' +
    'overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}' +
    '.tf-sidepanel-head .x{border:0;background:none;color:var(--ink-400,#6b7689);cursor:pointer;' +
    'font-size:11px;padding:2px 4px;border-radius:4px;line-height:1;}' +
    '.tf-sidepanel-head .x:hover{background:var(--ink-800,#181d28);color:var(--ink-200,#aeb7c6);}' +
    '.tf-sidepanel-body{flex:1;min-height:0;overflow-y:auto;padding:6px 10px;}' +
    '.tf-sact{font-size:11px;color:var(--ink-400,#6b7689);margin-bottom:6px;}' +
    '.tf-srow{display:flex;align-items:center;gap:6px;margin-bottom:6px;}' +
    '.tf-srow .lb{width:36px;flex-shrink:0;font-size:9px;color:var(--ink-400,#6b7689);' +
    'overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}' +
    '.tf-sbar{height:4px;flex:1;overflow:hidden;border-radius:999px;background:var(--ink-800,#181d28);}' +
    '.tf-sbar .fill{height:100%;border-radius:999px;background:var(--accent,#7c9cff);' +
    'transition:width .5s ease;min-width:0;}' +
    '.tf-sbar .fill.hot{background:#f87171;}' +
    '.tf-srow .vl,.tf-carry .vl{width:20px;flex-shrink:0;text-align:right;font-size:9px;' +
    'color:var(--ink-500,#4a5468);font-variant-numeric:tabular-nums;}' +
    '.tf-sgoal{display:flex;align-items:center;gap:8px;font-size:9px;color:var(--ink-500,#4a5468);padding-top:2px;}' +
    '.tf-sgoal .g{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}' +
    '.tf-sgoal .h{flex-shrink:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}' +
    '.tf-carry{display:flex;align-items:center;gap:6px;font-size:10px;margin-bottom:8px;}' +
    '.tf-carry .em{flex-shrink:0;font-size:11px;line-height:1;}' +
    '.tf-carry .tf-sbar{height:6px;}' +
    '.tf-carry .fill{background:#10b981;}' +
    '.tf-carry .fill.heavy{background:#fbbf24;}' +
    '.tf-carry .vl.heavy{color:#fcd34d;}' +
    '.tf-chips{display:flex;flex-wrap:wrap;gap:4px;}' +
    '.tf-schip{display:flex;align-items:center;gap:4px;padding:4px 6px;border-radius:6px;' +
    'border:1px solid var(--ink-700,#212836);background:rgba(17,21,29,.6);' +
    'color:var(--ink-200,#aeb7c6);font-size:11px;}' +
    '.tf-schip b{border-radius:4px;background:var(--ink-800,#181d28);padding:0 4px;' +
    'font-size:10px;color:var(--ink-300,#8d97a8);font-weight:400;}' +
    '.tf-schip img{image-rendering:pixelated;border-radius:2px;}' +
    '.tf-sempty{font-size:11px;color:var(--ink-500,#4a5468);}' +
    '.tf-memrow,.tf-histrow{font-size:11px;line-height:1.625;display:flex;gap:6px;align-items:baseline;}' +
    '.tf-memrow{margin-bottom:4px;}' +
    '.tf-memrow .k{flex-shrink:0;border-radius:4px;padding:1px 4px;font-size:9px;}' +
    '.tf-memrow .tx{color:var(--ink-300,#8d97a8);}' +
    '.tf-histrow .ts{flex-shrink:0;color:var(--ink-500,#4a5468);}' +
    '.tf-histrow .ty{flex-shrink:0;font-weight:500;}' +
    '.tf-histrow .tx{color:var(--ink-300,#8d97a8);min-width:0;}' +
    '.tf-relrow{display:flex;align-items:center;gap:8px;margin-bottom:6px;}' +
    '.tf-relrow .nm{width:56px;flex-shrink:0;font-size:11px;overflow:hidden;' +
    'text-overflow:ellipsis;white-space:nowrap;}' +
    '.tf-relrow .tf-sbar{height:6px;}' +
    '.tf-relrow .vl{width:80px;flex-shrink:0;text-align:right;font-size:10px;' +
    'color:var(--ink-400,#6b7689);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}' +
    '@media (max-width:600px){.tf-rail{top:50px;}.tf-railbtn span:not(.em){display:none;}' +
    '.tf-railbtn{padding:7px 8px;}.tf-sidepanel-card{width:calc(100vw - 5.5rem);}}';
  document.head.appendChild(railSt);
})();
