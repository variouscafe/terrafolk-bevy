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

  // ── 레일 브리지 — tf-rail.js(우측 정보 레일)가 선택·로그 배타를 공유한다.
  window.TF_CHAT = {
    getSel: function () { return sel; },
    setLogOpen: setLogOpen,
  };
})();
