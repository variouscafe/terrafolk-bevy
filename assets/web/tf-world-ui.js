// Terrafolk 월드 앵커 오버레이 — FE scene/agents.ts의 씬 오버레이군 포트.
// Rust(src/world/overlay.rs)가 매 프레임 주민 머리 앵커를 화면 좌표로 투영해
// tf.onOverlay(json)로 주면 이 모듈이 DOM으로 렌더한다:
//   · 이름 라벨(저줌 숨김) · 말풍선(§17.44 · 사용자 발언 금색 ⟪이름⟫)
//   · 활동 배지(진행 링 + ACT_ICON) · 수면 zZz · 욕구 경고 ❗
//   · 하이라이트 토스트(우하단, FE highlightOf 20종)
//   · 미니맵(지형 1회 베이크 + 점·뷰포트 박스 + 클릭 점프 tf.panTo)
// 데이터는 TF_UI.state(1s 스냅숏)·TF_UI.feed wrap(채팅·이벤트)로 받는다 —
// 로드 순서: tf-ui.js 직후(이 파일은 TF_UI·TF_I18N를 사용 시점에 해석).
(function () {
  'use strict';
  var tf = window.terrafolk || (window.terrafolk = {});
  function UI() { return window.TF_UI || null; }
  function I() { return window.TF_I18N || null; }

  // ── 상수(FE scene/consts.ts 직역) ──
  var ACT_ICON = {
    chop: '🪓', mine: '⛏', forage: '🌾', hunt: '🏹', eat: '🍖', drink: '💧',
    campfire: '🔥', warm: '♨️', wear: '🧥', give: '💝', trade: '🤝',
    gather: '🌿', build: '🏠', invent: '💡', craft: '⚒️', perform: '🎭',
    fetch: '💧', drop: '👐', pickup: '👐', store: '📦', withdraw: '📦',
  };
  var LABEL_ZOOM_MAX = 1.9;   // 이 줌(월드 약 42m 시야)보다 멀면 이름 숨김(FE zoom<0.85)
  var BUBBLE_LIFE = 4100;     // 3.4s 노출 + 0.7s 페이드(FE 트윈 동일)
  var BUBBLE_FADE = 700;
  var TOAST_LIFE = 6000;      // FE 6s 후 제거
  var TOAST_MAX = 4;
  var MM_DESK = 148, MM_MOB = 112;
  var TERRAIN_COLORS = {
    1: '#63c5cf', 2: '#eaa56c', 3: '#8dc435', 4: '#4f9448', 5: '#9a948a', 6: '#c8a06a',
  };

  // ── 스타일 — 팔레트는 tf-ui.css 시맨틱과 동일값(ink/accent) ──
  var st = document.createElement('style');
  st.textContent =
    // 월드 앵커 컨테이너 — 캔버스 위·HUD·탑바 아래
    '#tf-worldui{position:fixed;inset:0;z-index:25;pointer-events:none;overflow:hidden;' +
    'font:11px/1.4 ui-sans-serif,system-ui,sans-serif;}' +
    '.tf-wa{position:absolute;left:0;top:0;display:flex;flex-direction:column;' +
    'align-items:center;gap:2px;transform:translate(-50%,-100%);' +
    'will-change:transform;user-select:none;}' +
    '.tf-wa.off{display:none;}' +
    '.tf-wname{max-width:110px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;' +
    'padding:1px 4px;border-radius:4px;color:#f5f2e8;background:rgba(0,0,0,.6);' +
    'text-shadow:0 1px 2px rgba(0,0,0,.8);}' +
    '.tf-wname.sel{color:#ffe9b0;background:rgba(20,16,4,.72);outline:1px solid rgba(255,233,176,.35);}' +
    '.tf-wrow{display:flex;align-items:center;gap:3px;}' +
    // 활동 배지 — 진행 링(SVG) + 이모지. 링 굵기=진행률(FE setStrokeStyle(2+frac*3))
    '.tf-wbadge{position:relative;width:30px;height:30px;display:grid;place-items:center;}' +
    '.tf-wbadge svg{position:absolute;inset:0;}' +
    '.tf-wbadge .ic{position:relative;font-size:12px;line-height:1;' +
    'filter:drop-shadow(0 1px 1px rgba(0,0,0,.7));}' +
    // 말풍선 — 검은 배경, 사용자 발언 금색. 3.4s 후 0.7s 페이드
    '.tf-wbubble{max-width:190px;padding:3px 5px;border-radius:6px;color:#f5f2e8;' +
    'background:rgba(0,0,0,.8);text-align:center;text-shadow:0 1px 2px rgba(0,0,0,.8);}' +
    '.tf-wbubble.user{color:#ffe9b0;}' +
    '.tf-wbubble.bye{animation:tf-wb-fade .7s ease-in forwards;}' +
    '@keyframes tf-wb-fade{to{opacity:0;}}' +
    // 수면 zZz — 상승 페이드 반복(FE 1600ms)
    '.tf-wzzz{color:#cfe3ff;font-weight:700;font-size:13px;' +
    'animation:tf-wz 1.6s linear infinite;}' +
    '@keyframes tf-wz{0%{opacity:0;transform:translateY(2px);}20%{opacity:1;}' +
    '100%{opacity:0;transform:translateY(-10px);}}' +
    // 욕구 경고 — 바운스(FE ±4px 320ms yoyo)
    '.tf-walert{font-size:15px;line-height:1;animation:tf-wa-b .32s ease-in-out ' +
    'infinite alternate;}' +
    '@keyframes tf-wa-b{from{transform:translateY(0);}to{transform:translateY(-4px);}}' +
    // 하이라이트 토스트 — 우하단(FE WorldSimPage 하이라이트 피드)
    '#tf-toasts{position:fixed;right:12px;bottom:calc(12px + env(safe-area-inset-bottom,0px));' +
    'z-index:30;display:flex;flex-direction:column;gap:6px;width:min(300px,45vw);' +
    'pointer-events:none;}' +
    '.tf-toast{display:flex;align-items:flex-start;gap:8px;padding:8px 12px;' +
    'border-radius:12px;border:1px solid var(--ink-700,rgba(52,58,66,.9));' +
    'background:var(--ink-950,#0b0e14);color:var(--ink-100,#ece7dd);' +
    'font-size:11px;line-height:1.625;box-shadow:0 10px 24px rgba(0,0,0,.4);' +
    'backdrop-filter:blur(6px);animation:tf-toast-in .25s ease-out;}' +
    '.tf-toast .ic{font-size:16px;line-height:1.1;}' +
    '.tf-toast .who{font-weight:600;color:var(--ink-50,#f8f5ef);}' +
    '@keyframes tf-toast-in{from{opacity:0;transform:translateY(6px);}}' +
    // 미니맵 — 좌측 상단, 탑바 바로 아래(FE와 동일 위치)
    '#tf-minimap{position:fixed;left:12px;top:46px;z-index:26;overflow:hidden;' +
    'border-radius:8px;border:2px solid rgba(6,8,10,.9);' +
    'box-shadow:0 10px 24px rgba(0,0,0,.4);cursor:crosshair;pointer-events:auto;}' +
    '#tf-minimap canvas{display:block;width:100%;height:100%;image-rendering:pixelated;}' +
    // 자원 호버 툴팁 — 마우스 전용(FE makeTooltip 룩: 검은 배경 12px, 220px wrap)
    '#tf-restip{position:fixed;z-index:27;display:none;max-width:220px;padding:4px 6px;' +
    'border-radius:4px;background:rgba(0,0,0,.85);color:#f5f2e8;font-size:12px;' +
    'line-height:1.5;white-space:pre-wrap;pointer-events:none;' +
    'text-shadow:0 1px 2px rgba(0,0,0,.8);}' +
    // 계절 틴트 — FE SEASON_TINT의 오버레이 포트(겨울 청색 등)
    '#tf-season{position:fixed;inset:0;z-index:24;pointer-events:none;' +
    'mix-blend-mode:multiply;transition:background 2s;}' +
    // 번개 섬광 — storm 국면(체인 이중 타격 포함)
    '#tf-lightning{position:fixed;inset:0;z-index:29;pointer-events:none;' +
    'background:#eaf2ff;opacity:0;}' +
    // 사망 연출 — 🕯 상승 페이드 2.4s + 불씨 파편 7개(FE scene.ts 포트)
    '.tf-candle{position:fixed;z-index:26;pointer-events:none;font-size:18px;' +
    'animation:tf-candle-up 2.4s ease-out forwards;}' +
    '.tf-candle .sp{position:absolute;left:9px;top:9px;width:3px;height:3px;' +
    'border-radius:50%;background:#ffb35c;animation:tf-candle-sp 2.4s ease-out forwards;}' +
    '@keyframes tf-candle-up{0%{opacity:0;transform:translateY(6px);}' +
    '15%{opacity:1;}100%{opacity:0;transform:translateY(-46px);}}' +
    '@keyframes tf-candle-sp{0%{opacity:0;transform:translate(0,0);}' +
    '20%{opacity:.9;}100%{opacity:0;transform:translate(var(--dx),var(--dy));}}';
  document.head.appendChild(st);

  // ═══════════ 주민 앵커 오버레이 ═══════════
  var root = document.createElement('div');
  root.id = 'tf-worldui';
  document.body.appendChild(root);

  var nodes = {};        // id → {el, name, badge(svg·icon·lastIcon·lastW), bubble, zzz, alert, bubbleTimer}
  var proj = {};         // id → {x,y} — Rust 투영(최신)
  var zoom = 1;
  var seen = {};         // 이번 프레임에 투영된 id — 밖/사망 노드 숨김

  function ensureNode(id) {
    var n = nodes[id];
    if (n) return n;
    var el = document.createElement('div');
    el.className = 'tf-wa off';
    var badge = document.createElement('div');
    badge.className = 'tf-wbadge';
    badge.style.display = 'none';
    var row = document.createElement('div');
    row.className = 'tf-wrow';
    var name = document.createElement('div');
    name.className = 'tf-wname';
    var zzz = document.createElement('div');
    zzz.className = 'tf-wzzz';
    zzz.style.display = 'none';
    zzz.textContent = 'z';
    var alert_ = document.createElement('div');
    alert_.className = 'tf-walert';
    alert_.style.display = 'none';
    alert_.textContent = '❗';
    row.appendChild(alert_);
    row.appendChild(name);
    row.appendChild(zzz);
    el.appendChild(badge);
    el.appendChild(row);
    root.appendChild(el);
    n = nodes[id] = {
      el: el, name: name, badge: badge, zzz: zzz, alert: alert_,
      badgeSvg: null, badgeIcon: null, lastIcon: '', lastRing: -1,
      nameText: '', bubble: null, bubbleTimer: 0, bubbleText: '',
    };
    return n;
  }

  /** 진행 링 SVG — r12 뷰박스, 굵기 2+frac*3(FE 동일 수식), 색 #8fe08a */
  function ringSvg(frac) {
    var w = (2 + Math.max(0.02, Math.min(1, frac)) * 3).toFixed(1);
    return '<svg viewBox="0 0 30 30"><circle cx="15" cy="15" r="12" fill="rgba(0,0,0,.45)"' +
      ' stroke="#8fe08a" stroke-width="' + w + '" opacity=".95"/></svg>';
  }

  /** 욕구 경고 판정 — 4종 최댓값 ≥ 90(FE updateActivity 동일 기준) */
  function criticalFlag(id, a) {
    if (!a) return false;
    var needs = a.needs || {};
    return Math.max(
      Number(needs.hunger != null ? needs.hunger : 0),
      Number(needs.thirst != null ? needs.thirst : 0),
      Number(needs.fatigue != null ? needs.fatigue : 0),
      Number(needs.warmth != null ? needs.warmth : 0)) >= 90;
  }

  /** 투영 피드 소비 — 노드 배치(매 프레임) + 상태 갱신(1s 프레임 데이터 기반) */
  function applyProjection() {
    var S = UI() && UI().state;
    var live = S ? S.live : {};
    var selId = S ? S.selected : null;
    var showLabel = zoom <= LABEL_ZOOM_MAX;
    for (var id in nodes) {
      if (!seen[id]) nodes[id].el.classList.add('off');
    }
    for (var id2 in proj) {
      var p = proj[id2];
      var n = ensureNode(id2);
      var a = live[id2];
      var onScreen = p.x > -80 && p.y > -60 && p.x < window.innerWidth + 80 &&
        p.y < window.innerHeight + 60;
      if (!onScreen) {
        n.el.classList.add('off');
        continue;
      }
      n.el.classList.remove('off');
      n.el.style.transform = 'translate(' + p.x.toFixed(1) + 'px,' + p.y.toFixed(1) + 'px) translate(-50%,-100%)';

      // 이름 — 라벨 저줌 숨김(선택 주민은 유지)
      var labelOn = showLabel || id2 === selId;
      n.name.style.display = labelOn ? '' : 'none';
      n.zzz.style.display = (a && a.status === 'sleeping') ? '' : 'none';
      n.alert.style.display = criticalFlag(id2, a) ? '' : 'none';
      if (a && n.nameText !== a.name) {
        n.nameText = a.name;
        n.name.textContent = a.name;
      }
      n.name.classList.toggle('sel', id2 === selId);

      if (!a) continue;

      var act = a.actType || '';
      var prog = Number(a.actProgress != null ? a.actProgress : 0);
      var hasBadge = act && ACT_ICON[act] && prog > 0;
      if (hasBadge) {
        var icon = ACT_ICON[act];
        if (n.lastIcon !== icon || n.badgeIcon == null) {
          n.lastIcon = icon;
          n.badge.innerHTML = ringSvg(prog) + '<span class="ic">' + icon + '</span>';
          n.badgeIcon = n.badge.querySelector('.ic');
          n.badgeSvg = n.badge.querySelector('circle');
          n.lastRing = -1;
        } else if (n.badgeSvg) {
          var w = 2 + Math.max(0.02, Math.min(1, prog)) * 3;
          var w1 = Math.round(w * 10) / 10;
          if (n.lastRing !== w1) {
            n.lastRing = w1;
            n.badgeSvg.setAttribute('stroke-width', String(w1));
          }
        }
        n.badge.style.display = '';
      } else {
        n.badge.style.display = 'none';
        n.lastIcon = '';
      }
    }
  }

  // Rust → JS 투영 브리지 (src/world/overlay.rs overlay_feed)
  tf.onOverlay = function (json) {
    try {
      var f = JSON.parse(json);
    } catch (e) { return; }
    zoom = Number(f.z) || 1;
    viewRect = f.v || null;
    proj = {};
    seen = {};
    var arr = f.a || [];
    for (var i = 0; i < arr.length; i++) {
      proj[arr[i][0]] = { x: arr[i][1], y: arr[i][2] };
      seen[arr[i][0]] = 1;
    }
    applyProjection();
  };

  // ═══════════ 말풍선 — TF_UI.feed.chat wrap(FE showBubble 포트) ═══════════
  function showBubble(m) {
    if (!m || !m.agent_id || !proj[m.agent_id]) return;   // 화면 밖·미스폰은 스킵
    var S = UI() && UI().state;
    var live = S ? S.live : {};
    var n = ensureNode(m.agent_id);
    var isUser = String(m.speaker || '').indexOf('@') >= 0 || m.speaker === 'user' ||
      !live[m.speaker];
    var tag = isUser ? '⟪' + String(m.speaker || '').split('@')[0] + '⟫ ' : '';
    var text = tag + String(m.text || m.text_en || '').slice(0, 60);
    if (n.bubbleText === text && n.bubble) return;   // 에코 확정 재방송 — 무변화
    if (n.bubbleTimer) { clearTimeout(n.bubbleTimer); n.bubbleTimer = 0; }
    if (!n.bubble) {
      n.bubble = document.createElement('div');
      n.el.insertBefore(n.bubble, n.el.firstChild);
    }
    n.bubble.className = 'tf-wbubble' + (isUser ? ' user' : '');
    n.bubble.textContent = text;
    n.bubbleText = text;
    n.bubbleTimer = setTimeout(function () {
      if (!n.bubble) return;
      n.bubble.classList.add('bye');
      var b = n.bubble;
      setTimeout(function () {
        b.remove();
        if (n.bubble === b) { n.bubble = null; n.bubbleText = ''; }
      }, BUBBLE_FADE);
      if (n.bubble === b) n.bubbleTimer = 0;
    }, BUBBLE_LIFE - BUBBLE_FADE);
  }

  // ═══════════ 하이라이트 토스트 — TF_UI.feed.event wrap ═══════════
  var toastHost = document.createElement('div');
  toastHost.id = 'tf-toasts';
  document.body.appendChild(toastHost);

  function pushToast(h) {
    var el = document.createElement('div');
    el.className = 'tf-toast';
    var ic = document.createElement('span');
    ic.className = 'ic';
    ic.textContent = h.icon || '✦';
    var body = document.createElement('span');
    if (h.who) {
      var who = document.createElement('span');
      who.className = 'who';
      who.textContent = h.who + ' — ';
      body.appendChild(who);
    }
    body.appendChild(document.createTextNode(h.text || ''));
    el.appendChild(ic);
    el.appendChild(body);
    toastHost.appendChild(el);
    while (toastHost.children.length > TOAST_MAX) toastHost.removeChild(toastHost.firstChild);
    setTimeout(function () { el.remove(); }, TOAST_LIFE);
  }

  // ── feed wrap — tf-ui.js 로드 이후 원본을 감싸 함께 소비한다 ──
  function wrapFeed() {
    var T = UI();
    if (!T || T.feed.__worldui) return;
    var origFrame = T.feed.frame;
    T.feed.frame = function (f) {
      if (origFrame) origFrame(f);
      syncAmbience();
    };
    var origChat = T.feed.chat;
    T.feed.chat = function (m) {
      if (origChat) origChat(m);
      showBubble(m);
    };
    var origEvent = T.feed.event;
    T.feed.event = function (e) {
      if (origEvent) origEvent(e);
      var i18n = I();
      if (!i18n || !i18n.highlightOf) return;
      var h = i18n.highlightOf(e, {
        locale: i18n.getLocale(),
        tOr: function (k, fb, p) { return i18n.tOr(k, fb, p); },
        nameOf: function (id) {
          var a = T.state.live[id];
          return a ? a.name : '';
        },
      });
      if (h && h.text) pushToast(h);
      if (e && e.kind === 'death') deathCandle(e.agent_id);
    };
    T.feed.__worldui = true;
  }
  wrapFeed();
  // tf-ui.js가 늦게 로드되는 경합 방지(현재 순서는 본 파일이 뒤지만 방어)
  document.addEventListener('DOMContentLoaded', wrapFeed);

  // ═══════════ 계절 틴트 · 번개 · 사망 연출(FE scene·weatherfx 포트) ═══════════
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

  // 번개 — storm에서만. 3.5~9s 간격, 25% 이중 타격(FE flashLightning 체인)
  var boltTimer = 0;
  function scheduleLightning(weather) {
    var storm = weather === 'storm';
    if (boltTimer) { clearTimeout(boltTimer); boltTimer = 0; }
    if (!storm) { lightningEl.style.opacity = '0'; return; }
    boltTimer = setTimeout(function () {
      flashBolt(0);
      if (Math.random() < 0.25) setTimeout(function () { flashBolt(0.12); }, 120 + Math.random() * 180);
      scheduleLightning('storm');
    }, 3500 + Math.random() * 5500);
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
    var p = proj[agentId];
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

  // ═══════════ 자원 툴팁 — §17.78 "누구 집"·묘지 비문·유물 설명 ═══════════
  var tipEl = document.createElement('div');
  tipEl.id = 'tf-restip';
  document.body.appendChild(tipEl);
  var tipLockUntil = 0;   // 터치 고정 해제 시각(ms)

  /** 표시 번역(FE mtTooltip) — 비동기 캐시 충전, 이번 틱은 원문 */
  function mtTip(text) {
    var i18n = I();
    if (!i18n || i18n.getLocale() === 'en' || !text) return String(text);
    i18n.translateText(i18n.getLocale(), String(text)).catch(function () {});
    var cached = i18n.cachedText ? i18n.cachedText(i18n.getLocale(), String(text)) : null;
    return cached != null ? cached : String(text);
  }

  function resTipText(r) {
    var i18n = I();
    var st = r.state || {};
    if (r.kind === 'hut') {
      var bld = i18n ? i18n.tOr('bld.' + String(st.type || 'HUT'), String(st.type || 'HUT')) : String(st.type || 'HUT');
      var done = st.stage === 'COMPLETED';
      var owner = st.ownerName || st.owner;
      if (done) {
        return owner
          ? i18n.t('tip.hut.home', { name: String(owner), bld: bld, x: r.x, y: r.y })
          : i18n.t('tip.hut.vacant', { bld: bld, x: r.x, y: r.y });
      }
      return i18n.t('tip.hut.site', {
        name: String(st.by || '?'), bld: bld,
        p: Math.round(Number(st.progress || 0) * 100),
      });
    }
    if (r.kind === 'grave') {
      var epitaph = st.epitaph ? '\n「' + mtTip(String(st.epitaph).slice(0, 60)) + '」' : '';
      return i18n.t('tip.grave', {
        name: String(st.name || i18n.t('tip.grave.someone')),
        age: st.age != null ? st.age : '?',
      }) + epitaph;
    }
    if (r.kind === 'relic') {
      var purpose = st.purpose ? '\n' + mtTip(String(st.purpose).slice(0, 200)) : '';
      return i18n.t('tip.relic', { by: String(st.by || '') }) + purpose;
    }
    return null;
  }

  function showTip(r, x, y) {
    var i18n = I();
    if (!i18n || !r) { tipEl.style.display = 'none'; return; }
    var text = resTipText(r);
    if (!text) { tipEl.style.display = 'none'; return; }
    tipEl.textContent = text;
    tipEl.style.display = 'block';
    var w = tipEl.offsetWidth, h = tipEl.offsetHeight;
    tipEl.style.left = Math.min(x + 16, window.innerWidth - w - 8) + 'px';
    tipEl.style.top = Math.max(4, y - 8 - h) + 'px';
    // 자유문(비문·설명) 번역 완료 후 같은 대상이면 재렌더(FE mtTooltip 패턴)
    var st = r.state || {};
    var free = st.epitaph || st.purpose;
    if (free && i18n.getLocale() !== 'en') {
      i18n.translateText(i18n.getLocale(), String(free)).then(function (out) {
        if (out !== free && lastHover === r.id && tipEl.style.display === 'block') {
          showTip(r, curMouseX, curMouseY);
        }
      }).catch(function () {});
    }
  }

  // Rust hover 브리지 — 마우스 커서 아래 자원(overlay.rs hover_res)
  var lastHover = null;
  tf.onHoverRes = function (id) {
    lastHover = id;
    if (id == null) {
      // 터치 고정(2.5s) 중에는 해제 통보를 무시 — 고정이 우선한다
      if (Date.now() >= tipLockUntil) tipEl.style.display = 'none';
      return;
    }
    var S = UI() && UI().state;
    var r = S && S.resMap ? S.resMap[id] : null;
    if (!r) { tipEl.style.display = 'none'; return; }
    showTip(r, curMouseX, curMouseY);
  };
  // 커서 위치 추적 — Rust 통보 프레임 사이에서도 툴팁이 커서를 따라가게
  var curMouseX = 0, curMouseY = 0;
  window.addEventListener('mousemove', function (e) {
    curMouseX = e.clientX;
    curMouseY = e.clientY;
    if (Date.now() >= tipLockUntil && tipEl.style.display === 'block' && lastHover != null) {
      var S = UI() && UI().state;
      var r = S && S.resMap ? S.resMap[lastHover] : null;
      showTip(r, curMouseX, curMouseY);
    }
  });

  /** 탭 고정 툴팁(터치) — 2.5s 후 자동 해제(FE pointerdown 패턴). tf-ui.pickAt이 호출. */
  function showResAt(x, y) {
    var S = UI() && UI().state;
    if (!S || !S.resMap) return false;
    var tx = Math.floor(x), ty = Math.floor(y);
    var best = null, bestD = 1.44;
    for (var rid in S.resMap) {
      var r = S.resMap[rid];
      var st = r.state || {};
      if (r.kind === 'hut') {
        var w = Number.isInteger(st.w) && st.w >= 1 ? st.w : (st.type === 'LONGHOUSE' ? 3 : st.type === 'WAREHOUSE' ? 3 : st.type === 'WINDBREAK' ? 1 : 2);
        var h = Number.isInteger(st.h) && st.h >= 1 ? st.h : (st.type === 'WAREHOUSE' ? 3 : st.type === 'WINDBREAK' ? 1 : 2);
        if (tx >= r.x && tx < r.x + w && ty >= r.y && ty < r.y + h) { best = r; break; }
      } else if (r.kind === 'grave' || r.kind === 'relic') {
        var d = (r.x + 0.5 - x) * (r.x + 0.5 - x) + (r.y + 0.5 - y) * (r.y + 0.5 - y);
        if (d < bestD) { bestD = d; best = r; }
      }
    }
    if (!best) return false;
    showTip(best, curMouseX || window.innerWidth / 2, curMouseY || 120);
    tipLockUntil = Date.now() + 2500;
    setTimeout(function () {
      if (Date.now() >= tipLockUntil - 50) tipEl.style.display = 'none';
    }, 2550);
    return true;
  }

  // ═══════════ 미니맵 — 지형 1회 베이크 + 점·뷰포트 + 클릭 점프 ═══════════
  var viewRect = null;   // [cx, cy, w, h] 타일 단위(Rust onOverlay)
  var mmHost = null, mmCvs = null, mmBake = null;
  var mmTimer = 0;

  function mmSide() { return window.innerWidth < 640 ? MM_MOB : MM_DESK; }

  function mmBakeTerrain() {
    var raw = window.TF_MAP_JSON;
    if (!raw) return false;
    var map;
    try { map = JSON.parse(raw); } catch (e) { return false; }
    var W = map.width || 128, H = map.height || 128;
    var data = (map.layers && map.layers[0] && map.layers[0].data) || [];
    var c = document.createElement('canvas');
    c.width = W;
    c.height = H;
    var ctx = c.getContext('2d');
    for (var y = 0; y < H; y++) {
      for (var x = 0; x < W; x++) {
        var g = data[y * W + x] || 0;
        ctx.fillStyle = TERRAIN_COLORS[g] || '#8dc435';
        ctx.fillRect(x, y, 1, 1);
      }
    }
    mmBake = c;
    return true;
  }

  function mmDraw() {
    if (!mmBake || !mmCvs) return;
    var S = UI() && UI().state;
    var ctx = mmCvs.getContext('2d');
    var Sc = mmCvs.width;
    var W = mmBake.width;
    var k = Sc / W;
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, Sc, Sc);
    ctx.drawImage(mmBake, 0, 0, Sc, Sc);
    // 자원 색 — 나무·광맥(종별)·식물(FE WorldMiniMap 동일 팔레트)
    if (S) {
      for (var rid in S.resMap) {
        var r = S.resMap[rid];
        if (r.kind === 'tree') ctx.fillStyle = '#2f5d2a';
        else if (r.kind === 'gold_vein') {
          ctx.fillStyle = r.species === 'quartz_vein' ? '#d8d8e0'
            : r.species === 'clay_bed' ? '#b5651d'
            : r.species === 'obsidian_vein' ? '#5a3a6e'
            : '#f0c02e';
        } else if (r.kind === 'plant') {
          ctx.fillStyle = (r.species === 'reed' || r.species === 'water_dropwort')
            ? '#c8b878' : '#b8e986';
        } else continue;
        ctx.fillRect(r.x * k, r.y * k, Math.max(1, k), Math.max(1, k));
      }
    }
    // 뷰포트 박스 — 이중 스트로크(어두운 외곽 + 노랑 코어)
    if (viewRect) {
      var rx = (viewRect[0] - viewRect[2] / 2) * k;
      var ry = (viewRect[1] - viewRect[3] / 2) * k;
      var vw = Math.max(6, viewRect[2] * k);
      var vh = Math.max(6, viewRect[3] * k);
      ctx.lineWidth = 3;
      ctx.strokeStyle = 'rgba(20,24,16,.65)';
      ctx.strokeRect(rx, ry, vw, vh);
      ctx.lineWidth = 1.5;
      ctx.strokeStyle = 'rgba(255,224,138,.98)';
      ctx.strokeRect(rx, ry, vw, vh);
    }
    // 주민 점 — 선택 흰색
    if (S) {
      for (var id in S.live) {
        var a = S.live[id];
        var sel = id === S.selected;
        ctx.beginPath();
        ctx.arc(a.x * k, a.y * k, sel ? 3.5 : 2.5, 0, 6.283);
        ctx.fillStyle = sel ? '#ffffff' : '#ff6b6b';
        ctx.fill();
        ctx.lineWidth = 1;
        ctx.strokeStyle = 'rgba(16,22,15,.7)';
        ctx.stroke();
      }
    }
  }

  function mmEnsure() {
    if (mmHost) return;
    if (!mmBakeTerrain()) return;   // 맵 도착 전 — 다음 틱 재시도
    var side = mmSide();
    mmHost = document.createElement('div');
    mmHost.id = 'tf-minimap';
    mmHost.title = 'Minimap';
    mmCvs = document.createElement('canvas');
    mmCvs.width = side;
    mmCvs.height = side;
    mmHost.appendChild(mmCvs);
    document.body.appendChild(mmHost);
    mmHost.addEventListener('click', function (e) {
      if (!mmBake || !tf.panTo) return;
      var r = mmHost.getBoundingClientRect();
      var x = (e.clientX - r.left) / r.width * mmBake.width;
      var y = (e.clientY - r.top) / r.height * mmBake.height;
      tf.panTo(x, y);
    });
    mmDraw();
  }

  mmTimer = setInterval(function () {
    mmEnsure();
    mmDraw();
    // 창 폭 전환 — 데스크톱 148/모바일 112px 재적용(FE SIDE_DESK/SIDE_MOB)
    if (mmHost) {
      var side = mmSide();
      if (mmCvs.width !== side) {
        mmCvs.width = side;
        mmCvs.height = side;
        mmHost.style.width = side + 'px';
        mmHost.style.height = side + 'px';
      }
    }
  }, 500);

  // 로케일 전환 — 라벨·배지 문자열은 상태 기반이라 다음 갱신에 자동 반영
  window.TF_WORLD_UI = { showResAt: showResAt };
})();
