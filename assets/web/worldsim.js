// Terrafolk 라이브 월드 브리지 — colyseus.js(벤더 UMD) 재사용.
// Rust(wasm)는 window.terrafolk.setSink(fn)으로 콜백을 등록하고
// connect(apiBase, wsBase)로 접속을 요청한다. 상태 변화마다 평면 JSON을
// 스킴 → sink 로 흘린다(패치 주기 1s — 전체 스냅숏 전송으로 충분).
// 접속 실패·끊김은 5초 후 재시도(관전은 무인증 갱신).
(function () {
  'use strict';
  let sink = null;
  let room = null;
  let retryTimer = null;
  let lastMap = null; // mapJson은 fullStateOnly — 최초 1회만 오므로 캐시
  const names = {}; // agent id → 표시명(채팅 스피커)

  // ── HUD — 관전 채팅·이벤트 오버레이 + 접속 상태 배지
  let hud = null;
  let badge = null;
  let lastEvAt = 0;

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
  }

  function esc(s) {
    return String(s).replace(/[&<>"]/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c];
    });
  }

  function hudLine(html, cls) {
    ensureHud();
    if (cls === 'ev') {
      // 이벤트 홍수 제한 — 1.5s당 1줄(최신 것만)
      const now = Date.now();
      if (now - lastEvAt < 1500) return;
      lastEvAt = now;
    }
    const el = document.createElement('div');
    el.style.cssText =
      'text-shadow:0 1px 3px rgba(0,0,0,.85);opacity:0;transition:opacity .5s;';
    if (cls === 'ev') el.style.color = 'rgba(255,244,214,.92)';
    el.innerHTML = html;
    hud.appendChild(el);
    requestAnimationFrame(function () { el.style.opacity = '1'; });
    while (hud.children.length > 7) hud.removeChild(hud.children[1]);
    setTimeout(function () {
      el.style.opacity = '0';
      setTimeout(function () { el.remove(); }, 600);
    }, cls === 'chat' ? 15000 : 9000);
  }

  function snapshot(st) {
    const agents = [];
    st.agents.forEach(function (a, id) {
      names[id] = a.name;
      agents.push({
        id: id, name: a.name, kind: a.kind,
        x: a.x, y: a.y, facing: a.facing,
        status: a.status, actType: a.actType,
        look: a.lookJson,
      });
    });
    const resources = [];
    st.resources.forEach(function (r) {
      resources.push({
        id: r.id, kind: r.kind, species: r.species,
        x: r.x, y: r.y, state: r.stateJson,
      });
    });
    if (typeof st.mapJson === 'string' && st.mapJson) lastMap = st.mapJson;
    return JSON.stringify({
      tick: st.tick, day: st.day, hour: st.hour, minute: st.minute,
      season: st.season, weather: st.weather,
      sunrise: st.sunrise, sunset: st.sunset,
      map: lastMap,
      agents: agents, resources: resources,
    });
  }

  function autoBases() {
    const local = /^(localhost|127\.0\.0\.1)$/.test(location.hostname);
    return local
      ? { api: 'http://127.0.0.1:8892', ws: 'http://127.0.0.1:8893' }
      : { api: 'https://tf-api.suseona.com', ws: 'https://tf-world.suseona.com' };
  }

  async function connect(apiBase, wsBase) {
    if (!apiBase || !wsBase) {
      const b = autoBases();
      apiBase = b.api; wsBase = b.ws;
    }
    if (!window.Colyseus || !window.Colyseus.Client) {
      console.warn('[worldsim] colyseus.js 미로딩 — 3초 후 재시도');
      retryTimer = setTimeout(function () { connect(apiBase, wsBase); }, 3000);
      return;
    }
    try {
      clearTimeout(retryTimer);
      // 관전 티켓(비로그인 게이트 예외)
      const tRes = await fetch(apiBase + '/api/worldsim/spectate-ticket');
      if (!tRes.ok) throw new Error('티켓 HTTP ' + tRes.status);
      const ticket = (await tRes.json()).ticket;

      const client = new window.Colyseus.Client(wsBase);
      client.auth.token = ticket;
      room = await client.joinOrCreate('world');
      console.info('[worldsim] 룸 접속 완료');
      setStatus(true);

      room.onStateChange(function (st) {
        try {
          if (sink) sink(snapshot(st));
        } catch (e) { console.error('[worldsim] sink 오류', e); }
      });
      // 채팅 — 발화 원문 우선(§17.55), 없으면 영문 정본
      room.onMessage('chat', function (m) {
        const nm = names[m.agent_id] || m.speaker || '?';
        hudLine('<b>' + esc(nm) + '</b> · ' + esc(m.text || m.text_en || ''), 'chat');
      });
      // 이벤트 — 영문 렌더 정본 표시(관전 정보창)
      room.onMessage('event', function (e) {
        const tx = e && e.payload && e.payload.text;
        if (tx) hudLine(esc(tx), 'ev');
      });
      room.onLeave(function () {
        console.warn('[worldsim] 퇴장 — 5초 후 재접속');
        setStatus(false);
        retryTimer = setTimeout(function () { connect(apiBase, wsBase); }, 5000);
      });
      room.onError(function (code, msg) {
        console.warn('[worldsim] 룸 오류', code, msg);
      });
    } catch (e) {
      console.warn('[worldsim] 접속 실패(5초 후 재시도):', String(e));
      setStatus(false);
      retryTimer = setTimeout(function () { connect(apiBase, wsBase); }, 5000);
    }
  }

  window.terrafolk = {
    setSink: function (fn) { sink = fn; },
    connect: connect,
  };
})();
