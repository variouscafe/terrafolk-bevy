// Terrafolk ☰ 메뉴·중앙 팝업 DOM 오버레이 — FE worldsim 화면(TopBar·메뉴 드롭다운·
// 팝업 6종·크레딧·외형 편집)의 바닐라 JS 포트. 상태는 worldsim.js(룸 접속)가
// TF_UI.feed.*로 밀어주고, 이 모듈이 렌더·REST(선택 상세·참여·외형)·로케일을 소유한다.
// 채팅 바·대화 로그 독은 ui.js 담당 — 선택 단일 소스는 Rust 픽킹
// (tf.selectAgent 요청 → tf.onPickAgent 통보, src/world/pick.rs). 이 파일은 로드
// 순서상 ui.js 뒤라 기존 onPickAgent 핸들러를 chaining으로 이어받는다.
// 로드 순서: tf-data → tf-i18n → tf-look → ui.js → (tf-ui).
(function () {
  'use strict';
  var I = window.TF_I18N;
  var DATA = window.TF_DATA;

  // ═══════════════ 상태 ═══════════════
  var S = {
    menuOpen: false,
    popup: null,            // 'chars'|'res'|'inventions'|'log'|'set'|'account'
    selected: null,
    selRelic: null,
    selInv: null,
    logCat: 'all',
    notice: '',
    clock: {},
    live: {},               // id → {id,name,status,actType,needs,inv,goalText,mine}
    liveSig: '',
    resMap: {},             // id → {id,kind,species,x,y,state}
    events: [],             // 수신 순(오래된→최신), 렌더에서 reverse
    chat: [],
    relations: [],
    memories: [],
    history: [],
    inventionsAll: [],
    inventions: {},         // name → {tpl,primary,accent,purpose,inventor,based_on}
    scope: null,            // {kind:'user'|'spectator', email?}
    mineDetail: [],
    joinForm: { name: '', age: '30', sex: 'female', persona: '' },
    busy: false,
    lookEdit: null,
    creditsOpen: false,
    netLive: false,
  };
  var rowSeq = 0;
  function nextRowId() {
    rowSeq = (rowSeq + 1) % 10000;
    return Date.now() * 10000 + rowSeq;
  }

  // ═══════════════ 렌더 스케줄 ═══════════════
  var raf = 0;
  function schedule() {
    if (raf) return;
    raf = requestAnimationFrame(function () { raf = 0; render(); });
  }
  // rAF 기아 방지 — 무거운 wasm 렌더 루프·백그라운드 탭에서 rAF가 밀릴 때
  // 대기 중 렌더를 250ms 내 강제 플러시(렌더 대기가 없으면 무동작).
  setInterval(function () {
    if (raf) {
      cancelAnimationFrame(raf);
      raf = 0;
      render();
    }
  }, 250);

  function esc(s) {
    return String(s == null ? '' : s).replace(/[&<>"']/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
    });
  }
  // lucide 아이콘(FE와 동일 모양) — stroke 방식, size px
  var ICONS = {
    Menu: '<line x1="4" x2="20" y1="6" y2="6"/><line x1="4" x2="20" y1="12" y2="12"/><line x1="4" x2="20" y1="18" y2="18"/>',
    Users: '<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/>',
    TreePine: '<path d="m17 14 3 3.3a1 1 0 0 1-.7 1.7H4.7a1 1 0 0 1-.7-1.7L7 14h-.3a1 1 0 0 1-.7-1.7L9 9h-.2A1 1 0 0 1 8 7.3L12 3l4 4.3a1 1 0 0 1-.8 1.7H15l3 3.3a1 1 0 0 1-.7 1.7H17Z"/><path d="M12 22v-3"/>',
    Landmark: '<line x1="3" x2="21" y1="22" y2="22"/><line x1="6" x2="6" y1="18" y2="11"/><line x1="10" x2="10" y1="18" y2="11"/><line x1="14" x2="14" y1="18" y2="11"/><line x1="18" x2="18" y1="18" y2="11"/><polygon points="12 2 20 7 4 7"/>',
    ScrollText: '<path d="M15 12h-5"/><path d="M15 8h-5"/><path d="M19 17V5a2 2 0 0 0-2-2H4"/><path d="M8 21h12a2 2 0 0 0 2-2v-1a1 1 0 0 0-1-1H11a1 1 0 0 0-1 1v1a2 2 0 1 1-4 0V5a2 2 0 1 0-4 0v2a1 1 0 0 0 1 1h3"/>',
    Settings: '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>',
    User: '<path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/>',
    LogIn: '<path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4"/><polyline points="10 17 15 12 15 12 10 7"/><line x1="15" x2="3" y1="12" y2="12"/>',
    LogOut: '<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 21 12 16 7"/><line x1="21" x2="9" y1="12" y2="12"/>',
    X: '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
    MapPin: '<path d="M20 10c0 6-8 12-8 12s-8-6-8-12a8 8 0 0 1 16 0Z"/><circle cx="12" cy="10" r="3"/>',
    ChevronLeft: '<path d="m15 18-6-6 6-6"/>',
    ChevronRight: '<path d="m9 18 6-6-6-6"/>',
    Lock: '<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>',
    UserPlus: '<path d="M2 21a8 8 0 0 1 13.292-6"/><circle cx="10" cy="8" r="5"/><path d="M19 16v6"/><path d="M22 19h-6"/>',
  };
  function svg(paths, size) {
    return '<svg viewBox="0 0 24 24" width="' + size + '" height="' + size +
      '" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
      paths + '</svg>';
  }
  function t(k, p) { return I.t(k, p); }
  function tOr(k, fb, p) { return I.tOr(k, fb, p); }
  var mtMap = {};   // "locale|text" → 번역(완료 후 재렌더)
  function mt(text) {
    var locale = I.getLocale();
    if (locale === 'en' || !text) return text;
    var key = locale + '|' + text;
    if (mtMap[key] !== undefined) return mtMap[key];
    I.translateText(locale, text).then(function (tr) {
      if (tr !== text && mtMap[key] === undefined) {
        mtMap[key] = tr;
        schedule();   // 팝업 몸통·히어로 카드 갱신 — 틱을 기다리지 않는다
      }
    }).catch(function () {});
    return text;
  }

  function apiBase() { return (window.TF && window.TF.apiBase) || ''; }
  function authFetch(path, opts) {
    var f = window.terrafolk && window.terrafolk.authFetch;
    if (f) return f(apiBase() + path, opts);
    return fetch(apiBase() + path, opts);
  }
  function hasToken() {
    return !!(window.terrafolk && window.terrafolk.getAuth && window.terrafolk.getAuth().at);
  }

  // 아이콘 캐시(칩 <img>) — 스펙 → dataURL, 완료 후 재렌더
  var iconUrls = {};
  function chipIconHtml(name, px) {
    px = px || 18;
    if (DATA.ITEM_EMOJI[name]) return '<span>' + DATA.ITEM_EMOJI[name] + '</span>';
    var spec = S.inventions[name];
    if (spec && spec.tpl) {
      var key = name + '|' + px;
      if (iconUrls[key]) {
        return '<img src="' + iconUrls[key] + '" alt="' + esc(name) + '" width="' + px + '" height="' + px +
          '" style="image-rendering:pixelated;border-radius:2px;background:' + spec.accent + '40">';
      }
      window.TF_LOOK.renderItemIcon(spec, 32).then(function (u) {
        if (u && !iconUrls[key]) { iconUrls[key] = u; schedule(); }
      }).catch(function () {});
      return '<span>⚒</span>';
    }
    return '<span>' + (spec ? '⚒' : '🎒') + '</span>';
  }

  // ── 문명 도우미(FE civ.ts 포트) ──
  var CIV_GROUPS = ['living', 'production', 'infrastructure', 'culture', 'knowledge', 'institutions'];
  var ERA_STEPS = [
    { key: 'band', at: 0 }, { key: 'camp', at: 6 }, { key: 'village', at: 12 },
    { key: 'large_village', at: 20 }, { key: 'town', at: 32 }, { key: 'city', at: 48 },
    { key: 'metropolis', at: 70 }, { key: 'megacity', at: 100 }, { key: 'beyond', at: 140 },
  ];
  function invCatId(c) {
    var s = String(c == null ? '' : c).trim().toLowerCase();
    return DATA.INVCAT_ALIAS[s] != null ? DATA.INVCAT_ALIAS[s] : s;
  }
  function invGroupOf(c) { return DATA.INVCAT_GROUPS[invCatId(c)] || ''; }
  function invCatEmoji(c) { return DATA.INVCAT_EMOJI[invCatId(c)] || '⚒️'; }
  function civScore(list) {
    var groups = {};
    var n = 0;
    list.forEach(function (i) { var g = invGroupOf(i.category); if (g && !groups[g]) { groups[g] = 1; n++; } });
    return list.length + n * 2;
  }
  function eraOf(score) {
    var key = 'band';
    ERA_STEPS.forEach(function (s) { if (score >= s.at) key = s.key; });
    return key;
  }
  function civGroupStats(list) {
    return CIV_GROUPS.map(function (group) {
      var cats = {};
      var total = 0;
      list.forEach(function (inv) {
        if (invGroupOf(inv.category) !== group) return;
        var cat = invCatId(inv.category);
        (cats[cat] = cats[cat] || []).push(inv);
        total++;
      });
      return { group: group, total: total, cats: cats };
    });
  }

  // ── 자원 이용가능 판정(FE resUsable 포트) ──
  function resUsable(kind, state, season) {
    if (!state) return true;
    if (kind === 'tree') return Number(state.maturity != null ? state.maturity : 0) >= 2;
    if (kind === 'gold_vein') return Number(state.amount != null ? state.amount : 0) > 0;
    if (kind === 'plant') {
      if (!state.available) return false;
      var seasons = state.seasons;
      if (Array.isArray(seasons) && seasons.length > 0 && season) return seasons.indexOf(season) >= 0;
      return true;
    }
    if (kind === 'animal') return state.alive !== false;
    return true;
  }
  function resStats() {
    var byKind = {};
    Object.keys(S.resMap).forEach(function (rid) {
      var r = S.resMap[rid];
      if (r.kind === 'campfire' || r.kind === 'hut') return;   // 발명 도감으로(§17.38)
      var k = byKind[r.kind];
      if (!k) k = byKind[r.kind] = { kind: r.kind, total: 0, usable: 0, species: {} };
      var u = resUsable(r.kind, r.state, S.clock.season);
      k.total++;
      if (u) k.usable++;
      var s = k.species[r.species];
      if (!s) s = k.species[r.species] = { n: 0, usable: 0 };
      s.n++;
      if (u) s.usable++;
    });
    return Object.keys(byKind).map(function (k) { return byKind[k]; })
      .sort(function (a, b) { return b.total - a.total; });
  }

  function isHumanRow(c) {
    var a = S.live[c.agent_id];
    return String(c.speaker || '').indexOf('@') >= 0 || (a !== undefined && c.speaker !== a.name);
  }
  function isAlreadyKorean(s) {
    var h = (String(s).match(/[가-힣]/g) || []).length;
    var l = (String(s).match(/[A-Za-z]/g) || []).length;
    return h > 0 && h >= l * 3;
  }
  function chatDisplayText(cm) {
    var src = cm.orig != null ? cm.orig : cm.text;
    var lang = cm.lang || 'en';
    var locale = I.getLocale();
    if (locale === lang) return src;
    if (locale === 'en' && cm.text_en) return String(cm.text_en);
    if (locale === 'ko' && isAlreadyKorean(src)) return src;
    return mt(src);
  }

  // ═══════════════ 상단 바 ═══════════════
  function ensureRoot() {
    var root = document.getElementById('tf-ui');
    if (!root) {
      root = document.createElement('div');
      root.id = 'tf-ui';
      document.body.appendChild(root);
    }
    return root;
  }

  function topbarHtml() {
    var clock = S.clock;
    var n = Object.keys(S.live).length;
    var isUser = S.scope && S.scope.kind === 'user';
    var weather = clock.weather
      ? '<span class="tf-chip">' + (I.WEATHER_ICON[clock.weather] || '') + ' ' +
        tOr('weather.' + clock.weather, clock.weather) + '</span>'
      : '';
    var menuItems = [
      ['chars', t('menu.chars'), ICONS.Users],
      ['res', t('menu.resources'), ICONS.TreePine],
      ['inventions', t('menu.civilization'), ICONS.Landmark],
      ['log', t('menu.log'), ICONS.ScrollText],
      ['set', t('menu.settings'), ICONS.Settings],
      ['account', hasToken() ? t('menu.account') : t('menu.login'), hasToken() ? ICONS.User : ICONS.LogIn],
    ].map(function (m) {
      return '<button class="tf-menu-item" data-act="popup" data-key="' + m[0] + '">' +
        svg(m[2], 14) + ' ' + esc(m[1]) + '</button>';
    }).join('');
    return (
      '<div class="tf-topbar">' +
        '<div class="tf-topbar-row">' +
          '<span class="tf-topbar-title">' + esc(t('world.title')) + '</span>' +
          '<span class="tf-clock-wrap">' +
            I.dayDialSvg(clock.hour, clock.minute, clock.sunrise, clock.sunset) +
            '<span class="tf-clock">' + esc(I.clockText(clock)) + '</span>' +
          '</span>' +
          weather +
          '<span class="tf-chip">' + esc(t('world.people', { n: n })) + '</span>' +
          '<span class="tf-chip' + (isUser ? ' tf-chip-user' : '') + '">' +
            esc(isUser ? t('world.joined') : t('world.spectate')) + '</span>' +
          '<span class="tf-topbar-spacer"></span>' +
          '<div class="tf-menu-anchor">' +
            '<button class="tf-menu-btn" data-act="menu" title="' + esc(t('menu.game')) + '">' +
              svg(ICONS.Menu, 16) + '</button>' +
            (S.menuOpen
              ? '<div class="tf-menu">' + menuItems + '</div>'
              : '') +
          '</div>' +
        '</div>' +
        (S.notice
          ? '<div class="tf-notice" data-act="clear-notice">' + esc(S.notice) + '</div>'
          : '') +
      '</div>'
    );
  }

  // ═══════════════ 팝업 ═══════════════
  var POPUP_META = {
    chars: { titleKey: 'menu.chars', width: 'max-w-3xl' },
    res: { titleKey: 'menu.resources', width: 'max-w-lg' },
    inventions: { titleKey: 'menu.civilization', width: 'max-w-3xl' },
    log: { titleKey: 'menu.log', width: 'max-w-3xl' },
    account: { titleKey: 'menu.account', width: 'max-w-sm' },
    set: { titleKey: 'menu.settings', width: 'max-w-sm' },
  };

  function popupBody(key) {
    if (key === 'chars') return charsHtml();
    if (key === 'res') return resHtml();
    if (key === 'inventions') return civHtml();
    if (key === 'log') return logHtml();
    if (key === 'account') return accountHtml();
    return setHtml();
  }

  // 팝업 렌더 — 오버레이·대화상자 뼈대는 유지하고 몸통만 교체한다.
  // worldsim 틱마다 render가 도는데 루트를 통째로 다시 쓰면 입장 애니메이션
  // (tf-fade-in)이 매 틱 재시작돼 팝업이 깜빡인다.
  var popupDom = { key: null, title: null, body: null };
  function popupUpdate() {
    var slot = slots.popup;
    if (!S.popup) {
      if (popupDom.key !== null) {
        popupDom = { key: null, title: null, body: null };
        slot.innerHTML = '';
      }
      return;
    }
    var meta = POPUP_META[S.popup];
    var title = S.popup === 'account' && !hasToken() ? t('menu.login') : t(meta.titleKey);
    if (popupDom.key !== S.popup) {
      popupDom = { key: S.popup, title: title, body: popupBody(S.popup) };
      slot.innerHTML =
        '<div class="tf-overlay" data-act="close-popup">' +
          '<div class="tf-dialog ' + meta.width + '" role="dialog" aria-modal="true">' +
            '<div class="tf-dialog-head">' +
              '<h2 class="tf-dialog-title">' + esc(title) + '</h2>' +
              '<button class="tf-dialog-close" data-act="close-popup" title="' + esc(t('btn.close')) + '">' +
                svg(ICONS.X, 16) + '</button>' +
            '</div>' + popupDom.body +
          '</div>' +
        '</div>';
      return;
    }
    if (popupDom.title !== title) {
      popupDom.title = title;
      var titleEl = slot.querySelector('.tf-dialog-title');
      if (titleEl) titleEl.textContent = title;
    }
    var body = popupBody(S.popup);
    if (popupDom.body !== body) {
      popupDom.body = body;
      var dialog = slot.querySelector('.tf-dialog');
      // head 다음 몸통 루트만 교체 — 오버레이 노드는 그대로(애니메이션 유지)
      if (dialog && dialog.children.length > 1) dialog.children[1].outerHTML = body;
    }
  }

  // ── 캐릭터 팝업(FE CharsPopup 포트) ──
  function charsHtml() {
    var isUser = S.scope && S.scope.kind === 'user';
    var agents = Object.keys(S.live).map(function (k) { return S.live[k]; });
    var left;

    if (isUser) {
      var presets = I.PERSONA_PRESETS.map(function (p) {
        return '<button class="tf-preset' + (S.joinForm.persona === p.text ? ' on' : '') +
          '" data-act="preset" data-key="' + p.key + '" title="' + esc(p.text) + '">' +
          p.emoji + ' ' + esc(t('preset.' + p.key)) + '</button>';
      }).join('');
      var mine = S.mineDetail.length
        ? '<div style="margin-top:.375rem;display:flex;flex-direction:column;gap:.25rem">' +
            S.mineDetail.map(function (m) {
              return '<button class="tf-btn-ghost" data-act="look-edit" data-id="' + esc(m.id) + '">' +
                '<span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">' + esc(m.name) + '</span>' +
                '<span class="edit">' + esc(t('look.edit')) + '</span></button>';
            }).join('') + '</div>'
        : '';
      left =
        '<section class="tf-card">' +
          '<h3>' + svg(ICONS.UserPlus, 14) + ' ' + esc(t('panel.create.title')) + '</h3>' +
          '<div style="margin-top:.5rem;display:flex;flex-direction:column;gap:.375rem">' +
            '<input class="tf-input" id="tf-join-name" placeholder="' + esc(t('panel.create.name')) +
              '" value="' + esc(S.joinForm.name) + '">' +
            '<div class="tf-join-row">' +
              '<input class="tf-input age" id="tf-join-age" inputmode="numeric" placeholder="' + esc(t('panel.create.age')) +
                '" title="10~80" value="' + esc(S.joinForm.age) + '">' +
              '<select class="tf-input sm" id="tf-join-sex">' +
                '<option value="female"' + (S.joinForm.sex === 'female' ? ' selected' : '') + '>' + esc(t('panel.create.female')) + '</option>' +
                '<option value="male"' + (S.joinForm.sex === 'male' ? ' selected' : '') + '>' + esc(t('panel.create.male')) + '</option>' +
              '</select>' +
            '</div>' +
            '<input class="tf-input" id="tf-join-persona" placeholder="' + esc(t('panel.create.persona')) +
              '" value="' + esc(S.joinForm.persona) + '">' +
            '<div class="tf-presets">' + presets + '</div>' +
            '<button class="tf-btn-accent" data-act="join" ' +
              (S.busy || !S.joinForm.name.trim() ? 'disabled' : '') + '>' + esc(t('panel.create.submit')) + '</button>' +
            mine +
          '</div>' +
        '</section>';
    } else {
      left =
        '<section class="tf-card" style="font-size:12px;color:var(--ink-400)">' +
          '<h3>' + svg(ICONS.LogIn, 14) + ' ' + esc(t('panel.spectate.title')) + '</h3>' +
          '<p style="margin:.375rem 0 0;line-height:1.625">' + esc(t('panel.spectate.desc')) + '</p>' +
        '</section>';
    }

    var residents = agents.map(function (a) {
      var act = a.actType || a.status;
      return '<li><button class="tf-resident' + (S.selected === a.id ? ' on' : '') +
        '" data-act="select" data-id="' + esc(a.id) + '">' +
        '<div class="tf-resident-row">' +
          '<span class="tf-resident-name">' + esc(a.name) +
            (a.mine ? '<span class="tf-mine-badge">' + esc(t('mine.badge')) + '</span>' : '') + '</span>' +
          '<span class="tf-resident-act">' + (act ? esc(tOr('act.' + act, act)) : '') + '</span>' +
        '</div>' +
        (a.goalText ? '<div class="tf-resident-goal">' + esc(mt(a.goalText)) + '</div>' : '') +
        '</button></li>';
    }).join('');
    left +=
      '<section class="tf-card">' +
        '<h3>' + esc(t('panel.residents', { n: agents.length })) + '</h3>' +
        '<ul class="tf-residents">' +
          (agents.length ? residents : '<li style="padding:.25rem .5rem;font-size:12px;color:var(--ink-400)">' + esc(t('panel.residents.empty')) + '</li>') +
        '</ul>' +
      '</section>';

    var right = selDetailHtml();
    return '<div class="tf-chars tf-scroll" data-scroll="chars">' +
      '<div class="tf-chars-left">' + left + '</div>' +
      '<div class="tf-chars-right">' + right + '</div>' +
    '</div>';
  }

  function selDetailHtml() {
    var sel = S.selected ? S.live[S.selected] : null;
    if (!sel) {
      return '<div class="tf-select-hint">' + esc(t('panel.select.hint')) + '</div>';
    }
    var needs = HERO_NEEDS.map(function (pair) {
        var v = Number((sel.needs || {})[pair[1]] != null ? sel.needs[pair[1]] : 0);
        var pct = Math.min(100, v);
        return '<div class="tf-need">' +
          '<span class="tf-need-label">' + esc(t(pair[0])) + '</span>' +
          '<div class="tf-bar"><div class="tf-bar-fill' + (v > 70 ? ' hot' : '') +
            '" style="width:' + pct + '%"></div></div>' +
          '<span class="tf-need-val">' + Math.round(v) + '</span>' +
        '</div>';
      }).join('');

    var invEntries = Object.keys(sel.inv || {}).filter(function (k) { return (sel.inv[k] || 0) > 0; });
    var inv = invEntries.length
      ? invEntries.map(function (item) {
          return '<span class="tf-chipbox" title="' + esc(tOr('item.' + item, item)) + '">' +
            chipIconHtml(item) + '<span>' + esc(tOr('item.' + item, item)) + '</span>' +
            '<span class="tf-qty">×' + sel.inv[item] + '</span></span>';
        }).join('')
      : '<span class="tf-empty">' + esc(t('panel.inventory.empty')) + '</span>';

    var hist = S.history.length
      ? S.history.map(function (h) {
          return '<div class="tf-hist">' +
            '<span class="tf-hist-time">' + esc(I.fmtTime(h.ts)) + '</span>' +
            '<span class="tf-hist-type" style="color:' + (I.HIST_COLOR[h.type] || 'var(--ink-300)') + '">' +
              esc(tOr('hist.' + h.type, h.type)) + '</span>' +
            '<span class="tf-hist-text">' + esc(mt(h.orig || h.text)) + '</span>' +
          '</div>';
        }).join('')
      : '<div class="tf-empty">' + esc(t('panel.history.empty')) + '</div>';

    var rels = S.relations
      .filter(function (r) { return r.a_id === sel.id || r.b_id === sel.id; })
      .sort(function (a, b) { return b.warmth - a.warmth; })
      .slice(0, 6);
    var relHtml = rels.length
      ? rels.map(function (r) {
          var otherId = r.a_id === sel.id ? r.b_id : r.a_id;
          var nm = (S.live[otherId] && S.live[otherId].name) || otherId;
          return '<div class="tf-rel" title="' + esc(t('panel.relations.chats', { n: r.chats, m: r.trades })) + '">' +
            '<span class="tf-rel-name" style="color:' + I.agentColor(otherId) + '">' + esc(nm) + '</span>' +
            '<div class="tf-bar"><div class="tf-bar-fill" style="width:' + r.warmth + '%;background:hsl(' +
              Math.round(r.warmth * 1.2) + ',65%,55%)"></div></div>' +
            '<span class="tf-rel-val">' + esc(t('rel.' + I.relBucket(r.warmth))) + ' ' + r.warmth + '</span>' +
          '</div>';
        }).join('')
      : '<div class="tf-empty">' + esc(t('panel.relations.empty')) + '</div>';

    var memHtml = '';
    if (S.scope && S.scope.kind === 'user') {
      var rows = S.memories.length
        ? S.memories.map(function (m) {
            return '<div class="tf-mem">' +
              '<span class="tf-mem-kind ' + esc(m.kind) + '">' + esc(tOr('mem.' + m.kind, m.kind)) + '</span>' +
              '<span style="color:var(--ink-300)">' + esc(mt(m.orig || m.text)) + '</span>' +
            '</div>';
          }).join('')
        : '<div class="tf-empty">' + esc(t('panel.memories.empty')) + '</div>';
      memHtml =
        '<section class="tf-card">' +
          '<h3>' + svg(ICONS.MapPin, 14) + ' ' + esc(t('panel.memories')) + '</h3>' +
          '<div class="tf-scroll" data-scroll="mem" style="margin-top:.375rem;max-height:10rem;overflow-y:auto;display:flex;flex-direction:column;gap:.25rem">' +
            rows + '</div>' +
        '</section>';
    }

    return (
      '<section class="tf-card">' +
        '<h3>' + esc(sel.name) + ' — ' + esc(t('panel.status')) + '</h3>' +
        '<div style="margin-top:.375rem;display:flex;flex-direction:column;gap:.375rem">' + needs + '</div>' +
      '</section>' +
      '<section class="tf-card">' +
        '<h3>' + esc(t('panel.inventory')) + '</h3>' +
        '<div class="tf-invs">' + inv + '</div>' +
      '</section>' +
      '<section class="tf-card">' +
        '<h3>' + esc(t('panel.history')) + '</h3>' +
        '<div class="tf-scroll" data-scroll="hist" style="margin-top:.375rem;max-height:9rem;overflow-y:auto;display:flex;flex-direction:column;gap:.25rem">' + hist + '</div>' +
      '</section>' +
      '<section class="tf-card">' +
        '<h3>' + esc(t('panel.relations')) + '</h3>' +
        '<div style="margin-top:.375rem;display:flex;flex-direction:column;gap:.375rem">' + relHtml + '</div>' +
      '</section>' + memHtml
    );
  }

  // ═══════════════ 주민 히어로 카드(맵 클릭 선택 시 좌하단 컨텍스트 패널) ═══════════════
  // 상용 시뮬 관전 패턴 — 선택 즉시 신원·행동·욕구·소지를 한눈에, 주요 동작
  // (카메라 재추적·대화·상세)은 원클릭. 초상은 LPC walk 시트(64px 프레임)를
  // CSS steps로 애니메이션 — 데이터는 TF_LOOK 캐시가 재사용된다.
  var heroPortraits = {};   // id → dataURL(charSheetUrl 완료 시점 갱신)

  /** 초상 성별 — villager.rs model_path_for와 동일 판정. lookJson.sex는 첫
   *  글자 F/f=여성, 없으면 id 바이트 해시 홀수=여성(31은 홀수라 u64 래핑
   *  곱의 패리티는 바이트 패리티 합과 같다) — 3D 모델과 초상이 어긋나지
   *  않는다. */
  function heroSexOf(id, look) {
    if (look && look.sex && String(look.sex).charAt(0)) {
      return /^[Ff]/.test(String(look.sex)) ? 'female' : 'male';
    }
    var p = 0;
    for (var i = 0; i < id.length; i++) p += id.charCodeAt(i);
    return (p % 2) === 1 ? 'female' : 'male';
  }

  function heroPortrait(id) {
    var a = S.live[id];
    if (!window.TF_LOOK) return null;
    if (a) {
      var look = null;
      try { look = a.look ? JSON.parse(a.look) : null; } catch (e) { /* lookJson 파손 — 해시 초상 */ }
      var uuid = (look && look.uuid) || id;
      var sex = heroSexOf(id, look);
      var age = look && look.age != null ? Number(look.age) : 40;
      window.TF_LOOK.charSheetUrl({ uuid: uuid, sex: sex, age: age })
        .then(function (u) {
          if (heroPortraits[id] !== u) {
            heroPortraits[id] = u;
            if (S.selected === id) schedule();
          }
        })
        .catch(function () { /* 부품 PNG 로딩 실패 — 초상 없이 렌더 */ });
    }
    // 프레임 공백(재접속·드랍)에도 캐시 초상 유지 — stale 카드의 나머지와 함께
    return heroPortraits[id] || null;
  }

  var HERO_NEEDS = [['needs.hunger', 'hunger'], ['needs.thirst', 'thirst'],
    ['needs.fatigue', 'fatigue'], ['needs.warmth', 'warmth']];

  function heroNeedBar(pair, needs) {
    var v = Number(needs != null && needs[pair[1]] != null ? needs[pair[1]] : 0);
    var pct = Math.min(100, Math.max(0, Math.round(v)));
    return '<div class="tf-hero-need" title="' + esc(t(pair[0])) + '">' +
      '<span class="tf-hero-need-label">' + esc(t(pair[0])) + '</span>' +
      '<div class="tf-bar tf-hero-bar"><div class="tf-bar-fill' + (v > 70 ? ' hot' : '') +
        '" style="width:' + pct + '%"></div></div>' +
      '<span class="tf-hero-need-val' + (v > 70 ? ' hot' : '') + '">' + Math.round(v) + '</span>' +
    '</div>';
  }

  var heroCache = {};  // id → 마지막 스냅숏 — 재접속 공백에도 카드 유지(stale 표시)
  var heroSelPrev = null;  // 직전 선택 id — 변경 시 옛 캐시·초상 정리

  /** 선택 주민 해석 — 라이브 우선, 프레임 공백(재접속·드랍)은 캐시 스냅숏으로
   *  흡수(stale 표시). 선택 해제는 Rust 단일 소스(tf.onPickAgent(null))에 맡긴다. */
  function heroResolve() {
    var a = S.selected ? S.live[S.selected] : null;
    var stale = false;
    if (S.selected !== heroSelPrev) {
      // 선택 해제·전환 — 프레임 공백 흡수 임무는 끝낸 스냅숏·초상 폐기
      delete heroCache[heroSelPrev];
      delete heroPortraits[heroSelPrev];
      heroSelPrev = S.selected;
    }
    if (!a) {
      a = heroCache[S.selected || ''];
      if (!a) return null;
      stale = true; // 프레임 공백 — Rust 유예(5s) 안이면 곧 돌아온다
    } else {
      heroCache[a.id] = a;
    }
    return { a: a, stale: stale };
  }

  function heroBadgeHtml(a) {
    return a.mine
      ? '<span class="tf-hero-badge mine">' + esc(t('mine.badge')) + '</span>'
      : (a.kind === 'bot'
        ? '<span class="tf-hero-badge bot">' + esc(tOr('hero.badge.bot', '봇')) + '</span>'
        : '');
  }

  /** 소지 칩(최대 4종 + 나머지 수) — 뼈대 생성과 값 갱신이 같은 문자열을 쓴다. */
  function heroInvHtml(a) {
    var invKeys = Object.keys(a.inv || {}).filter(function (k) { return (a.inv[k] || 0) > 0; });
    var inv = invKeys.slice(0, 4).map(function (item) {
      return '<span class="tf-chipbox" title="' + esc(tOr('item.' + item, item)) + '">' +
        chipIconHtml(item) + '<span class="tf-qty">×' + a.inv[item] + '</span></span>';
    }).join('');
    if (invKeys.length > 4) inv += '<span class="tf-chipbox more">+' + (invKeys.length - 4) + '</span>';
    return inv;
  }

  /** 카드 뼈대 — 선택·로케일이 바뀔 때에만 다시 쓴다(입장 애니메이션은 그때만).
   *  틱마다 달라지는 값(초상·욕구·소지·행동)은 heroApply가 제자리 갱신 —
   *  노드가 유지돼야 걷기 스프라이트·입장 애니메이션이 매 틱 재시작되지
   *  않는다(깜빡임의 원인). */
  function heroHtml(a, stale, portrait) {
    var inv = heroInvHtml(a);
    var badge = heroBadgeHtml(a);
    var act = a.actType || a.status || '';
    return '<aside class="tf-hero' + (stale ? ' stale' : '') + '" role="region" aria-label="' + esc(tOr('hero.card', '주민 카드')) + '">' +
      '<div class="tf-hero-frame">' +
        '<div class="tf-hero-ground"></div>' +
        (portrait
          ? '<div class="tf-hero-sprite" style="background-image:url(' + portrait + ')"></div>'
          : '<div class="tf-hero-sprite empty">' + svg(ICONS.User, 28) + '</div>') +
      '</div>' +
      '<div class="tf-hero-main">' +
        '<header class="tf-hero-head">' +
          '<span class="tf-hero-name" style="color:' + I.agentColor(a.id) + '">' + esc(a.name) + '</span>' +
          '<span class="tf-hero-badge-slot"' + (badge ? '' : ' style="display:none"') + '>' + badge + '</span>' +
          '<button class="tf-hero-x" data-act="hero-close" title="' + esc(t('btn.close')) + '" aria-label="' + esc(t('btn.close')) + '">' + svg(ICONS.X, 14) + '</button>' +
        '</header>' +
        '<div class="tf-hero-act">' +
          '<span class="tf-hero-act-kind"' + (act ? '' : ' style="display:none"') + '>' +
            (act ? esc(tOr('act.' + act, act)) : '') + '</span>' +
          '<span class="tf-hero-goal"' + (a.goalText ? '' : ' style="display:none"') + '>' +
            (a.goalText ? esc(mt(a.goalText)) : '') + '</span>' +
        '</div>' +
        '<div class="tf-hero-needs">' +
          HERO_NEEDS.map(function (p) { return heroNeedBar(p, a.needs); }).join('') +
        '</div>' +
        '<div class="tf-hero-inv' + (inv ? '' : ' empty') + '">' +
          (inv || '<span class="tf-empty">' + esc(t('panel.inventory.empty')) + '</span>') +
        '</div>' +
        '<div class="tf-hero-btns">' +
          '<button class="tf-hero-btn" data-act="hero-cam" title="' + esc(tOr('hero.cam.hint', '카메라가 이 주민을 따라갑니다')) + '">' +
            svg(ICONS.MapPin, 14) + '<span>' + esc(tOr('hero.cam', '카메라')) + '</span></button>' +
          '<button class="tf-hero-btn" data-act="hero-chat">' +
            svg(ICONS.Users, 14) + '<span>' + esc(tOr('hero.chat', '대화')) + '</span></button>' +
          '<button class="tf-hero-btn" data-act="hero-detail">' +
            svg(ICONS.ScrollText, 14) + '<span>' + esc(tOr('hero.detail', '상세')) + '</span></button>' +
        '</div>' +
      '</div>' +
    '</aside>';
  }

  // 카드 DOM 참조 — 뼈대 재구축(선택·로케일 변경) 시 다시 잡는다
  var heroDom = null;
  function heroUpdate() {
    var r = heroResolve();
    var slot = slots.hero;
    if (!r) {
      if (heroDom) { heroDom = null; slot.innerHTML = ''; }
      return;
    }
    var a = r.a;
    var portrait = heroPortrait(a.id);
    if (!heroDom || heroDom.id !== a.id || heroDom.locale !== I.getLocale() || !slot.contains(heroDom.el)) {
      slot.innerHTML = heroHtml(a, r.stale, portrait);
      var el = slot.firstElementChild;
      heroDom = {
        id: a.id, locale: I.getLocale(), el: el,
        sprite: el.querySelector('.tf-hero-sprite'),
        name: el.querySelector('.tf-hero-name'),
        badge: el.querySelector('.tf-hero-badge-slot'),
        actKind: el.querySelector('.tf-hero-act-kind'),
        goal: el.querySelector('.tf-hero-goal'),
        needs: Array.prototype.map.call(el.querySelectorAll('.tf-hero-need'), function (row) {
          return { fill: row.querySelector('.tf-bar-fill'), val: row.querySelector('.tf-hero-need-val') };
        }),
        inv: el.querySelector('.tf-hero-inv'),
        nameText: null, badgeHtml: null, actText: null, goalShown: null, invHtml: null, portrait: null,
      };
    }
    heroApply(a, r.stale, portrait);
  }

  /** 값 갱신 — 노드를 유지한 채 고치므로 입장·걷기 애니메이션이 이어지고
   *  욕구 바 transition도 자연스럽게 흐른다. */
  function heroApply(a, stale, portrait) {
    var d = heroDom;
    if (!d) return;
    d.el.classList.toggle('stale', stale);
    if (d.portrait !== (portrait || '')) {
      d.portrait = portrait || '';
      if (portrait) {
        d.sprite.classList.remove('empty');
        d.sprite.style.backgroundImage = 'url(' + portrait + ')';
        d.sprite.innerHTML = '';
      } else {
        d.sprite.classList.add('empty');
        d.sprite.style.backgroundImage = '';
        d.sprite.innerHTML = svg(ICONS.User, 28);
      }
    }
    if (d.nameText !== a.name) {
      d.nameText = a.name;
      d.name.textContent = a.name;
      d.name.style.color = I.agentColor(a.id);
    }
    var badge = heroBadgeHtml(a);
    if (d.badgeHtml !== badge) {
      d.badgeHtml = badge;
      d.badge.innerHTML = badge;
      d.badge.style.display = badge ? '' : 'none';
    }
    var act = a.actType || a.status || '';
    if (d.actText !== act) {
      d.actText = act;
      d.actKind.textContent = act ? tOr('act.' + act, act) : '';
      d.actKind.style.display = act ? '' : 'none';
    }
    var goal = a.goalText || '';
    // 원문이 아니라 '화면에 띄울 문장'(mt 캐시 반영)으로 가드 — 번역 완료
    // schedule에서도 교체돼야 한다(원문 가드면 번역이 영원히 안 뜬다).
    var goalShown = goal ? mt(goal) : '';
    if (d.goalShown !== goalShown) {
      d.goalShown = goalShown;
      d.goal.textContent = goalShown;
      d.goal.style.display = goal ? '' : 'none';
    }
    HERO_NEEDS.forEach(function (pair, i) {
      var row = d.needs[i];
      if (!row) return;
      var v = Number(a.needs != null && a.needs[pair[1]] != null ? a.needs[pair[1]] : 0);
      var hot = v > 70;
      var pct = Math.min(100, Math.max(0, Math.round(v)));
      row.fill.style.width = pct + '%';
      row.fill.classList.toggle('hot', hot);
      row.val.textContent = Math.round(v);
      row.val.classList.toggle('hot', hot);
    });
    var inv = heroInvHtml(a);
    if (d.invHtml !== inv) {
      d.invHtml = inv;
      d.inv.innerHTML = inv || '<span class="tf-empty">' + esc(t('panel.inventory.empty')) + '</span>';
      d.inv.classList.toggle('empty', !inv);
    }
  }

  // ── 자원 팝업(FE popups res 포트) ──
  function resHtml() {
    var relic = '';
    if (S.selRelic) {
      relic =
        '<section class="tf-card amber">' +
          '<div style="display:flex;align-items:flex-start;justify-content:space-between;gap:.5rem">' +
            '<h3 class="tf-h-amber"><span>⚒</span> ' + esc(S.selRelic.name) +
              (S.selRelic.basedOn ? ' <span class="tf-improved">' + esc(t('relic.card.improved', { x: S.selRelic.basedOn })) + '</span>' : '') +
            '</h3>' +
            '<button class="tf-close-x" data-act="close-relic" aria-label="' + esc(t('btn.close')) + '">✕</button>' +
          '</div>' +
          '<div style="margin-top:.125rem;font-size:10px;color:var(--ink-400)">' +
            esc(t('relic.card.by')) + ' · ' + esc(S.selRelic.by || '—') + '</div>' +
          '<p style="margin:.375rem 0 0;white-space:pre-wrap;font-size:11px;line-height:1.625;color:var(--ink-200)">' +
            (S.selRelic.purpose ? esc(mt(S.selRelic.purpose)) : esc(t('relic.card.desc.empty'))) + '</p>' +
        '</section>';
    }
    var stats = resStats();
    var statsHtml = stats.length
      ? stats.map(function (k) {
          var species = Object.keys(k.species).sort(function (a, b) { return k.species[b].n - k.species[a].n; })
            .map(function (sp) {
              var s = k.species[sp];
              return '<span class="tf-chipbox" title="' + esc(t('res.usable', { n: s.usable })) + '">' +
                esc(tOr('sp.' + sp, sp)) + ' <span class="tf-qty">×' + s.n + '</span></span>';
            }).join('');
          return '<section class="tf-card">' +
            '<div style="display:flex;align-items:center;justify-content:space-between">' +
              '<h3 style="color:var(--ink-200)"><span>' + (I.RES_KIND_ICON[k.kind] || '⛏') + '</span> ' +
                esc(tOr('res.kind.' + k.kind, k.kind)) + '</h3>' +
              '<span style="font-size:10px;color:var(--ink-400)">' +
                esc(t('res.usable', { n: k.usable })) + ' · ' + k.total + '</span>' +
            '</div>' +
            '<div class="tf-species">' + species + '</div>' +
          '</section>';
        }).join('')
      : '<div style="padding:.5rem 0;text-align:center;font-size:12px;color:var(--ink-500)">' + esc(t('res.empty')) + '</div>';

    var invNames = Object.keys(S.inventions);
    var invHtml = invNames.length
      ? invNames.map(function (name) {
          var inv = S.inventions[name];
          return '<span class="tf-chipbox" title="' + esc(inv.inventor ? inv.inventor + ' — ' + mt(inv.purpose || '') : (mt(inv.purpose || '') || '')) + '">' +
            chipIconHtml(name) + esc(name) +
            (inv.based_on ? '<span class="tf-amber-up" title="' + esc(t('relic.card.improved', { x: inv.based_on })) + '">↑</span>' : '') +
            '</span>';
        }).join('')
      : '<div class="tf-empty">' + esc(t('res.inventions.empty')) + '</div>';

    return '<div class="tf-res-body tf-scroll" data-scroll="res">' + relic + statsHtml +
      '<section class="tf-card">' +
        '<h3 style="color:var(--ink-200)">' + esc(t('res.inventions')) + '</h3>' +
        '<div class="tf-species">' + invHtml + '</div>' +
      '</section></div>';
  }

  // ── 문명 팝업(FE CivPopup 포트) ──
  function civHtml() {
    var all = S.inventionsAll;
    var score = civScore(all);
    var era = eraOf(score);
    var stats = civGroupStats(all);
    var sel = S.selInv ? all.filter(function (i) { return i.name === S.selInv; })[0] : null;

    // 만든 이들 — 발명가별 발명 수(존중 표시층 근사, §17.60)
    var makersMap = {};
    all.forEach(function (i) {
      var w = String(i.inventor || '').trim();
      if (w && w !== 'the ancestors') makersMap[w] = (makersMap[w] || 0) + 1;
    });
    var makers = Object.keys(makersMap).sort(function (a, b) { return makersMap[b] - makersMap[a]; }).slice(0, 4);

    var summary =
      '<section class="tf-card amber">' +
        '<div class="tf-civ-summary-head">' +
          '<h3 class="tf-h-amber">🏛 ' + esc(t('menu.civilization')) + '</h3>' +
          '<span style="font-size:11px;color:var(--ink-300)">' + esc(tOr('era.' + era, era)) + '</span>' +
          '<span style="margin-left:auto;font-size:11px;color:var(--ink-400)">' +
            esc(t('civ.total', { n: all.length })) + ' · ' + esc(t('civ.score', { n: score })) + '</span>' +
        '</div>' +
        '<div class="tf-groups">' +
          stats.map(function (s) {
            var dots = '';
            for (var i = 0; i < 8; i++) dots += '<span class="tf-dot' + (i < Math.min(s.total, 8) ? ' on' : '') + '"></span>';
            return '<div class="tf-group">' +
              '<div class="tf-group-head">' +
                '<span class="tf-group-name">' + esc(tOr('invgroup.' + s.group, s.group)) + '</span>' +
                '<span class="tf-group-n' + (s.total > 0 ? '' : ' zero') + '">' + s.total + '</span>' +
              '</div>' +
              '<div class="tf-dots">' + dots + '</div>' +
            '</div>';
          }).join('') +
        '</div>' +
        (makers.length
          ? '<div class="tf-makers"><span style="color:var(--ink-400)">⚒ ' + esc(t('civ.makers')) + ' ·</span>' +
              makers.map(function (m) {
                return '<span class="tf-maker">' + esc(m) + ' <span style="color:var(--ink-400)">' + makersMap[m] + '</span></span>';
              }).join('') + '</div>'
          : '') +
      '</section>';

    // 살아있는 전통(§17.60) — 문화 계열 발명의 거행 통계
    var living = all.filter(function (i) { return invGroupOf(i.category) === 'culture'; });
    var livingHtml = '';
    if (living.length) {
      var held = living.filter(function (i) { return (i.performs || 0) > 0; });
      livingHtml =
        '<section class="tf-card"><h4>🎏 ' + esc(t('civ.living')) + '</h4>' +
        (held.length === 0
          ? '<div class="tf-empty" style="margin-top:.25rem">' + esc(t('civ.living.empty')) + '</div>'
          : '<div class="tf-species">' + held.map(function (i) {
              var n = i.performs || 0;
              var depth = Math.min(3, Math.floor(n / 4));
              return '<span class="tf-chipbox"' +
                (i.last_perform_day != null ? ' title="' + esc(t('civ.living.last', { d: i.last_perform_day })) + '"' : '') + '>' +
                '<span style="font-size:14px;line-height:1">' + (i.icon && i.icon.tpl ? chipIconHtml(i.name, 20) : invCatEmoji(i.category)) + '</span>' +
                esc(i.name) +
                '<span class="tf-qty" style="color:#fcd34d">' + esc(t('civ.living.holds', { n: n, d: depth })) + '</span>' +
                '</span>';
            }).join('') + '</div>') +
        '</section>';
    }

    // 마을 연대기 — kind='chronicle' 최근 12행(최신 우선)
    var chron = S.events.filter(function (e) { return e.kind === 'chronicle'; })
      .map(function (e) { return I.eventText(e.payload || {}, I.getLocale()); })
      .filter(Boolean).slice(-12).reverse();
    var chronHtml =
      '<section class="tf-card"><h4>📜 ' + esc(t('civ.story')) + '</h4>' +
      (chron.length === 0
        ? '<div class="tf-empty" style="margin-top:.25rem">' + esc(t('civ.story.empty')) + '</div>'
        : '<ol style="margin:.375rem 0 0;padding-left:1.25rem;display:flex;flex-direction:column;gap:.25rem;font-size:11px;line-height:1.625;color:var(--ink-300)">' +
            chron.map(function (r) { return '<li style="color:rgba(254,243,199,.9)">' + esc(r) + '</li>'; }).join('') + '</ol>') +
      '</section>';

    var listHtml;
    if (all.length === 0) {
      listHtml = '<div style="padding:1.5rem 0;text-align:center;font-size:12px;color:var(--ink-500)">' +
        esc(t('res.inventions.empty')) + '</div>';
    } else if (sel) {
      listHtml = invDetailHtml(sel);
    } else {
      listHtml = stats.filter(function (s) { return s.total > 0; }).map(function (s) {
        var rows = [];
        Object.keys(s.cats).forEach(function (cat) { rows = rows.concat(s.cats[cat]); });
        rows.sort(function (a, b) { return (b.day || 0) - (a.day || 0); });
        return '<section class="tf-card">' +
          '<h4>' + esc(tOr('invgroup.' + s.group, s.group)) +
            ' <span style="margin-left:.375rem;font-weight:400;color:var(--ink-500)">(' + s.total + ')</span></h4>' +
          '<div class="tf-invrows">' + rows.map(function (i) { return invRowHtml(i); }).join('') + '</div>' +
        '</section>';
      }).join('');
    }

    return '<div class="tf-civ tf-scroll" data-scroll="civ">' + summary + livingHtml + chronHtml + listHtml + '</div>';
  }

  function invIconHtml(i, px) {
    if (i.icon && i.icon.tpl) return chipIconHtml(i.name, px);
    return '<span style="font-size:' + (px >= 24 ? 24 : 16) + 'px;line-height:1">' +
      (I.INV_SEED_ICON[i.name] || invCatEmoji(i.category)) + '</span>';
  }
  function invRowHtml(i) {
    var year = I.dateOf(i.day || 0).year;
    var cat = invCatId(i.category);
    return '<button class="tf-invrow" data-act="sel-inv" data-name="' + esc(i.name) + '">' +
      '<span class="tf-invrow-icon">' + invIconHtml(i, 22) + '</span>' +
      '<span class="tf-invrow-main">' +
        '<span class="tf-invrow-line1">' +
          '<span class="tf-invrow-name">' + esc(i.name) + '</span>' +
          (i.based_on ? '<span class="tf-amber-up" title="' + esc(t('relic.card.improved', { x: i.based_on })) + '">↑</span>' : '') +
          '<span class="tf-invrow-year">' + esc(t('civ.inv.year', { y: year })) + '</span>' +
        '</span>' +
        '<span class="tf-invrow-line2">' + invCatEmoji(i.category) + ' ' + esc(tOr('invcat.' + cat, cat || '—')) +
          ' · ' + esc(t('relic.card.by')) + ' ' + esc(i.inventor || '—') + '</span>' +
        (i.purpose ? '<span class="tf-invrow-desc">' + esc(mt(i.purpose)) + '</span>' : '') +
      '</span>' +
    '</button>';
  }
  function invDetailHtml(sel) {
    var cal = I.dateOf(sel.day || 0);
    var mat = sel.materials
      ? Object.keys(sel.materials).map(function (it) { return it + '×' + sel.materials[it]; }).join(' · ')
      : '';
    var cat = invCatId(sel.category);
    return '<section class="tf-card amber tf-invdetail">' +
      '<div class="tf-invdetail-head">' +
        '<div class="tf-invdetail-icon">' + invIconHtml(sel, 32) + '</div>' +
        '<div style="min-width:0;flex:1 1 0%">' +
          '<div style="display:flex;align-items:center;gap:.375rem">' +
            '<h3 class="tf-invdetail-name">' + esc(sel.name) + '</h3>' +
            (sel.based_on ? '<span class="tf-improved">' + esc(t('relic.card.improved', { x: sel.based_on })) + '</span>' : '') +
          '</div>' +
          '<div class="tf-invdetail-meta">' +
            '<span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">' + esc(t('relic.card.by')) + ' · ' + esc(sel.inventor || '—') + '</span>' +
            '<span class="tf-invdetail-date">' + esc(t('civ.inv.date', { y: cal.year, m: cal.month, d: cal.dayOfMonth })) + '</span>' +
            '<span class="tf-invdetail-cat">' + esc(tOr('invcat.' + cat, cat || '—')) + '</span>' +
          '</div>' +
        '</div>' +
        '<button class="tf-close-x" data-act="close-inv" aria-label="' + esc(t('ui.back')) + '">✕</button>' +
      '</div>' +
      (mat ? '<div class="tf-invdetail-mat"><span style="color:var(--ink-400)">' + esc(t('inv.materials')) + ' · </span>' + esc(mat) + '</div>' : '') +
      '<p class="tf-invdetail-purpose">' +
        (sel.purpose ? esc(mt(sel.purpose)) : esc(t('relic.card.desc.empty'))) + '</p>' +
    '</section>';
  }

  // ── 기록 팝업(FE LogFeed 포트) ──
  function logHtml() {
    var rows = S.events.map(function (e) {
      var h = I.highlightOf(e, {
        locale: I.getLocale(),
        tOr: tOr,
        nameOf: function (id) { return (S.live[id] && S.live[id].name) || ''; },
      });
      var p = e.payload || {};
      var text = (h && h.text) || I.eventText(p, I.getLocale()) || String(p.goal != null ? p.goal : '');
      var who = (h && h.who) || (e.agent_id ? ((S.live[e.agent_id] && S.live[e.agent_id].name) || '') : '');
      var icon = (h && h.icon) || I.EV_KIND_ICON[e.kind] || '•';
      return { e: e, h: h, text: text, who: who, icon: icon, cat: I.EV_CAT_OF[e.kind] || 'world' };
    }).filter(function (r) { return r.text; }).reverse();
    function inCat(r, c) {
      if (c === 'all') return true;
      if (c === 'highlight') return r.h !== null;
      return r.cat === c;
    }
    var tabs = [{ key: 'all', labelKey: 'panel.log.tab.all' }, { key: 'highlight', labelKey: 'panel.log.tab.highlight' }]
      .concat(I.EV_CATS.map(function (c) { return { key: c, labelKey: 'panel.log.tab.' + c }; }))
      .map(function (tb) {
        var n = rows.filter(function (r) { return inCat(r, tb.key); }).length;
        return '<button class="tf-logtab' + (S.logCat === tb.key ? ' on' : '') + '" data-act="logcat" data-key="' + tb.key + '">' +
          esc(t(tb.labelKey)) + ' <span class="n">' + n + '</span></button>';
      }).join('');
    var shown = rows.filter(function (r) { return inCat(r, S.logCat); });
    var feed =
      '<section class="tf-card" style="display:flex;min-height:180px;flex-direction:column">' +
        '<h3 style="flex-shrink:0">' + esc(t('panel.log')) + '</h3>' +
        '<div class="tf-logtabs">' + tabs + '</div>' +
        '<div class="tf-logrows tf-scroll" data-scroll="log">' +
          (shown.length
            ? shown.map(function (r) {
                return '<div class="tf-logrow">' +
                  '<span class="tf-logrow-icon">' + r.icon + '</span>' +
                  '<span class="tf-logrow-text" style="color:' + (I.EVENT_COLOR[r.e.kind] || 'var(--ink-300)') + '">' +
                    (r.who ? '<span class="tf-logrow-who">' + esc(r.who) + ' — </span>' : '') +
                    esc(r.text) +
                    (r.e.ts ? '<span class="tf-logrow-ts">' + esc(I.fmtTime(r.e.ts)) + '</span>' : '') +
                  '</span></div>';
              }).join('')
            : '<div class="tf-empty">' + esc(t('panel.history.empty')) + '</div>') +
        '</div>' +
      '</section>';

    var chat =
      '<section class="tf-card" style="display:flex;min-height:180px;flex-direction:column">' +
        '<h3 style="flex-shrink:0">' + esc(t('panel.chat')) + '</h3>' +
        '<div class="tf-chatlog tf-scroll" data-scroll="chatlog">' +
          (S.chat.length
            ? S.chat.map(function (c) {
                return '<div class="tf-chatrow"><span class="who' + (isHumanRow(c) ? ' human' : '') + '"' +
                  (isHumanRow(c) ? '' : ' style="color:' + I.agentColor(c.agent_id) + '"') + '>' +
                  esc(c.speaker) + ':</span> <span style="color:var(--ink-300)">' + esc(chatDisplayText(c)) + '</span></div>';
              }).join('')
            : '<div class="tf-empty">' + esc(t('chat.log.empty')) + '</div>') +
        '</div>' +
      '</section>';
    return '<div class="tf-log tf-scroll" data-scroll="logpage">' + feed + chat + '</div>';
  }

  // ── 계정 팝업 ──
  function accountHtml() {
    var body;
    if (hasToken()) {
      body = '<button class="tf-logout" data-act="logout">' + svg(ICONS.LogOut, 14) + ' ' + esc(t('menu.logout')) + '</button>';
    } else {
      body =
        '<div style="display:flex;flex-direction:column;gap:.25rem">' +
          '<a class="tf-oauth-google" href="/auth/google?next=%2F">' +
            '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">' +
              '<path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 0 1-2.2 3.32v2.77h3.57c2.08-1.92 3.27-4.74 3.27-8.1Z"/>' +
              '<path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84A11 11 0 0 0 12 23Z"/>' +
              '<path fill="#FBBC05" d="M5.84 14.1a6.6 6.6 0 0 1 0-4.2V7.06H2.18a11 11 0 0 0 0 9.88l3.66-2.84Z"/>' +
              '<path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15A11 11 0 0 0 2.18 7.06l3.66 2.84c.87-2.6 3.3-4.52 6.16-4.52Z"/>' +
            '</svg>' + esc(t('menu.google')) + '</a>' +
          '<a class="tf-oauth-kakao" href="/auth/kakao?next=%2F">' + esc(t('menu.kakao')) + '</a>' +
        '</div>';
    }
    return '<div class="tf-res-body tf-scroll"><section class="tf-card">' +
      '<h3>' + esc(t('set.account')) + '</h3>' +
      '<div style="margin-top:.375rem">' + body + '</div>' +
    '</section></div>';
  }

  // ── 설정 팝업 ──
  function setHtml() {
    var locale = I.getLocale();
    var langs = I.LOCALES.map(function (l) {
      return '<button class="tf-lang-btn' + (locale === l.code ? ' on' : '') + '" data-act="locale" data-key="' + l.code + '">' +
        l.flag + ' ' + esc(l.label) + '</button>';
    }).join('');
    return '<div class="tf-res-body tf-scroll">' +
      '<section class="tf-card">' +
        '<h3>' + esc(t('lang.select')) + '</h3>' +
        '<div class="tf-lang-grid">' + langs + '</div>' +
      '</section>' +
      '<section class="tf-card">' +
        '<h3>' + esc(t('set.credits')) + '</h3>' +
        '<button class="tf-credits-btn" data-act="credits">' + svg(ICONS.ScrollText, 14) + ' Terrafolk — ' + esc(t('world.tagline')) + '</button>' +
      '</section>' +
    '</div>';
  }

  // ── 크레딧 모달(FE CreditsPage 포트) ──
  function creditsHtml() {
    var authors = Object.keys(DATA.CREDITS.authors).map(function (name) {
      var c = DATA.CREDITS.authors[name];
      return '<div class="tf-credit-card"><div class="nm">' + esc(name) + '</div>' +
        '<div class="lc">' + esc(c.licenses.join(' · ')) + '</div></div>';
    }).join('');
    var tools = {};
    Object.keys(DATA.CREDITS.tools).forEach(function (k) { tools[k] = DATA.CREDITS.tools[k]; });
    Object.keys(DATA.CREDITS.spears).forEach(function (k) { tools[k] = DATA.CREDITS.spears[k]; });
    var toolHtml = Object.keys(tools).map(function (name) {
      var c = tools[name];
      return '<div class="tf-credit-card"><div class="nm">' + esc(name) + '</div>' +
        '<div class="lc">' + esc(c.licenses.join(' · ')) + ' — ' + esc(c.items.join(', ')) + '</div></div>';
    }).join('');
    return '<div class="tf-overlay" data-act="close-credits">' +
      '<div class="tf-dialog max-w-3xl">' +
        '<div class="tf-dialog-head">' +
          '<h2 class="tf-dialog-title">' + esc(t('set.credits')) + '</h2>' +
          '<button class="tf-dialog-close" data-act="close-credits">' + svg(ICONS.X, 16) + '</button>' +
        '</div>' +
        '<div class="tf-credits tf-scroll" data-scroll="credits">' +
          '<section class="tf-card" style="padding:1rem">' +
            '<h2>' + esc(t('credits.tiles.h')) + '</h2>' +
            '<p>' + esc(t('credits.tiles.body1')) + ' <a href="https://kenney.nl" target="_blank" rel="noreferrer">Kenney</a> ' + esc(t('credits.tiles.body2')) + '</p>' +
          '</section>' +
          '<section class="tf-card" style="padding:1rem">' +
            '<h2>' + esc(t('credits.icons.h')) + '</h2>' +
            '<p>' + esc(t('credits.icons.body1')) + ' <a href="https://game-icons.net" target="_blank" rel="noreferrer">game-icons.net</a> ' +
            esc(t('credits.icons.body2')) + ' <strong>Lorc</strong>(stone-spear, stone-axe, curvy-knife, flat-hammer, needle-drill, harpoon-trident, mantrap, spears), ' +
            '<strong>Delapouite</strong>(bow-arrow, hand-saw, sling, fishing-pole, basket, cooking-pot, plain-arrow).</p>' +
          '</section>' +
          '<section class="tf-card" style="padding:1rem">' +
            '<h2>' + esc(t('credits.sprites.h')) + '</h2>' +
            '<p style="margin-bottom:.5rem">' + esc(t('credits.sprites.body1')) +
            ' <a href="https://github.com/LiberatedPixelCup/Universal-LPC-Spritesheet-Character-Generator" target="_blank" rel="noreferrer">Universal LPC Spritesheet Character Generator</a> ' +
            esc(t('credits.sprites.body2')) + ' ' + esc(t('credits.sprites.body3')) + '</p>' +
            '<div class="tf-credits-grid">' + authors + '</div>' +
            '<h3>' + esc(t('credits.tools.h')) + ' (docs §17.39)</h3>' +
            '<p style="margin-bottom:.5rem">' + esc(t('credits.tools.body')) + '</p>' +
            '<div class="tf-credits-grid">' + toolHtml + '</div>' +
            '<p style="margin-top:.75rem;font-size:12px;color:var(--ink-400)">' + esc(t('credits.docs')) +
            ' <a href="https://github.com/LiberatedPixelCup/Universal-LPC-Spritesheet-Character-Generator/blob/master/README.md" target="_blank" rel="noreferrer">' +
            esc(t('credits.repo')) + '</a></p>' +
          '</section>' +
        '</div>' +
      '</div>' +
    '</div>';
  }

  // ── 외형 편집 모달(FE LookEditor 포트) ──
  function lookHtml() {
    var E = S.lookEdit;
    var tab = E.tab;
    var rows;
    if (tab === 'clothes') {
      rows = ['top', 'bottom', 'feet'].map(function (slot) {
        var label = t(slot === 'top' ? 'look.top' : slot === 'bottom' ? 'look.bottom' : 'look.feet');
        var options = E.wearOptions(slot);
        var value = E.wears[slot] || 'auto';
        return optRowHtml(label, options, value, E);
      }).join('');
    } else if (tab === 'hair') {
      rows = optRowHtml(t('look.hair'), E.opts.hairs.map(function (v) { return { key: v, ko: E.prettyPart(v) }; }), E.parts.hair || E.opts.hairs[0], E) +
        optRowHtml(t('look.hairColor'), E.opts.hairColors.map(function (v) { return { key: v, ko: E.prettyPart(v) }; }), E.parts.hairColor || E.opts.hairColors[0], E);
    } else {
      rows = optRowHtml(t('look.skin'), E.opts.skins.map(function (v) { return { key: v, ko: E.prettyPart(v) }; }), E.parts.skin || E.opts.skins[0], E) +
        optRowHtml(t('look.eye'), E.opts.eyes.map(function (v) { return { key: v, ko: E.prettyPart(v) }; }), E.parts.eye || E.opts.eyes[0], E) +
        optRowHtml(t('look.beard'), E.opts.beards.map(function (v) { return { key: v, ko: E.prettyPart(v) }; }), E.parts.beard || 'none', E) +
        optRowHtml(t('look.glasses'), E.opts.glasses.map(function (v) { return { key: v, ko: E.prettyPart(v) }; }), E.parts.glasses || 'none', E);
    }
    var tabs = [['clothes', 'look.tab.clothes'], ['hair', 'look.tab.hair'], ['face', 'look.tab.face']].map(function (tb) {
      return '<button class="tf-look-tab' + (tab === tb[0] ? ' on' : '') + '" data-act="look-tab" data-key="' + tb[0] + '">' + esc(t(tb[1])) + '</button>';
    }).join('');
    return '<div class="tf-overlay" data-act="close-look">' +
      '<div class="tf-look tf-scroll" data-scroll="look">' +
        '<div class="tf-look-head">' +
          '<h3 class="tf-look-title">' + esc(t('look.title')) + ' — ' + esc(E.name) + '</h3>' +
          '<button class="tf-dialog-close" data-act="close-look">' + svg(ICONS.X, 16) + '</button>' +
        '</div>' +
        '<div class="tf-look-body">' +
          frameHtml(E, 136, false) +
          '<div class="tf-look-main">' +
            '<div class="tf-look-tabs">' + tabs + '</div>' +
            '<div class="tf-look-rows">' + rows + '</div>' +
          '</div>' +
        '</div>' +
        '<div class="tf-look-actions">' +
          '<button class="tf-btn-accent" data-act="look-save" ' + (E.busy ? 'disabled' : '') + '>' + esc(t('look.save')) + '</button>' +
          '<button class="tf-btn-outline" data-act="close-look">' + esc(t('look.cancel')) + '</button>' +
        '</div>' +
        (tab === 'clothes'
          ? '<p class="tf-look-note">' + svg(ICONS.Lock, 12) + ' ' + esc(t('look.auto')) + ' · 🔒 ' + esc(t('look.locked')) + '</p>'
          : '') +
      '</div>' +
    '</div>';
  }

  /** 시트 프레임 크롭 박스(FE Frame 포트) — walk 시트 down행(2행) 0프레임. */
  function frameHtml(E, size, dim) {
    var s = size / 64;
    return '<div class="tf-frame" style="width:' + size + 'px;height:' + size + 'px">' +
      (E.frameUrl
        ? '<img src="' + E.frameUrl + '" alt="" class="' + (dim ? 'dim' : '') +
          '" style="top:' + (-64 * 2 * s) + 'px;width:' + (576 * s) + 'px;height:' + (256 * s) + 'px">'
        : '<div style="display:flex;height:100%;align-items:center;justify-content:center;font-size:10px;color:var(--ink-500)">…</div>') +
      '</div>';
  }
  function optRowHtml(label, options, value, E) {
    var idx = Math.max(0, options.findIndex(function (o) { return o.key === value; }));
    var cur = options[idx] || options[0] || { key: 'none', ko: '' };
    return '<div class="tf-optrow">' +
      '<span class="tf-optrow-label">' + esc(label) + '</span>' +
      '<button class="tf-optrow-nav" data-act="look-slide" data-dir="-1">' + svg(ICONS.ChevronLeft, 16) + '</button>' +
      '<div class="tf-optrow-main">' +
        frameHtml(E, 44, !!cur.locked) +
        '<div class="tf-optrow-name">' +
          '<div class="tf-optrow-nm' + (cur.locked ? ' locked' : '') + '">' +
            (cur.key === 'none' ? esc(t('look.none')) : esc(cur.ko)) + '</div>' +
          '<div class="tf-optrow-sub">' + (cur.locked ? '🔒 ' + esc(t('look.locked')) : (idx + 1) + '/' + options.length) + '</div>' +
        '</div>' +
      '</div>' +
      '<button class="tf-optrow-nav" data-act="look-slide" data-dir="1">' + svg(ICONS.ChevronRight, 16) + '</button>' +
    '</div>';
  }

  /** 외형 편집기 상태 — 옷(소지 기반 잠금)·헤어·얼굴 슬라이드, 저장은 PATCH look. */
  function makeLookEditor(m) {
    var opts = window.TF_LOOK.lookPartOptions(m.sex, m.age);
    var base = window.TF_LOOK.buildLook(m.id, m.sex, m.age).picks;
    var E = {
      agentId: m.id, name: m.name, sex: m.sex || 'female', age: m.age || 30,
      opts: opts,
      inv: m.inventory || {}, invClothes: m.clothingInventions || [],
      tab: 'clothes',
      parts: Object.assign({}, base, m.parts || {}),
      wears: Object.assign({}, m.wears || {}),
      frameUrl: '',
      busy: false,
      prettyPart: function (v) {
        var s = (String(v).split('_').pop() || v).replace(/([a-z])([A-Z])/g, '$1 $2');
        return s.charAt(0).toUpperCase() + s.slice(1);
      },
      setPart: function (k, v) { E.parts[k] = v; E.refreshFrame(); schedule(); },
      pickWear: function (slot, key) {
        if (key === 'auto') delete E.wears[slot];
        else E.wears[slot] = key;
        E.refreshFrame();
        schedule();
      },
      // 옷 카탈로그(FE CLOTHES 미러) — 소지 기반 잠금
      CLOTHES: [
        { item: 'grass_skirt', slot: 'bottom', cf: 0.72, look: { bottom: 'legs_skirts_plain', bottomColor: 'forest' } },
        { item: 'leather_wrap', slot: 'top', cf: 0.65, look: { top: 'torso_clothes_sleeveless2', topColor: 'leather' } },
        { item: 'woven_leather', slot: 'top', cf: 0.5, look: { top: 'torso_clothes_longsleeve2', topColor: 'walnut' } },
        { item: 'stitched_leather', slot: 'top', cf: 0.35, look: { top: 'torso_clothes_longsleeve2_cardigan', topColor: 'brown' } },
        { item: 'leather_shoes', slot: 'feet', cf: 0.9, look: { feet: 'feet_boots_basic', feetColor: 'leather' } },
      ],
      INVENTED_LOOK: function (color) { return { top: 'torso_clothes_longsleeve', topColor: color }; },
      effectiveWearPatch: function () {
        var patch = {};
        var owned = function (i) { return (E.inv[i] || 0) > 0; };
        ['top', 'bottom', 'feet'].forEach(function (slot) {
          var cands = E.CLOTHES.filter(function (c) { return c.slot === slot && owned(c.item); }).concat(
            E.invClothes.filter(function (ic) { return owned(ic.name); })
              .map(function (ic) { return { item: ic.name, cf: ic.cf, look: E.INVENTED_LOOK(ic.color) }; })
          );
          var pick = E.wears[slot];
          if (pick && owned(pick)) {
            var f = cands.filter(function (c) { return c.item === pick; })[0];
            if (f) { Object.assign(patch, f.look); return; }
          }
          var best = cands.slice().sort(function (a, b) { return a.cf - b.cf; })[0];
          if (best) Object.assign(patch, best.look);
        });
        return patch;
      },
      fullParts: function () { return Object.assign({}, E.parts, E.effectiveWearPatch()); },
      wearOptions: function (slot) {
        var owned = function (i) { return (E.inv[i] || 0) > 0; };
        return [{ key: 'auto', ko: t('look.auto') }]
          .concat(E.CLOTHES.filter(function (c) { return c.slot === slot; }).map(function (c) {
            return { key: c.item, ko: tOr('item.' + c.item, c.item), locked: !owned(c.item) };
          }))
          .concat(E.invClothes.map(function (ic) { return { key: ic.name, ko: ic.name }; }));
      },
      refreshFrame: function () {
        window.TF_LOOK.charSheetUrl({ uuid: E.agentId, sex: E.sex, age: E.age, parts: E.fullParts() })
          .then(function (u) { if (S.lookEdit === E) { E.frameUrl = u; schedule(); } })
          .catch(function () {});
      },
    };
    E.refreshFrame();
    return E;
  }

  // ═══════════════ 렌더 본체 ═══════════════
  function collectScrolls(root) {
    var out = {};
    root.querySelectorAll('[data-scroll]').forEach(function (el) {
      out[el.getAttribute('data-scroll')] = el.scrollTop;
    });
    return out;
  }
  function applyScrolls(root, saved) {
    root.querySelectorAll('[data-scroll]').forEach(function (el) {
      var k = el.getAttribute('data-scroll');
      if (saved[k] != null) el.scrollTop = saved[k];
    });
  }

  // 섹션 슬롯 — worldsim 틱마다 render가 도므로 루트를 통째로 다시 쓰면
  // 팝업(tf-fade-in)·히어로 카드(tf-hero-in·걷기)의 입장 애니메이션이
  // 매 틱 재시작돼 깜빡인다. 슬롯을 고정하고 바뀐 섹션만 교체하며, 팝업·
  // 히어로는 뼈대 유지 + 값 갱신(heroUpdate·popupUpdate)으로 렌더한다.
  var SLOT_ORDER = ['topbar', 'hero', 'popup', 'credits', 'look'];
  var slots = {};
  var slotHtml = {};   // name → 마지막으로 쓴 HTML — 문자열 비교로 교체 차단
  function ensureSlots(root) {
    SLOT_ORDER.forEach(function (name) {
      var el = slots[name];
      if (!el || !root.contains(el)) {
        el = root.querySelector(':scope > [data-slot="' + name + '"]');
        if (!el) {
          el = document.createElement('div');
          el.setAttribute('data-slot', name);
          root.appendChild(el);
        }
        slots[name] = el;
        slotHtml[name] = null;
      }
    });
  }
  function setSlot(name, html) {
    if (slotHtml[name] === html) return;
    slotHtml[name] = html;
    slots[name].innerHTML = html;
  }

  function render() {
    var root = ensureRoot();
    ensureSlots(root);
    var saved = collectScrolls(root);
    var focus = document.activeElement && document.activeElement.id;
    setSlot('topbar', topbarHtml());
    heroUpdate();
    popupUpdate();
    setSlot('credits', S.creditsOpen ? creditsHtml() : '');
    setSlot('look', S.lookEdit ? lookHtml() : '');
    applyScrolls(root, saved);
    if (focus) {
      var el = document.getElementById(focus);
      if (el && document.activeElement !== el) {
        el.focus();
        if (el.setSelectionRange && el.value != null) {
          try { el.setSelectionRange(el.value.length, el.value.length); } catch (e) { /* pass */ }
        }
      }
    }
    wireJoinInputs();
  }

  /** 참여 폼 입력 — 타이핑 중 재렌더 금지(포커스 유지), 상태만 갱신. */
  function wireJoinInputs() {
    var name = document.getElementById('tf-join-name');
    var age = document.getElementById('tf-join-age');
    var sex = document.getElementById('tf-join-sex');
    var persona = document.getElementById('tf-join-persona');
    if (name) name.oninput = function () { S.joinForm.name = name.value; };
    if (age) age.oninput = function () {
      S.joinForm.age = age.value.replace(/\D/g, '').replace(/^0+/, '').slice(0, 2);
      if (age.value !== S.joinForm.age) age.value = S.joinForm.age;
    };
    if (sex) sex.onchange = function () { S.joinForm.sex = sex.value; schedule(); };
    if (persona) persona.oninput = function () { S.joinForm.persona = persona.value; };
  }

  // ═══════════════ 이벤트 위임 ═══════════════
  document.addEventListener('click', function (ev) {
    var el = ev.target.closest ? ev.target.closest('[data-act]') : null;
    if (S.menuOpen && !el && !ev.target.closest('.tf-menu-anchor')) {
      S.menuOpen = false;
      schedule();
      return;
    }
    if (!el) return;
    var act = el.getAttribute('data-act');
    var key = el.getAttribute('data-key');
    if (act === 'menu') { S.menuOpen = !S.menuOpen; schedule(); }
    else if (act === 'popup') { openPopup(key); }
    else if (act === 'clear-notice') { S.notice = ''; schedule(); }
    else if (act === 'hero-close') {
      // 선택 해제 — 단일 소스(Rust Selection)에 빈 요청 → 링·추적·채팅 독 함께 정리
      if (window.terrafolk && window.terrafolk.selectAgent) window.terrafolk.selectAgent('');
    }
    else if (act === 'hero-cam') {
      // 재추적 — 같은 id 재선택도 epoch로 구분돼 카메라가 다시 따라온다(pick.rs)
      if (S.selected && window.terrafolk && window.terrafolk.selectAgent) {
        window.terrafolk.selectAgent(S.selected);
      }
    }
    else if (act === 'hero-chat') {
      // 대화 — ui.js 채팅 로그 펼치고 입력에 포커스
      var head = document.querySelector('.tf-chathead');
      if (head && !head.classList.contains('on')) head.click();
      var inp = document.querySelector('.tf-chatdock .tf-input');
      if (inp) inp.focus();
    }
    else if (act === 'hero-detail') { openPopup('chars'); }
    else if (act === 'close-popup') {
      if (ev.target === el) { S.popup = null; schedule(); }   // 백드롭·X만 — 내부 클릭 전파 무시
    }
    else if (act === 'select') {
      S.selected = el.getAttribute('data-id');
      S.selRelic = null;
      // 선택 단일 소스 — Rust Selection으로 요청(링·카메라 추적·채팅 바 ui.js)
      if (window.terrafolk && window.terrafolk.selectAgent) window.terrafolk.selectAgent(S.selected);
      onSelected();
      schedule();
    }
    else if (act === 'preset') {
      var p = I.PERSONA_PRESETS.filter(function (x) { return x.key === key; })[0];
      if (p) { S.joinForm.persona = p.text; schedule(); }
    }
    else if (act === 'join') { join(); }
    else if (act === 'look-edit') {
      var m = S.mineDetail.filter(function (x) { return x.id === el.getAttribute('data-id'); })[0];
      if (m) { S.lookEdit = makeLookEditor(m); schedule(); }
    }
    else if (act === 'look-tab') { if (S.lookEdit) { S.lookEdit.tab = key; schedule(); } }
    else if (act === 'look-save') { lookSave(); }
    else if (act === 'close-look') { if (ev.target === el) { S.lookEdit = null; schedule(); } }
    else if (act === 'close-relic') { S.selRelic = null; schedule(); }
    else if (act === 'sel-inv') { S.selInv = key; schedule(); }
    else if (act === 'close-inv') { if (ev.target === el) { S.selInv = null; schedule(); } }
    else if (act === 'logcat') { S.logCat = key; schedule(); }
    else if (act === 'locale') { I.setLocale(key); schedule(); }
    else if (act === 'credits') { S.creditsOpen = true; schedule(); }
    else if (act === 'close-credits') { if (ev.target === el) { S.creditsOpen = false; schedule(); } }
    else if (act === 'logout') { logout(); }
  });

  // look-slide는 행(row) 단위로 처리 — 캡처 단계에서 클릭된 optrow의 순서를 읽는다.
  // 잠긴 옵션(미보유 의상)은 볼 수만 있고 선택은 스킵(FE와 동일).
  document.addEventListener('click', function (ev) {
    var nav = ev.target.closest ? ev.target.closest('[data-act="look-slide"]') : null;
    if (!nav || !S.lookEdit) return;
    var row = nav.closest('.tf-optrow');
    if (!row) return;
    var idx = Array.prototype.indexOf.call(row.parentNode.children, row);
    var E = S.lookEdit;
    var tab = E.tab;
    var spec;
    if (tab === 'clothes') spec = { slot: ['top', 'bottom', 'feet'][idx] };
    else if (tab === 'hair') spec = { part: ['hair', 'hairColor'][idx] };
    else spec = { part: ['skin', 'eye', 'beard', 'glasses'][idx] };
    var dir = Number(nav.getAttribute('data-dir'));
    var options, value, apply;
    if (spec.slot) {
      options = E.wearOptions(spec.slot);
      value = E.wears[spec.slot] || 'auto';
      apply = function (k) { E.pickWear(spec.slot, k); };
    } else {
      var list = E.opts[spec.part] || [];
      options = list.map(function (v) { return { key: v, ko: E.prettyPart(v) }; });
      value = E.parts[spec.part] || (options[0] && options[0].key) || 'none';
      apply = function (k) { E.setPart(spec.part, k); };
    }
    var i = Math.max(0, options.findIndex(function (o) { return o.key === value; }));
    var next = options[(i + dir + options.length) % options.length];
    if (next && !next.locked) apply(next.key);
    else schedule();
  }, true);

  var modalEscLatch = false;
  document.addEventListener('keydown', function (ev) {
    if (ev.key === 'Escape') {
      if (S.lookEdit) { S.lookEdit = null; modalEscLatch = true; schedule(); }
      else if (S.creditsOpen) { S.creditsOpen = false; modalEscLatch = true; schedule(); }
      else if (S.popup) { S.popup = null; modalEscLatch = true; schedule(); }
      else if (S.menuOpen) { S.menuOpen = false; modalEscLatch = true; schedule(); }
    }
  });

  // ═══════════════ REST ═══════════════
  function notify(msg) {
    S.notice = String(msg || '');
    schedule();
    if (I.getLocale() !== 'en' && S.notice) {
      I.translateText(I.getLocale(), S.notice).then(function (tr) {
        if (tr !== S.notice) { S.notice = tr; schedule(); }
      }).catch(function () {});
    }
  }

  function openPopup(key) {
    S.menuOpen = false;
    S.popup = key;
    schedule();
    if (key === 'chars') { loadRelations(); onSelected(); }
  }

  var backlogSeeded = false;
  /** 백로그(이벤트·채팅) — 부팅 시 1회 시딩. 실패 시 2.5s 간격 3회 재시도. */
  function loadBacklogs() {
    if (backlogSeeded) return;
    var tries = 0;
    var load = function () {
      var okEvents = false, okChat = false;
      authFetch('/api/worldsim/events').then(function (r) { return r.ok ? r.json() : null; })
        .then(function (rows) {
          if (rows) {
            okEvents = true;
            var seeded = rows.slice(-400).map(function (r2) {
              return { id: r2.id, tick: r2.tick, kind: r2.kind, agent_id: r2.agent_id, payload: r2.payload, ts: r2.ts };
            });
            S.events = seeded.concat(S.events).slice(-400);
            schedule();
          }
        }).catch(function () {}).finally(function () { retryCheck(okEvents); });
      authFetch('/api/worldsim/chat').then(function (r) { return r.ok ? r.json() : null; })
        .then(function (rows) {
          if (rows) {
            okChat = true;
            var seeded = rows.slice(-120).map(function (r2) {
              return Object.assign({}, r2, { id: nextRowId(), orig: r2.text });
            });
            S.chat = seeded.concat(S.chat).slice(-120);
            schedule();
          }
        }).catch(function () {}).finally(function () { retryCheck(okChat); });
    };
    var retryCheck = function (ok) {
      if (ok) backlogSeeded = true;
      else if (++tries < 3) setTimeout(function () { if (!backlogSeeded) load(); }, 2500);
    };
    load();
  }

  var relTimer = null;
  function loadRelations() {
    if (relTimer) clearInterval(relTimer);
    var load = function () {
      authFetch('/api/worldsim/relations').then(function (r) { return r.ok ? r.json() : null; })
        .then(function (rows) { if (rows) { S.relations = rows; if (S.popup === 'chars') schedule(); } })
        .catch(function () {});
    };
    load();
    relTimer = setInterval(load, 20000);
  }

  var selTimer = null;
  /** 선택 주민 상세 — 생활기록(공개)·기억(로그인), 15s 갱신. */
  function onSelected() {
    if (selTimer) { clearInterval(selTimer); selTimer = null; }
    if (!S.selected) return;
    var load = function () {
      authFetch('/api/worldsim/agents/' + encodeURIComponent(S.selected) + '/history')
        .then(function (r) { return r.ok ? r.json() : null; })
        .then(function (rows) {
          if (rows) {
            S.history = rows.slice(0, 25).map(function (h) {
              return { ts: h.ts, type: h.type, text: h.text, orig: h.text };
            });
            if (S.popup === 'chars') schedule();
          }
        }).catch(function () {});
      if (S.scope && S.scope.kind === 'user') {
        authFetch('/api/worldsim/agents/' + encodeURIComponent(S.selected) + '/memories')
          .then(function (r) { return r.ok ? r.json() : null; })
          .then(function (rows) {
            if (rows) {
              S.memories = rows.slice(0, 20).map(function (m) {
                return { id: m.id, kind: m.kind, text: m.text, orig: m.text, ts: m.ts };
              });
              if (S.popup === 'chars') schedule();
            }
          }).catch(function () {});
      }
    };
    load();
    selTimer = setInterval(load, 15000);
  }

  function loadMine() {
    if (!(S.scope && S.scope.kind === 'user')) return;
    authFetch('/api/worldsim/me').then(function (r) { return r.ok ? r.json() : null; })
      .then(function (mine) {
        if (!mine) return;
        mine.forEach(function (m) {
          var cur = S.live[m.id];
          if (cur) { cur.mine = true; cur.name = m.name; }
        });
        S.liveSig = liveSignature();
        S.mineDetail = mine.map(function (m) {
          return {
            id: m.id, name: m.name, sex: m.sex || 'female', age: m.age || 30,
            parts: m.parts || {}, wears: m.wears || {},
            inventory: m.inventory || {}, clothingInventions: m.clothingInventions || [],
          };
        });
        schedule();
      }).catch(function () {});
  }

  function join() {
    if (S.busy || !S.joinForm.name.trim()) return;
    S.busy = true;
    schedule();
    authFetch('/api/worldsim/join', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        name: S.joinForm.name.trim() || t('join.name.default'),
        persona: {
          age: Number(S.joinForm.age) || 30,
          sex: S.joinForm.sex,
          persona: S.joinForm.persona.trim() || undefined,
        },
      }),
    }).then(function (r) {
      return r.ok ? r.json() : r.json().then(function (j) { throw new Error(j.message || j.detail || 'join failed'); });
    }).then(function (res) {
      S.busy = false;
      S.joinForm.name = '';
      S.joinForm.persona = '';
      notify(t('notice.join', { name: res.name }));
      loadMine();
      schedule();
    }).catch(function (e) {
      S.busy = false;
      notify((e && e.message) || t('err.generic'));
      schedule();
    });
  }

  function lookSave() {
    var E = S.lookEdit;
    if (!E || E.busy) return;
    E.busy = true;
    schedule();
    // 옷 슬롯은 wears가 관장 — 수동 parts의 옷 잔존값은 정리해 파생이 우선하게.
    // wears는 에디터 선택(E.wears, 미지정 슬롯=auto 초기화)을 그대로 — 하드코딩
    // auto면 사용자 선택·기존 저장값이 전부 지워진다(FE LookEditor 규약)
    var partsOut = Object.assign({}, E.parts);
    ['top', 'topColor', 'bottom', 'bottomColor', 'feet', 'feetColor'].forEach(function (k) { delete partsOut[k]; });
    var wearsOut = { top: E.wears.top || 'auto', bottom: E.wears.bottom || 'auto', feet: E.wears.feet || 'auto' };
    authFetch('/api/worldsim/agents/' + encodeURIComponent(E.agentId) + '/look', {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ parts: partsOut, wears: wearsOut }),
    }).then(function (r) {
      if (!r.ok) return r.json().then(function (j) { throw new Error(j.message || j.detail || 'save failed'); });
      return r.json();
    }).then(function () {
      S.lookEdit = null;
      notify(t('look.saved'));
      loadMine();
      schedule();
    }).catch(function (e) {
      E.busy = false;
      notify((e && e.message) || t('look.err.save'));
      schedule();
    });
  }

  function logout() {
    var f = window.terrafolk && window.terrafolk.logout;
    if (f) Promise.resolve(f()).then(function () {
      S.scope = null;
      S.mineDetail = [];
      Object.keys(S.live).forEach(function (k) { S.live[k].mine = false; });
      schedule();
    });
  }

  // ═══════════════ feed — worldsim.js가 호출 ═══════════════
  // 침전 오버레이 — CSS(.tf-precip) 구동. 3D 파이프라인은 WebGL2 프래그먼트
  // 유니폼 한도(12) 초과로 침전 재질을 만들 수 없어 여기서 처리한다.
  var precipEl = null;
  function syncPrecip(weather) {
    if (!precipEl) precipEl = document.querySelector('.tf-precip');
    if (!precipEl) return;
    var cls = 'tf-precip';
    if (weather === 'rain') cls += ' rain';
    else if (weather === 'storm') cls += ' rain storm';
    else if (weather === 'snow') cls += ' snow';
    if (precipEl.className !== cls) precipEl.className = cls;
  }

  function feedFrame(f) {
    // 시계
    S.clock = {
      day: f.day, hour: f.hour, minute: f.minute, season: f.season,
      weather: f.weather, sunrise: f.sunrise, sunset: f.sunset,
    };
    syncPrecip(f.weather);
    // 주민 — 프레임에 없는 기존 행은 유지(이름 시드), 퇴장 제거
    var seen = {};
    (f.agents || []).forEach(function (a) {
      seen[a.id] = 1;
      var cur = S.live[a.id];
      S.live[a.id] = {
        id: a.id,
        name: a.name || (cur ? cur.name : a.id),
        status: a.status != null ? a.status : (cur ? cur.status : ''),
        actType: a.actType != null ? a.actType : (cur ? cur.actType : ''),
        actProgress: a.actProgress != null ? a.actProgress : (cur ? cur.actProgress : 0),
        needs: a.needs || (cur ? cur.needs : {}),
        inv: a.inventory != null ? I.parseInv(a.inventory) : (cur ? cur.inv : {}),
        goalText: a.goalText != null ? a.goalText : (cur ? cur.goalText : ''),
        look: a.look != null ? a.look : (cur ? cur.look : null),
        kind: a.kind != null ? a.kind : (cur ? cur.kind : ''),
        x: a.x != null ? a.x : (cur ? cur.x : 0),
        y: a.y != null ? a.y : (cur ? cur.y : 0),
        mine: cur ? cur.mine : false,
      };
    });
    Object.keys(S.live).forEach(function (k) { if (!seen[k]) delete S.live[k]; });
    // 자원 — 스냅숏 시드
    S.resMap = {};
    (f.resources || []).forEach(function (r) {
      S.resMap[r.id] = r;
    });
    // 발명
    if (f.inventions) {
      S.inventionsAll = f.inventions;
      var inv = {};
      f.inventions.forEach(function (i) {
        if (i.icon && i.icon.tpl) {
          inv[i.name] = {
            tpl: i.icon.tpl, primary: i.icon.primary, accent: i.icon.accent,
            purpose: i.purpose, inventor: i.inventor, based_on: i.based_on,
          };
        }
      });
      S.inventions = inv;
    }
    // 내 캐릭 확인 — 주민 집합이 바뀌면
    var sig = liveSignature();
    if (sig !== S.liveSig) {
      S.liveSig = sig;
      loadMine();
    }
    // 선택 해제는 Rust 단일 소스(tf.onPickAgent(null))에 맡긴다 — 프레임 공백
    // (재접속·드랍)으로 S.live가 잠깐 비어도 카드·추적이 튕기지 않는다.
    // Rust는 AgentGrace 유예 후 실제 철거 시에만 null을 통보한다.
    schedule();
  }
  function liveSignature() {
    return Object.keys(S.live).sort().join(',');
  }
  function feedEvent(e) {
    S.events.push(Object.assign({ id: nextRowId(), ts: new Date().toISOString() }, e));
    if (S.events.length > 400) S.events.splice(0, S.events.length - 400);
    // 스폰 이름 시드 — 접속 후 합류한 주민도 이름으로
    if (e.kind === 'spawn' && e.agent_id && e.payload && e.payload.name) {
      var cur = S.live[e.agent_id];
      if (cur && cur.name !== String(e.payload.name)) cur.name = String(e.payload.name);
    }
    if (S.popup === 'log' || S.popup === 'inventions') schedule();
  }
  function feedChat(c) {
    S.chat.push(Object.assign({ id: nextRowId(), orig: c.text, ts: new Date().toISOString() }, c));
    if (S.chat.length > 120) S.chat.splice(0, S.chat.length - 120);
    if (S.popup === 'log') schedule();
  }
  function feedScope(scope) {
    S.scope = scope;
    if (scope && scope.kind === 'user') loadMine();
    schedule();
  }

  // ── Rust 픽 브리지 — 빈 땅 클릭 좌표로 유물 카드(§17.38).
  //    주민 픽킹은 Rust pick.rs가直接 하고 tf.onPickAgent로 돌아온다.
  function pickAt(x, y) {
    // 건물·묘지 탭 툴팁(§17.78 — 터치 고정 2.5s) — tf-world-ui가 판정·표시.
    // 유물 카드보다 우선(발판 안 클릭은 집 안내가 우선).
    if (window.TF_WORLD_UI && window.TF_WORLD_UI.showResAt(x, y)) return;
    var bestR = null, bestRd = 4;
    Object.keys(S.resMap).forEach(function (rid) {
      var r = S.resMap[rid];
      if (r.kind !== 'relic') return;
      var d = (r.x + 0.5 - x) * (r.x + 0.5 - x) + (r.y + 0.5 - y) * (r.y + 0.5 - y);
      if (d < bestRd) { bestRd = d; bestR = r; }
    });
    if (bestR) {
      var st = bestR.state || {};
      S.selRelic = {
        name: bestR.species, by: String(st.by || ''),
        purpose: String(st.purpose || ''), basedOn: String(st.based_on || ''),
      };
      openPopup('res');
    }
  }

  // ── Rust 선택 통보 수신 — ui.js가 먼저 등록한 핸들러를 chaining으로 이어받는다.
  //    (로드 순서: ui.js → tf-ui.js — 채팅 바와 팝업이 같은 선택 신호를 함께 받는다)
  (function chainPickAgent() {
    var tf = window.terrafolk;
    if (!tf) return;
    var prev = tf.onPickAgent;
    tf.onPickAgent = function (id) {
      if (prev) { try { prev(id); } catch (e) { console.error('[tf-ui] onPickAgent 체인 오류', e); } }
      S.selected = id || null;
      S.selRelic = null;
      onSelected();
      schedule();   // 히어로 카드 즉시 반영 — 단선 시엔 틱이 안 오니 여기서 갱신
    };
  })();

  // ═══════════════ 공개 API ═══════════════
  // modalEsc — 이번 Escape로 DOM 모달을 닫았는지 읽고 지운다(Rust pick.rs).
  // 같은 키 입력에서 JS가 먼저 모달을 닫으므로 프레임 시점의 uiModal 읽기로는
  // 판별 불가 — 래치로 1회 소진 계약.
  var tfb = window.terrafolk;
  if (tfb) tfb.modalEsc = function () { var v = modalEscLatch; modalEscLatch = false; return v; };

  // 라이브 프라임 로딩 캡션(index.html 정적 폴백 텍스트)을 사전 언어로 교체 —
  // 키(boot.loading)는 ../terrafolk 정본 사전 등록 후 갱신된다(tOr 폴백 유지).
  document.addEventListener('DOMContentLoaded', function () {
    var cap = document.querySelector('.tf-boot-caption');
    if (cap) cap.textContent = tOr('boot.loading', '세계에 접속하는 중…');
  });
  window.TF_UI = {
    state: S,
    feed: {
      frame: feedFrame,
      event: feedEvent,
      chat: feedChat,
      scope: feedScope,
    },
    open: openPopup,
    openRelic: function (r) { S.selRelic = r; openPopup('res'); },
    pickAt: pickAt,
    seedBacklogs: loadBacklogs,
    notify: notify,
    setNet: function (live) { S.netLive = live; },
    render: schedule,
    // 소지 칩 아이콘 HTML — ui.js 우측 레일 소지품 패널이 재사용(동일 룩 유지)
    chipIcon: chipIconHtml,
  };
  I.onLocale(function () { schedule(); });

  // 루트 뼈대 — worldsim.js 접속 전에도 상단 바가 뜬다(로케일만으로 구성)
  schedule();
})();
