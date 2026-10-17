// Terrafolk 앰비언트 오버레이 — 계절 틴트·번개 섬꼉·사망 연출(FE scene·
// weatherfx 포트). TF_UI.feed.frame/event를 wrap해 소비하고, 사망 양쇠 위치는
// tf-world-ui의 투영(TF_WORLD_UI.proj)을 참조한다(기능별 파일 분리).
(function () {
  'use strict';
  function UI() { return window.TF_UI || null; }

  var SEASON_TINT = {
    spring: 'rgba(216,255,216,.10)', summer: 'rgba(255,242,200,.08)',
    autumn: 'rgba(255,216,168,.12)', winter: 'rgba(200,220,255,.14)',
  };
  var seasonEl = document.createElement('div');
  seasonEl.id = 'tf-season';
  document.body.appendChild(seasonEl);
  var lightningEl = document.createElement('div');
  lightningEl.id = 'tf-lightning';
  document.body.appendChild(lightningEl);

  function syncAmbience() {
    var S = UI() && UI().state;
    var clock = S ? S.clock : {};
    var tint = SEASON_TINT[clock.season] || 'rgba(255,255,255,0)';
    if (seasonEl.__bg !== tint) {
      seasonEl.__bg = tint;
      seasonEl.style.background = tint;
    }
    scheduleLightning(clock.weather);
  }

  // 번개 — storm에서만. 3.5~9s 간격, 25% 이중 타격(FE flashLightning 체인).
  // 무장 멱등 — feed.frame(1s)마다 호출되므로 이미 무장중이면 재무장하지
  // 않는다(리뷰 #6 — 매 호출마다 타이머를 리셋해 영구 무발동이었다).
  var boltTimer = 0;
  function armBolt() {
    boltTimer = setTimeout(function () {
      flashBolt(0);
      if (Math.random() < 0.25) setTimeout(function () { flashBolt(0.12); }, 120 + Math.random() * 180);
      armBolt();
    }, 3500 + Math.random() * 5500);
  }
  function scheduleLightning(weather) {
    if (weather !== 'storm') {
      if (boltTimer) { clearTimeout(boltTimer); boltTimer = 0; }
      lightningEl.style.opacity = '0';
      return;
    }
    if (!boltTimer) armBolt();
  }
  function flashBolt(delay) {
    if (delay) {
      setTimeout(function () { flashBolt(0); }, delay * 1000);
      return;
    }
    lightningEl.style.transition = 'none';
    lightningEl.style.opacity = '0.55';
    requestAnimationFrame(function () {
      lightningEl.style.transition = 'opacity .3s';
      lightningEl.style.opacity = '0';
    });
  }

  /** 사망 연출 — 마지막 투영 위치에서 🕯 상승 페이드 + 불씨 파편 7개 */
  function deathCandle(agentId) {
    var wu = window.TF_WORLD_UI;
    var p = wu ? wu.projOf(agentId) : null;
    if (!p) return;   // 화면 밖 사망 — 연출 생략(토스트는 표시됨)
    var el = document.createElement('div');
    el.className = 'tf-candle';
    el.textContent = '🕯';
    el.style.left = (p.x - 9) + 'px';
    el.style.top = (p.y - 30) + 'px';
    for (var i = 0; i < 7; i++) {
      var sp = document.createElement('span');
      sp.className = 'sp';
      sp.style.setProperty('--dx', ((Math.random() - 0.5) * 26).toFixed(0) + 'px');
      sp.style.setProperty('--dy', (-14 - Math.random() * 18).toFixed(0) + 'px');
      el.appendChild(sp);
    }
    document.body.appendChild(el);
    setTimeout(function () { el.remove(); }, 2500);
  }

  // feed wrap — 계절·날씨(프레임)와 사망(이벤트)을 함께 소비
  function wrapFeed() {
    var T = UI();
    if (!T || T.feed.__ambience) return;
    var origFrame = T.feed.frame;
    T.feed.frame = function (f) {
      if (origFrame) origFrame(f);
      syncAmbience();
    };
    var origEvent = T.feed.event;
    T.feed.event = function (e) {
      if (origEvent) origEvent(e);
      if (e && e.kind === 'death' && e.agent_id) deathCandle(e.agent_id);
    };
    T.feed.__ambience = true;
  }
  wrapFeed();
  document.addEventListener('DOMContentLoaded', wrapFeed);
})();
