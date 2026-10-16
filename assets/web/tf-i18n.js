// Terrafolk 웹 UI i18n 런타임 — FE src/lib/i18n.ts·eventRender.ts·worldsim/solar.ts·
// translate.ts·pages/worldsim/parts.tsx(DayDial·highlightOf)의 바닐라 JS 포트.
// 사전 데이터는 tf-data.js(TF_DATA) — 로드 순서: tf-data → tf-i18n.
(function () {
  'use strict';
  var DATA = window.TF_DATA;
  var IDX = { ko: 0, en: 1, zh: 2, ja: 3 };

  // ---- 사전 병합(FE i18n.ts와 동일 — VOCAB을 D 위로 덮음) ----
  var D = Object.assign({}, DATA.D);
  Object.keys(DATA.VOCAB).forEach(function (k) { D[k] = DATA.VOCAB[k]; });

  // ---- 로케일 스토어 ----
  var STORAGE_KEY = 'ws.locale';
  var LOCALES = [
    { code: 'ko', label: '한국어', flag: '🇰🇷' },
    { code: 'en', label: 'English', flag: '🇺🇸' },
    { code: 'zh', label: '中文', flag: '🇨🇳' },
    { code: 'ja', label: '日本語', flag: '🇯🇵' },
  ];
  function detect() {
    try {
      var saved = localStorage.getItem(STORAGE_KEY);
      if (saved === 'ko' || saved === 'en' || saved === 'zh' || saved === 'ja') return saved;
    } catch (e) { /* 저장소 차단 */ }
    var lang = (navigator.language || '').toLowerCase();
    if (lang.indexOf('ko') === 0) return 'ko';
    if (lang.indexOf('zh') === 0) return 'zh';
    if (lang.indexOf('ja') === 0) return 'ja';
    return 'en';
  }
  var current = detect();
  var listeners = new Set();
  function getLocale() { return current; }
  function setLocale(l) {
    current = l;
    try { localStorage.setItem(STORAGE_KEY, l); } catch (e) { /* 무시 */ }
    document.documentElement.lang = l;
    document.title = trNow('world.tabtitle');
    listeners.forEach(function (f) { f(); });
  }
  function onLocale(fn) { listeners.add(fn); return function () { listeners.delete(fn); }; }

  function translate(locale, key, params) {
    var entry = D[key];
    var s = entry ? (entry[IDX[locale]] != null ? entry[IDX[locale]] : entry[0]) : key;
    if (params) {
      Object.keys(params).forEach(function (k) {
        s = s.split('{' + k + '}').join(String(params[k]));
      });
    }
    return s;
  }
  function trNow(key, params) { return translate(current, key, params); }
  function trOr(key, fallback, params) {
    return D[key] ? translate(current, key, params) : fallback;
  }

  // ---- 이벤트 키 렌더(FE eventRender.ts 포트) ----
  function label4(prefix, id, locale) {
    var t = DATA.VOCAB[prefix + '.' + id];
    if (!t) return id;
    return t[IDX[locale] != null ? IDX[locale] : 1] != null ? t[IDX[locale] != null ? IDX[locale] : 1] : t[0];
  }
  function hasBatchim(s) {
    var code = s.charCodeAt(s.length - 1);
    if (code < 0xac00 || code > 0xd7a3) return false;
    return (code - 0xac00) % 28 > 0;
  }
  var JOSA = {
    '이가': function (s) { return hasBatchim(s) ? '이가' : '가'; },
    '을': function (s) { return hasBatchim(s) ? '을' : '를'; },
    '은': function (s) { return hasBatchim(s) ? '은' : '는'; },
    '와': function (s) { return hasBatchim(s) ? '과' : '와'; },
    '으로': function (s) { return hasBatchim(s) ? '으로' : '로'; },
    '이': function (s) { return hasBatchim(s) ? '이' : '가'; },
  };
  function resolveParams(tmpl, params, locale) {
    var out = tmpl;
    Object.keys(params || {}).forEach(function (name) {
      var raw = params[name];
      var s = String(raw == null ? '' : raw);
      var val = s;
      if (name === 'item' || name === 'item2') val = label4('item', s, locale);
      else if (name === 'species') val = label4('sp', s, locale);
      else if (name === 'dir') val = label4('dir', s, locale);
      else if (name === 'skill') val = D['act.' + s] ? translate(locale, 'act.' + s) : s;
      else if (name === 'cat') val = label4('invcat', s, locale);
      else if (name === 'era') val = label4('era', s, locale);
      else if (name === 'stage') {
        var seg = s.split(':');
        var bs = DATA.VOCAB['bldstage.' + seg[0] + '.' + seg[1]];
        val = seg[1] ? (bs ? (bs[IDX[locale] != null ? IDX[locale] : 1] != null ? bs[IDX[locale] != null ? IDX[locale] : 1] : bs[0]) : s) : s;
      } else if (name === 'label') {
        var kv = s.split(':');
        val = kv[0] === 'kind' ? label4('kind', kv[1] || '', locale) : label4('sp', kv[1] || '', locale);
      } else if (name === 'why') {
        val = String(raw == null ? '' : raw).trim() ? ' — ' + String(raw).trim() : '';
      } else if (name === 'acts') {
        val = s.split(',').filter(Boolean).map(function (a) {
          return D['act.' + a] ? translate(locale, 'act.' + a) : a;
        }).join('·');
      } else if (name === 'bases') {
        val = s.split(',').filter(Boolean).map(function (i) { return label4('item', i, locale); }).join('/');
      } else if (name === 'seasons') {
        val = s.split(',').filter(Boolean).map(function (x) {
          return D['season.' + x] ? translate(locale, 'season.' + x) : x;
        }).join('·');
      } else if (name === 'itemqtys') {
        val = s.split(',').filter(function (kv2) { return kv2.indexOf(':') >= 0; }).map(function (kv2) {
          var p = kv2.split(':');
          return label4('item', p[0], locale) + ' ' + p[1] + (locale === 'ko' ? '개' : '');
        }).join(', ');
      }
      Object.keys(JOSA).forEach(function (josa) {
        var token = '{' + name + josa + '}';
        if (out.indexOf(token) >= 0) out = out.split(token).join(val + JOSA[josa](val));
      });
      out = out.split('{' + name + '}').join(val);
    });
    return out;
  }
  function renderEventKey(key, params, locale, fallback) {
    var entry = DATA.EVENT_DICT[key];
    if (!entry) return fallback || '';
    var idx = IDX[locale] != null ? IDX[locale] : 1;
    var tmpl = entry[idx] != null ? entry[idx] : entry[0];
    return resolveParams(tmpl, params, locale);
  }
  /** 이벤트 payload → 문장 — key 우선, think는 goalKey, 없으면 text(영문 정본). */
  function eventText(p, locale, fb) {
    if (p.key) {
      var out = renderEventKey(p.key, p.params, locale, '');
      if (out) return out;
    }
    if (p.goalKey) {
      var out2 = renderEventKey(p.goalKey, p.goalParams, locale, '');
      if (out2) return out2;
    }
    return String(p.text != null ? p.text : (p.goal != null ? p.goal : (fb || '')));
  }

  // ---- 표시 계층 번역(FE translate.ts 포트 — 디바운스 배치 + 공유 캐시) ----
  var API_BASE = '';
  var mtCache = new Map();
  var mtPending = new Map();
  var mtQueue = [];
  var mtTimer = null;
  function mtFlush() {
    mtTimer = null;
    API_BASE = (window.TF && window.TF.apiBase) || '';
    var batch = mtQueue.splice(0, 40);
    if (!batch.length) return;
    var byLocale = new Map();
    batch.forEach(function (b) {
      if (!byLocale.has(b.locale)) byLocale.set(b.locale, []);
      byLocale.get(b.locale).push(b);
    });
    byLocale.forEach(function (rows, locale) {
      var seen = Object.create(null);
      var texts = rows.map(function (r) { return r.text; }).filter(function (t) {
        if (seen[t]) return false;
        seen[t] = 1;
        return true;
      });
      fetch(API_BASE + '/api/worldsim/translate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ texts: texts, target: locale }),
      }).then(function (r) { return r.ok ? r.json() : null; }).then(function (data) {
        var map = (data && data.texts) || {};
        rows.forEach(function (r) { r.resolve(map[r.text] || r.text); });
      }).catch(function () {
        rows.forEach(function (r) { r.resolve(r.text); });
      });
    });
    if (mtQueue.length && !mtTimer) mtTimer = setTimeout(mtFlush, 350);
  }
  /** 동기 캐시 조회(툴팁 패턴 — FE cachedText 포트): 없으면 null. */
  function cachedText(locale, text) {
    if (locale === 'en' || !text) return text;
    var v = mtCache.get(locale + '|' + text);
    return v !== undefined ? v : null;
  }
  function translateText(locale, text) {
    if (locale === 'en' || !text) return Promise.resolve(text);
    var key = locale + '|' + text;
    var hit = mtCache.get(key);
    if (hit !== undefined) return Promise.resolve(hit);
    var inflight = mtPending.get(key);
    if (inflight) return inflight;
    var p = new Promise(function (resolve) {
      mtQueue.push({
        locale: locale, text: text,
        resolve: function (s) { mtCache.set(key, s); mtPending.delete(key); resolve(s); },
      });
      if (!mtTimer) mtTimer = setTimeout(mtFlush, 350);
    });
    mtPending.set(key, p);
    return p;
  }

  // ---- 태양력 달력(FE solar.ts dateOf 포트) ----
  var YEAR_DAYS = DATA.CALENDAR.yearDays;
  var MONTH_CUM = DATA.CALENDAR.monthCum;
  function dateOf(day) {
    var d = Math.max(0, Math.floor(day));
    var doy = (d % YEAR_DAYS) + 1;
    var year = Math.floor(d / YEAR_DAYS) + 1;
    var month = 2, dayOfMonth = 28;
    for (var i = 0; i < MONTH_CUM.length; i++) {
      if (doy <= MONTH_CUM[i]) {
        month = ((i + 2) % 12) + 1;
        dayOfMonth = doy - (i > 0 ? MONTH_CUM[i - 1] : 0);
        break;
      }
    }
    return { year: year, doy: doy, month: month, dayOfMonth: dayOfMonth };
  }

  // ---- 시계 표시 문자열(FE world.clock 템플릿) ----
  function clockText(clock) {
    var cal = clock.day !== undefined ? dateOf(clock.day) : null;
    return trNow('world.clock', {
      season: clock.season ? trNow('season.' + clock.season) : '—',
      year: cal ? cal.year : 1,
      month: cal ? cal.month : 3,
      day: cal ? cal.dayOfMonth : 1,
      hour: String(clock.hour != null ? clock.hour : '').padStart(2, '0'),
      minute: String(clock.minute != null ? clock.minute : 0).padStart(2, '0'),
    });
  }

  // ---- DayDial(FE parts.tsx 포트) — 해/달 위치계 SVG 마크업 반환 ----
  function dayDialSvg(hour, minute, sunrise, sunset) {
    var W = 68, H = 18;
    var h = (((hour != null ? hour : 12) + (minute != null ? minute : 0) / 60) % 24 + 24) % 24;
    var rise = (sunrise !== undefined && sunset !== undefined) ? sunrise / 60 : 5;
    var set_ = (sunrise !== undefined && sunset !== undefined) ? sunset / 60 : 20;
    var dayLen = Math.max(1, set_ - rise);
    var nightLen = 24 - dayLen;
    var isDay = h >= rise && h < set_;
    var t = isDay ? (h - rise) / dayLen : ((((h - set_) % 24) + 24) % 24) / nightLen;
    var tt = Math.max(0, Math.min(1, t));
    var x = 3 + tt * (W - 6);
    var y = H - 3 - Math.sin(tt * Math.PI) * (H - 8);
    var arc = [];
    for (var i = 0; i <= 12; i++) {
      var p = i / 12;
      arc.push((3 + p * (W - 6)).toFixed(1) + ',' + (H - 3 - Math.sin(p * Math.PI) * (H - 8)).toFixed(1));
    }
    var body;
    if (isDay) {
      var rays = [0, 90, 180, 270].map(function (a) {
        var rad = (a * Math.PI) / 180;
        return '<line x1="' + (x + Math.cos(rad) * 4.6).toFixed(2) + '" y1="' + (y + Math.sin(rad) * 4.6).toFixed(2) +
          '" x2="' + (x + Math.cos(rad) * 5.8).toFixed(2) + '" y2="' + (y + Math.sin(rad) * 5.8).toFixed(2) +
          '" stroke="#ffd166" stroke-width="1"/>';
      }).join('');
      body = '<circle cx="' + x.toFixed(2) + '" cy="' + y.toFixed(2) + '" r="3.4" fill="#ffd166"/>' + rays;
    } else {
      body = '<circle cx="' + x.toFixed(2) + '" cy="' + y.toFixed(2) + '" r="3.2" fill="#dbe4f0"/>' +
        '<circle cx="' + (x + 1).toFixed(2) + '" cy="' + (y - 0.8).toFixed(2) + '" r="0.8" fill="#9aa7bd"/>' +
        '<circle cx="' + (x - 0.9).toFixed(2) + '" cy="' + (y + 0.9).toFixed(2) + '" r="0.6" fill="#9aa7bd"/>';
    }
    return '<svg width="' + W + '" height="' + H + '" aria-hidden="true" class="tf-daydial">' +
      '<polyline points="' + arc.join(' ') + '" fill="none" stroke="rgba(148,163,184,0.35)" stroke-width="1"/>' +
      '<line x1="0" y1="' + (H - 2) + '" x2="' + W + '" y2="' + (H - 2) + '" stroke="rgba(148,163,184,0.22)" stroke-width="1"/>' +
      body + '</svg>';
  }

  // ---- 하이라이트 판정(FE parts.tsx highlightOf 포트) ----
  var WEATHER_ICON = {
    clear: '☀️', overcast: '☁️', rain: '🌧️', storm: '⛈️', snow: '🌨️', fog: '🌫️',
  };
  /** 우하단 토스트·기록 하이라이트 탭 공유 판정. null이면 대상 아님(잡음). */
  function highlightOf(e, o) {
    var p = e.payload || {};
    var who = e.agent_id ? o.nameOf(e.agent_id) : '';
    var icon = '', text = '';
    if (e.kind === 'relation') { icon = '💖'; text = eventText(p, o.locale); who = ''; }
    else if (e.kind === 'trade' && p.ok) { icon = '🤝'; text = eventText(p, o.locale); }
    else if (e.kind === 'give' && p.ok) { icon = '💝'; text = eventText(p, o.locale); }
    else if (e.kind === 'skill') { icon = '⭐'; text = eventText(p, o.locale); }
    else if (e.kind === 'teach' && p.ok) { icon = '🎓'; text = eventText(p, o.locale); }
    else if (e.kind === 'inherit') { icon = '🤲'; text = eventText(p, o.locale); who = ''; }
    else if (e.kind === 'birth' && p.ok) { icon = '👶'; text = eventText(p, o.locale); who = ''; }
    else if (e.kind === 'routine') { icon = '🧭'; text = eventText(p, o.locale); who = ''; }
    else if (e.kind === 'perform' && p.ok) { icon = '🎭'; text = eventText(p, o.locale); }
    else if (e.kind === 'chronicle') { icon = '📜'; text = eventText(p, o.locale); who = ''; }
    else if (e.kind === 'home') { icon = '🏡'; text = eventText(p, o.locale); who = ''; }
    else if (e.kind === 'spawn') { icon = '👋'; text = (String(p.name || '') + ' — ' + who).trim(); }
    else if (e.kind === 'result' && p.action === 'campfire' && p.ok) { icon = '🔥'; text = eventText(p, o.locale); }
    else if (e.kind === 'result' && (p.action === 'drink' || p.action === 'fetch') && p.ok) { icon = '💧'; text = eventText(p, o.locale); }
    else if (e.kind === 'result' && p.action === 'thirst' && !p.ok) { icon = '🥵'; text = eventText(p, o.locale); }
    else if (e.kind === 'result' && (p.action === 'store' || p.action === 'withdraw') && p.ok) { icon = '📦'; text = eventText(p, o.locale); }
    else if (e.kind === 'result' && (p.action === 'drop' || p.action === 'pickup') && p.ok) { icon = '🎒'; text = eventText(p, o.locale); }
    else if (p.carry) { icon = '⚖️'; text = eventText(p, o.locale); }
    else if (e.kind === 'result' && p.action === 'wear' && p.ok) { icon = '🧥'; text = eventText(p, o.locale); }
    else if (e.kind === 'result' && p.action === 'build' && p.ok) { icon = '🏠'; text = eventText(p, o.locale); }
    else if (e.kind === 'invention') { icon = '🛠'; text = eventText(p, o.locale); who = ''; }
    else if (e.kind === 'talk') { icon = '💡'; text = eventText(p, o.locale); who = ''; }
    else if (e.kind === 'death') {
      icon = '🕯';
      text = eventText(p, o.locale) || String(p.name || '') + ' (aged ' + (p.age != null ? p.age : '?') + ')';
      who = '';
    }
    else if (e.kind === 'system' && p.weather) { icon = WEATHER_ICON[String(p.weather)] || '🌤'; text = eventText(p, o.locale); who = ''; }
    if (!icon || (!text && !who)) return null;
    return { icon: icon, who: who, text: text };
  }

  // ---- 공용 상수(FE parts.tsx) ----
  var AGENT_COLORS = [
    '#ffd28a', '#9ad0ff', '#a5e8b0', '#f0a8c8', '#d8c2ff', '#ffb0a0', '#a0e8e0', '#e8e09a',
  ];
  function agentColor(id) {
    var h = 0;
    for (var i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0;
    return AGENT_COLORS[h % AGENT_COLORS.length];
  }
  var EV_CAT_OF = {
    relation: 'social', give: 'social', talk: 'social', chat: 'social', perform: 'social',
    act: 'living', result: 'living', trade: 'living', think: 'living',
    skill: 'knowledge', invention: 'knowledge', teach: 'knowledge', inherit: 'knowledge', routine: 'knowledge',
    spawn: 'life', birth: 'life', death: 'life',
    system: 'world', resource: 'world', chronicle: 'world',
  };
  var EV_CATS = ['social', 'living', 'knowledge', 'life', 'world'];
  var EV_KIND_ICON = {
    act: '🏃', think: '💭', result: '✨', resource: '🌱',
    trade: '🤝', give: '💝', relation: '💖', skill: '⭐', spawn: '👋',
    death: '🕯', invention: '🛠', talk: '💡', teach: '🎓', inherit: '🤲',
    birth: '👶', routine: '🧭', system: '🌤', chat: '💬',
    perform: '🎭', chronicle: '📜',
  };
  var EVENT_COLOR = {
    act: '#7dd3fc', result: '#aeb7c6', think: '#c4b5fd',
    trade: '#fcd34d', skill: '#6ee7b7', chat: '#ccd3de',
    resource: '#bef264', spawn: '#6b7689', system: '#4a5468',
    relation: '#f9a8d4', death: '#d4d4d8', give: '#f9a8d4',
    invention: '#fde68a', teach: '#a7f3d0', inherit: '#bae6fd',
    birth: '#fbcfe8', routine: '#ddd6fe', talk: '#fef3c7',
    perform: '#f5d0fe', chronicle: '#fef3c7',
  };
  var HIST_COLOR = {
    gain: '#6ee7b7', trade: '#fcd34d', craft: '#7dd3fc', consume: '#fda4af',
    '획득': '#6ee7b7', '거래': '#fcd34d', '제작': '#7dd3fc', '소모': '#fda4af',
  };
  var RES_KIND_ICON = {
    tree: '🌳', plant: '🌿', gold_vein: '✨', animal: '🦌', water: '💧',
    hut: '🏠', campfire: '🔥', relic: '⚒', grave: '🪦',
  };
  var INV_SEED_ICON = {
    campfire: '🔥', windbreak: '🛖', hut: '🏠', longhouse: '🏘️', warehouse: '🏬',
    WINDBREAK: '🛖', HUT: '🏠', LONGHOUSE: '🏘️', WAREHOUSE: '🏬',
  };
  var PERSONA_PRESETS = [
    { key: 'kin', emoji: '🩸', text: 'Devotes deeply only to family and close kin. Gives freely and takes risks for blood, but is wary of strangers' },
    { key: 'reciprocal', emoji: '🤝', text: 'Values fairness — returns kindness twofold, but remembers betrayal long and keeps distance' },
    { key: 'universal', emoji: '🌍', text: 'Helps anyone who goes hungry. Generous even to strangers, puts the whole village first' },
    { key: 'selfish', emoji: '🐺', text: 'My survival comes first. Helps others only when it returns to me — a quick-witted survivor who takes no losses' },
  ];
  function relBucket(w) {
    return w < 30 ? 'bad' : w < 45 ? 'awkward' : w < 65 ? 'normal' : w < 80 ? 'good' : 'best';
  }
  function parseInv(raw) {
    try {
      var v = typeof raw === 'string' ? JSON.parse(raw || '{}') : raw;
      return v && typeof v === 'object' ? v : {};
    } catch (e) { return {}; }
  }
  function pad2(n) { return String(n).padStart(2, '0'); }
  function fmtTime(ts) {
    var d = new Date(ts);
    return isNaN(d.getTime()) ? '' : pad2(d.getHours()) + ':' + pad2(d.getMinutes());
  }
  /** 채팅 gmin(발화 시점 게임 분) → 게임 시각 표기(FE §17.37 규약). */
  function fmtGmin(gmin) {
    if (gmin == null) return '';
    var day = Math.floor(gmin / 1440);
    var min = gmin % 1440;
    var cal = dateOf(day);
    return 'D' + day + ' ' + pad2(Math.floor(min / 60)) + ':' + pad2(min % 60) + ' · Y' + cal.year;
  }

  window.TF_I18N = {
    D: D, IDX: IDX, LOCALES: LOCALES,
    getLocale: getLocale, setLocale: setLocale, onLocale: onLocale,
    t: trNow, tOr: trOr, translate: translate,
    eventText: eventText, renderEventKey: renderEventKey,
    translateText: translateText,
    cachedText: cachedText,
    dateOf: dateOf, clockText: clockText, dayDialSvg: dayDialSvg,
    highlightOf: highlightOf, agentColor: agentColor,
    EV_CAT_OF: EV_CAT_OF, EV_CATS: EV_CATS, EV_KIND_ICON: EV_KIND_ICON, EVENT_COLOR: EVENT_COLOR,
    HIST_COLOR: HIST_COLOR, RES_KIND_ICON: RES_KIND_ICON, INV_SEED_ICON: INV_SEED_ICON,
    WEATHER_ICON: WEATHER_ICON, PERSONA_PRESETS: PERSONA_PRESETS,
    relBucket: relBucket, parseInv: parseInv, fmtTime: fmtTime, fmtGmin: fmtGmin,
  };
})();
