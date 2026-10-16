// Terrafolk 라이브 월드 브리지 — colyseus.js(벤더 UMD) 재사용.
// Rust(wasm)는 window.terrafolk.setSink(fn)으로 콜백을 등록하고
// connect(apiBase, wsBase)로 접속을 요청한다. 상태 변화마다 평면 JSON을
// 스킴 → sink 로 흘린다(패치 주기 1s — 전체 스냅숏 전송으로 충분).
// DOM UI(tf-ui.js — ☰ 메뉴·팝업, ui.js — 채팅 바)에는 상세 프레임
// (욕구·목표·인벤·발명)과 채팅 행을 함께 먹인다.
//
// 인증: OAuth 콜백 fragment(#at=..&rt=..) → localStorage. 401 시 rt로 1회 갱신.
// 접속 실패·끊김은 지수 백오프 재시도 + 탭 복귀·네트워크 회복 즉시 재시도.
(function () {
  'use strict';
  const AUTH_API = 'https://suseona-api.suseona.com';
  const AUTH_KEY = 'tf-rust.auth';
  let sink = null;
  let room = null;
  let retryTimer = null;
  let retry = 0;
  let alive = false;
  let lastMap = null; // mapJson은 fullStateOnly — 최초 1회만 오므로 캐시
  const names = {}; // agent id → 표시명(채팅 스피커)
  const agentsInfo = {}; // agent id → {name, actType, status} — ui.js 헤더 칩·행 판정
  const chatRowListeners = []; // ui.js — 대화 로그 패널 수신기

  // ── 인증(FE api/client.ts 축소 포트 — body rt 레인 1회 갱신) ──
  function readAuth() {
    try {
      return JSON.parse(localStorage.getItem(AUTH_KEY) || 'null');
    } catch (e) { return null; }
  }
  function writeAuth(a) {
    try {
      if (a) localStorage.setItem(AUTH_KEY, JSON.stringify(a));
      else localStorage.removeItem(AUTH_KEY);
    } catch (e) { /* 저장소 차단 */ }
  }
  // 토큰 fragment 핸드셰이크 — OAuth 콜백이 #at=..&rt=..&provider=..로 돌아온다
  (function captureHash() {
    const hash = location.hash.slice(1);
    if (!hash) return;
    const params = new URLSearchParams(hash);
    const at = params.get('at');
    const rt = params.get('rt');
    const provider = params.get('provider') || undefined;
    if (at && rt) {
      writeAuth({ at: at, rt: rt, provider: provider });
      history.replaceState(null, '', location.pathname + location.search);
    }
  })();

  let refreshing = null;
  function refreshAccessToken() {
    if (!refreshing) {
      const prev = readAuth();
      refreshing = fetch(AUTH_API + '/auth/refresh', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ refreshToken: prev ? prev.rt : '' }),
        credentials: 'include',
      }).then(function (r) {
        if (!r.ok) throw new Error('refresh ' + r.status);
        return r.json();
      }).then(function (data) {
        if (!data.accessToken) throw new Error('no accessToken');
        const cur = readAuth() || {};
        writeAuth({
          at: data.accessToken,
          rt: data.refreshToken != null ? data.refreshToken : cur.rt,
          provider: cur.provider,
        });
        return true;
      }).catch(function () {
        writeAuth(null);
        return false;
      }).finally(function () { refreshing = null; });
    }
    return refreshing;
  }
  /** 인증 fetch — Bearer 부착 + 401 시 1회 갱신 재시도. 미로그인이면 그냥 fetch. */
  function authFetch(url, opts) {
    opts = opts || {};
    const a = readAuth();
    if (a && a.at) {
      opts.headers = Object.assign({}, opts.headers, { Authorization: 'Bearer ' + a.at });
    }
    return fetch(url, opts).then(function (r) {
      if (r.status !== 401 || !a) return r;
      return refreshAccessToken().then(function (ok) {
        if (!ok) return r;
        const a2 = readAuth();
        opts.headers = Object.assign({}, opts.headers, { Authorization: 'Bearer ' + a2.at });
        return fetch(url, opts);
      });
    });
  }

  async function logoutEverywhere() {
    const a = readAuth();
    writeAuth(null);
    try {
      if (a && a.rt) {
        await fetch(AUTH_API + '/auth/logout', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ refreshToken: a.rt }),
          credentials: 'include',
        });
      }
    } catch (e) { /* 클라이언트 세션은 정리 완료 */ }
  }

  // ── HUD — 접속 상태 배지(관전 채팅·이벤트의 화면 표현은 말풍선·토스트가 담당)
  let hud = null;
  let badge = null;

  function ensureHud() {
    if (hud) return;
    // 스타일은 <style>로 — 미디어쿼리·safe-area(노치/홈인디케이터) 대응
    const st = document.createElement('style');
    st.textContent =
      '#tf-hud{position:fixed;z-index:50;pointer-events:none;display:flex;' +
      'flex-direction:column;gap:3px;color:#fff;' +
      'left:calc(12px + env(safe-area-inset-left,0px));' +
      'bottom:calc(14px + env(safe-area-inset-bottom,0px));' +
      'max-width:min(46ch,72vw);font:12px/1.45 ui-sans-serif,system-ui,sans-serif;}' +
      '#tf-hud .tf-badge{align-self:flex-start;padding:2px 9px;border-radius:999px;' +
      'margin-bottom:4px;background:rgba(20,24,32,.62);font-weight:600;' +
      'letter-spacing:.02em;box-shadow:0 1px 6px rgba(0,0,0,.35);transition:background .4s;}' +
      '@media (max-width:600px){#tf-hud{font-size:13.5px;max-width:84vw;}}';
    document.head.appendChild(st);
    hud = document.createElement('div');
    hud.id = 'tf-hud';
    document.body.appendChild(hud);
    badge = document.createElement('div');
    badge.className = 'tf-badge';
    hud.appendChild(badge);
    setStatus(false);
  }

  function setStatus(live) {
    ensureHud();
    badge.textContent = live ? '● LIVE · terrafolk' : '○ DEMO — 서버 재접속';
    badge.style.background = live ? 'rgba(22,101,52,.78)' : 'rgba(120,72,20,.72)';
    if (window.TF_UI) window.TF_UI.setNet(live);
  }

  function snapshot(st) {
    const agents = [];
    st.agents.forEach(function (a, id) {
      names[id] = a.name;
      agentsInfo[id] = { name: a.name, actType: a.actType, status: a.status };
      agents.push({
        id: id, name: a.name, kind: a.kind,
        x: a.x, y: a.y, facing: a.facing,
        status: a.status, actType: a.actType,
        actProgress: a.actProgress != null ? a.actProgress : 0,
        look: a.lookJson,
        // UI 상세(팝업용) — Rust LiveAgent는 미지정 필드를 무시한다
        needs: { hunger: a.hunger, thirst: a.thirst, fatigue: a.fatigue, warmth: a.warmth },
        goalText: a.goalText || '', goalKey: a.goalKey || '',
        inventory: a.invJson,
      });
    });
    const resources = [];
    st.resources.forEach(function (r) {
      resources.push({
        id: r.id, kind: r.kind, species: r.species,
        x: r.x, y: r.y, state: r.stateJson,
      });
    });
    if (typeof st.mapJson === 'string' && st.mapJson) {
      lastMap = st.mapJson;
      // 미니맵 베이크용 1회 노출 — tf-world-ui.js가 읽고 파싱한다(매초 전파 방지)
      if (!window.TF_MAP_JSON) window.TF_MAP_JSON = st.mapJson;
    }
    // 발명품 레지스트리(§17.30) — 문명 팝업·인벤 칩 아이콘의 원천
    let inventions = [];
    try {
      inventions = st.inventionsJson ? JSON.parse(st.inventionsJson) : [];
    } catch (e) { /* inventionsJson 파싱 실패 — 빈 목록 */ }
    if (sink) {
      try {
        sink(JSON.stringify({
          tick: st.tick, day: st.day, hour: st.hour, minute: st.minute,
          season: st.season, weather: st.weather,
          sunrise: st.sunrise, sunset: st.sunset,
          map: lastMap,
          agents: agents, resources: resources,
        }));
      } catch (e) { console.error('[worldsim] sink 오류', e); }
    }
    if (window.TF_UI) {
      window.TF_UI.feed.frame({
        day: st.day, hour: st.hour, minute: st.minute, season: st.season,
        weather: st.weather, sunrise: st.sunrise, sunset: st.sunset,
        agents: agents,
        resources: resources.map(function (r) {
          let state = {};
          try { state = r.state ? JSON.parse(r.state) : {}; } catch (e) { /* pass */ }
          return { id: r.id, kind: r.kind, species: r.species, x: r.x, y: r.y, state: state };
        }),
        inventions: inventions,
      });
    }
  }

  function autoBases() {
    const local = /^(localhost|127\.0\.0\.1)$/.test(location.hostname);
    return local
      ? { api: 'http://127.0.0.1:8892', ws: 'http://127.0.0.1:8893' }
      : { api: 'https://tf-api.suseona.com', ws: 'https://tf-world.suseona.com' };
  }

  // ── 접속(지수 백오프 + 탭 복귀 즉시 재시도, docs §17.29) ──
  function scheduleRetry() {
    alive = false;
    room = null;
    retry += 1;
    const wait = Math.min(3000 * 2 ** (retry - 1), 30000);
    if (retryTimer) clearTimeout(retryTimer);
    retryTimer = setTimeout(function () { void connect(); }, wait);
  }

  async function connect(apiBase, wsBase) {
    if (!apiBase || !wsBase) {
      const b = autoBases();
      apiBase = b.api; wsBase = b.ws;
    }
    // tf-i18n(mt 번역)·ui.js(백로그)가 쓰는 공용 베이스
    window.TF = window.TF || {};
    window.TF.apiBase = apiBase;
    window.TF.wsBase = wsBase;
    if (!window.Colyseus || !window.Colyseus.Client) {
      console.warn('[worldsim] colyseus.js 미로딩 — 3초 후 재시도');
      retryTimer = setTimeout(function () { connect(apiBase, wsBase); }, 3000);
      return;
    }
    try {
      clearTimeout(retryTimer);
      // 티켓 — 로그인이면 인증 티켓(스코프 user), 아니면 관전 티켓(비로그인 게이트 예외).
      // 토큰 만료·갱신 실패는 관전으로 강등(세션은 authFetch가 정리)
      let ticket = null;
      const auth = readAuth();
      if (auth && auth.at) {
        const r = await authFetch(apiBase + '/api/worldsim/ticket');
        if (r.ok) ticket = (await r.json()).ticket;
      }
      if (!ticket) {
        const tRes = await fetch(apiBase + '/api/worldsim/spectate-ticket');
        if (!tRes.ok) throw new Error('티켓 HTTP ' + tRes.status);
        ticket = (await tRes.json()).ticket;
      }

      const client = new window.Colyseus.Client(wsBase);
      client.auth.token = ticket;
      room = await client.joinOrCreate('world');
      console.info('[worldsim] 룸 접속 완료');
      ui.base = apiBase; // 백로그 fetch 등 ui.js가 직접 쓰는 API 베이스
      setStatus(true);
      alive = true;
      retry = 0;

      room.onStateChange(function (st) {
        try {
          snapshot(st);
        } catch (e) { console.error('[worldsim] 스냅숏 오류', e); }
      });
      // 스코프 — 룸 hello 메시지({kind:'user'|'spectator', email?})
      room.onMessage('hello', function (m) {
        if (window.TF_UI) window.TF_UI.feed.scope(m || { kind: 'spectator' });
      });
      // 채팅 — 발화 원문 우선(§17.55). 화면 표현은 3D 씬 말풍선(tf-world-ui.js)
      // 와 대화 로그(ui.js)가 담당 — FE와 동일(워드 HUD 줄은 폐지, §17.44 포트).
      room.onMessage('chat', function (m) {
        for (const fn of chatRowListeners) {
          try { fn(m); } catch (e) { console.error('[worldsim] ui 수신기 오류', e); }
        }
        if (window.TF_UI) window.TF_UI.feed.chat(m);
      });
      // 이벤트 — 하이라이트 토스트(tf-world-ui.js)·기록 팝업이 소비.
      room.onMessage('event', function (e) {
        if (window.TF_UI) window.TF_UI.feed.event(e);
      });
      room.onLeave(function () {
        console.warn('[worldsim] 퇴장 — 재접속');
        setStatus(false);
        scheduleRetry();
      });
      room.onError(function (code, msg) {
        console.warn('[worldsim] 룸 오류', code, msg);
      });
    } catch (e) {
      console.warn('[worldsim] 접속 실패(재시도):', String(e));
      setStatus(false);
      scheduleRetry();
    }
  }

  // 탭 복귀·네트워크 회복 — 끊긴 상태면 즉시 재접속
  function wake() {
    if (alive || document.visibilityState !== 'visible') return;
    if (retryTimer) clearTimeout(retryTimer);
    retryTimer = setTimeout(function () { void connect(); }, 300);
  }
  document.addEventListener('visibilitychange', wake);
  window.addEventListener('online', wake);

  // UI 브리지 — assets/web/ui.js(채팅 바·대화 로그 DOM)가 소비.
  // base는 룸 접속 확정 후 세팅되고, names는 스냅숏마다 갱신되는 live 객체다.
  const ui = {
    base: null,
    names: names,
    live: function () { return !!room; },
    logOpenFlag: false,
    sendChat: function (agentId, text, lang) {
      if (!room) return false;
      room.send('chat', { agentId: agentId, text: text, lang: lang || 'en' });
      return true;
    },
    onChatRow: function (fn) { chatRowListeners.push(fn); },
    agentInfo: function (id) { return agentsInfo[id] || null; },
  };

  window.terrafolk = {
    setSink: function (fn) { sink = fn; },
    connect: connect,
    ui: ui,
    // Rust 3D 씬 빈 땅 클릭 → 유물 카드 판정(tf-ui). pick.rs가 호출.
    uiPick: function (x, y) {
      if (window.TF_UI) window.TF_UI.pickAt(x, y);
    },
    authFetch: authFetch,
    getAuth: function () { return readAuth() || {}; },
    logout: logoutEverywhere,
  };

  // 부팅 — 접속 + 백로그 시딩(기록 팝업을 열면 즉시 최근 목록)
  function boot() {
    void connect();
    if (window.TF_UI) window.TF_UI.seedBacklogs();
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', boot);
  } else {
    boot();
  }
})();
