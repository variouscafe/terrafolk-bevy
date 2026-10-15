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

  function snapshot(st) {
    const agents = [];
    st.agents.forEach(function (a, id) {
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

      room.onStateChange(function (st) {
        try {
          if (sink) sink(snapshot(st));
        } catch (e) { console.error('[worldsim] sink 오류', e); }
      });
      room.onLeave(function () {
        console.warn('[worldsim] 퇴장 — 5초 후 재접속');
        retryTimer = setTimeout(function () { connect(apiBase, wsBase); }, 5000);
      });
      room.onError(function (code, msg) {
        console.warn('[worldsim] 룸 오류', code, msg);
      });
    } catch (e) {
      console.warn('[worldsim] 접속 실패(5초 후 재시도):', String(e));
      retryTimer = setTimeout(function () { connect(apiBase, wsBase); }, 5000);
    }
  }

  window.terrafolk = {
    setSink: function (fn) { sink = fn; },
    connect: connect,
  };
})();
