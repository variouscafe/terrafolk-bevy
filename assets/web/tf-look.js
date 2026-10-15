// Terrafolk LPC 외형 합성기 — FE src/lib/lpcChar.ts의 바닐라 JS 포트.
// 결정적 uuid 시드 부품 선택 + 팔레트 리컬러 + walk 시트 캔버스 합성.
// 외형 편집(tf-ui LookEditor) 미리보기가 사용. 부품 PNG: assets/lpc/parts.
(function () {
  'use strict';
  var LPC = window.TF_DATA.LPC;
  var R = LPC.RULES;
  var PALETTES = LPC.PALETTES;
  var SOURCE_PALETTES = LPC.SOURCE_PALETTES;
  var PARTS_BASE = LPC.PARTS_BASE;
  var BY_ID = new Map();
  LPC.POOL_ITEMS.forEach(function (i) { BY_ID.set(i.id, i); });

  // ---- 결정적 RNG(FE lib/rng.ts 포트) ----
  function hash32(s) {
    var h = 2166136261;
    for (var i = 0; i < s.length; i++) {
      h ^= s.charCodeAt(i);
      h = Math.imul(h, 16777619);
    }
    return h >>> 0;
  }
  function mulberry32(seed) {
    var a = seed;
    return function () {
      a |= 0;
      a = (a + 0x6d2b79f5) | 0;
      var t = Math.imul(a ^ (a >>> 15), 1 | a);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  function wpick(rng, w) {
    var keys = Object.keys(w).filter(function (k) { return w[k] > 0; });
    var total = 0;
    keys.forEach(function (k) { total += w[k]; });
    var r = rng() * total;
    for (var i = 0; i < keys.length; i++) {
      r -= w[keys[i]];
      if (r <= 0) return keys[i];
    }
    return keys.length ? keys[keys.length - 1] : '';
  }
  function pickForBt(rng, w, bt) {
    var ok = {};
    Object.keys(w).forEach(function (k) {
      var it = BY_ID.get(k);
      if (it && w[k] > 0 && (it.byBt[bt] || it.byBt['*'])) ok[k] = w[k];
    });
    var ks = Object.keys(ok);
    return ks.length ? wpick(rng, ok) : '';
  }

  /** 성별 정규화 — FE는 한국어 라벨만 받으나 이 클라는 vocab id('female')로도 받는다. */
  function gOf(sex) {
    return sex === '여자' || sex === 'female' || sex === 'f' || sex === 'F' ? 'f' : 'm';
  }

  /** 부위별 선택 가능 목록(FE lookPartOptions 포트 — 에디터 ◀▶ 사이클 재료). */
  function lookPartOptions(sex, age) {
    var g = gOf(sex);
    var a = age != null ? age : 40;
    var ac = a < 20 ? 'teen' : a < 65 ? 'adult' : 'senior';
    var bucket = g + '_' + ac;
    var bt = ac === 'teen' ? 'teen' : g === 'f' ? 'female' : 'male';
    function ok(w) {
      return Object.keys(w).filter(function (k) {
        var it = BY_ID.get(k);
        return w[k] > 0 && !!it && !!(it.byBt[bt] || it.byBt['*']);
      });
    }
    function pos(w) {
      return Object.keys(w).filter(function (k) { return w[k] > 0; });
    }
    return {
      skins: pos(R.skins),
      eyes: pos(R.eyes),
      hairColors: pos(R.hair_colors[ac]),
      hairs: ok((g === 'f' ? R.hair_female : R.hair_male)[bucket] || {}),
      beards: g === 'm' ? pos(R.mustaches).filter(function (k) { return BY_ID.has(k); }).concat(['none']) : ['none'],
      glasses: pos(R.glasses[ac]).filter(function (k) { return BY_ID.has(k); }).concat(['none']),
      tops: ok(R.tops[bucket] || {}),
      bottoms: g === 'f' ? ok(R.bottoms[bucket] || {}).concat(ok(R.skirts)) : ok(R.bottoms[bucket] || {}),
      feet: ok(R.feet),
    };
  }

  /** uuid → 외형(FE buildLook 포트 — 오버라이드 유효성·rng 소비 순서 동일). */
  function buildLook(uuid, sex, age, ov) {
    ov = ov || {};
    var rng = mulberry32(hash32(uuid + ':lpc'));
    var g = gOf(sex);
    var a = age != null ? age : 40;
    var ac = a < 20 ? 'teen' : a < 65 ? 'adult' : 'senior';
    var bucket = g + '_' + ac;
    var bt = ac === 'teen' ? 'teen' : g === 'f' ? 'female' : 'male';

    function has(w, k) { return !!k && !!w && (w[k] || 0) > 0 && BY_ID.has(k); }
    function colorOf(w, k, pick) {
      return (k && w[k]) ? k : (pick ? pick() : wpick(rng, w));
    }
    function pickId(w, k) { return has(w, k) ? k : pickForBt(rng, w || {}, bt); }

    var skin = colorOf(R.skins, ov.skin);
    var eye = colorOf(R.eyes, ov.eye);
    var hairColor = colorOf(R.hair_colors[ac], ov.hairColor);

    var layers = [];
    function add(id, colors) {
      var it = BY_ID.get(id);
      if (!it) return;
      var refs = it.byBt[bt] || it.byBt['*'];
      if (!refs) return;
      var recolor = [];
      Object.keys(it.mats).forEach(function (slot) {
        var color = colors[slot];
        if (!color) return;
        var table = PALETTES[it.mats[slot]];
        var src = table && table[SOURCE_PALETTES[it.mats[slot]] || ''];
        var dst = table && table[color];
        if (src && dst) recolor.push({ src: src, dst: dst });
      });
      refs.forEach(function (ref) {
        var png = ref.png === '{COLOR}' ? (colors[':color'] || 'black') : ref.png;
        layers.push({ z: ref.z, png: PARTS_BASE + id + '/' + png + '.png', recolor: recolor });
      });
    }

    add('body', { skin: skin });
    add(wpick(rng, R.heads[bucket] || {}), { skin: skin, eye: eye });
    add(wpick(rng, R.brows[ac]), { hair: hairColor });

    var nose = wpick(rng, R.noses[ac]);
    if (nose !== 'none') add(nose, { skin: skin });
    if (rng() < (R.wrinkles_prob[ac] || 0)) add('head_wrinkles', { skin: skin });

    var beard = 'none';
    if (g === 'm') {
      var okOv = ov.beard !== undefined && (ov.beard === 'none'
        || ((R.mustaches[ov.beard] || 0) > 0 && BY_ID.has(ov.beard)));
      if (okOv) beard = ov.beard;
      else if (rng() < (R.beard_prob[ac] || 0)) {
        var b = wpick(rng, R.beards[ac]);
        beard = b === 'beards_mustache_only' ? wpick(rng, R.mustaches) : b;
      }
    }
    if (beard !== 'none') add(beard, { hair: hairColor });

    var glasses = 'none';
    {
      var okOv2 = ov.glasses !== undefined && (ov.glasses === 'none'
        || ((R.glasses[ac][ov.glasses] || 0) > 0 && BY_ID.has(ov.glasses)));
      if (okOv2) glasses = ov.glasses;
      else if (rng() < (R.glasses_prob[ac] || 0)) glasses = wpick(rng, R.glasses[ac]);
    }
    if (glasses !== 'none') add(glasses, { ':color': wpick(rng, R.glasses_colors) });

    var hair = pickId((g === 'f' ? R.hair_female : R.hair_male)[bucket] || {}, ov.hair);
    add(hair, { hair: hairColor });

    var topColor = colorOf(R.cloth_top, ov.topColor);
    var top = pickId(R.tops[bucket] || {}, ov.top);
    add(top, { cloth: topColor });

    var bottomsW = R.bottoms[bucket] || {};
    var skirtOv = g === 'f' && (R.skirts[ov.bottom || ''] || 0) > 0 && BY_ID.has(ov.bottom || '');
    var bottom, bottomColor;
    if (has(bottomsW, ov.bottom) || skirtOv) {
      bottom = ov.bottom || '';
      bottomColor = colorOf(skirtOv ? R.cloth_skirt : R.cloth_bottom, ov.bottomColor);
    } else if (g === 'f' && rng() < (R.skirt_prob[bucket] || 0)) {
      bottom = pickForBt(rng, R.skirts, bt);
      bottomColor = colorOf(R.cloth_skirt, ov.bottomColor);
    } else {
      bottom = pickForBt(rng, bottomsW, bt);
      bottomColor = colorOf(R.cloth_bottom, ov.bottomColor);
    }
    add(bottom, { cloth: bottomColor });

    var feet = pickId(R.feet, ov.feet);
    var feetColor = colorOf(R.cloth_feet, ov.feetColor);
    add(feet, { cloth: feetColor });

    layers.sort(function (x, y) { return x.z - y.z; });
    return {
      bt: bt, bucket: bucket, layers: layers,
      picks: {
        skin: skin, eye: eye, hairColor: hairColor, hair: hair, beard: beard,
        glasses: glasses, top: top, topColor: topColor, bottom: bottom,
        bottomColor: bottomColor, feet: feet, feetColor: feetColor,
      },
    };
  }

  // ---- 렌더링(FE 렌더 파트 포트 — walk 4방향 행만 합성(미리보기용)) ----
  var SHEET_W = 576;
  var WALK_H = 4 * 64;
  var TOL = 8; // 팔레트 매칭 톨러런스(FE QA 검증값)

  function hexRgb(h) {
    return [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16)];
  }
  var pairCache = new Map();
  function colorPairs(recolor) {
    var out = [];
    recolor.forEach(function (r) {
      for (var i = 0; i < r.src.length && i < r.dst.length; i++) {
        var s = hexRgb(r.src[i]), d = hexRgb(r.dst[i]);
        out.push({ sr: s[0], sg: s[1], sb: s[2], dr: d[0], dg: d[1], db: d[2] });
      }
    });
    return out;
  }
  var imgCache = new Map();
  function loadImage(url) {
    var p = imgCache.get(url);
    if (!p) {
      p = new Promise(function (resolve, reject) {
        var img = new Image();
        img.onload = function () { resolve(img); };
        img.onerror = function () { reject(new Error('부품 로드 실패: ' + url)); };
        img.src = url;
      });
      p.catch(function () { imgCache.delete(url); });
      imgCache.set(url, p);
    }
    return p;
  }
  function drawRecolored(ctx, img, recolor, scratch) {
    var w = img.width, h = img.height;
    // 캐시 키는 전체 램프를 다 묶는다 — 첫 hex+길이만 쓰면 같은 값으로 시작하는
    // 서로 다른 팔레트 쌍이 충돌해 이전 색상표로 오리컬러된다
    var key = recolor.map(function (r) {
      return r.src.join('>') + '=>' + r.dst.join('>');
    }).join('|');
    var pairs = pairCache.get(key);
    if (!pairs) {
      pairs = colorPairs(recolor);
      pairCache.set(key, pairs);
    }
    var sctx = scratch.getContext('2d', { willReadFrequently: true });
    sctx.clearRect(0, 0, w, h);
    sctx.drawImage(img, 0, 0);
    if (pairs.length) {
      var data = sctx.getImageData(0, 0, w, h);
      var px = data.data;
      for (var i = 0; i < px.length; i += 4) {
        if (px[i + 3] === 0) continue;
        var r0 = px[i], g0 = px[i + 1], b0 = px[i + 2];
        for (var j = 0; j < pairs.length; j++) {
          var p = pairs[j];
          if (Math.abs(r0 - p.sr) <= TOL && Math.abs(g0 - p.sg) <= TOL && Math.abs(b0 - p.sb) <= TOL) {
            px[i] = p.dr; px[i + 1] = p.dg; px[i + 2] = p.db;
            break;
          }
        }
      }
      sctx.putImageData(data, 0, 0);
    }
    ctx.drawImage(scratch, 0, 0, w, h, 0, 0, w, h);
  }

  var sheetCache = new Map();
  /** walk 시트(576×256) dataURL — uuid+나이버킷+parts별 1회 캐시(FE와 동일 규약). */
  function charSheetUrl(p) {
    var partsKey = p.parts && Object.keys(p.parts).length
      ? ':' + Object.keys(p.parts).filter(function (k) { return p.parts[k] != null; })
          .map(function (k) { return k + '=' + p.parts[k]; }).sort().join(',')
      : '';
    var bucket = (p.age != null ? p.age : 40) < 20 ? 'teen' : (p.age != null ? p.age : 40) < 65 ? 'adult' : 'senior';
    var key = p.uuid + ':' + bucket + partsKey;
    var c = sheetCache.get(key);
    if (!c) {
      c = (async function () {
        var look = buildLook(p.uuid, p.sex, p.age, p.parts);
        var imgs = await Promise.all(look.layers.map(function (l) {
          return loadImage(l.png).catch(function () { return null; });
        }));
        var canvas = document.createElement('canvas');
        canvas.width = SHEET_W;
        canvas.height = WALK_H;
        var ctx = canvas.getContext('2d');
        for (var i = 0; i < look.layers.length; i++) {
          if (!imgs[i]) continue;
          // 부품 walk PNG는 576×256(4방향×9프레임) — 리컬러 후 통짜로 그린다
          var sc2 = document.createElement('canvas');
          sc2.width = imgs[i].width;
          sc2.height = imgs[i].height;
          drawRecolored(ctx, imgs[i], look.layers[i].recolor, sc2);
        }
        return canvas.toDataURL('image/png');
      })();
      c.catch(function () { sheetCache.delete(key); });
      sheetCache.set(key, c);
    }
    return c;
  }

  // ---- 발명품 아이콘 합성(FE lib/worldsim/itemIcon.ts 포트) ----
  // game-icons SVG(CC BY 3.0) 실루엣을 스펙(tpl+primary+accent)으로 리컬러 후
  // 16px 픽셀 그리드 스냅 → 정수배 확대. 실패 시 '' — 호출부가 이모지 폴백.
  var GRID = 16;
  var SNAP = 127;
  var iconCache = new Map();
  var iconSvgCache = new Map();
  function renderItemIcon(spec, size) {
    size = size || 32;
    var key = spec.tpl + '|' + spec.primary + '|' + spec.accent + '|' + size;
    var p = iconCache.get(key);
    if (!p) {
      p = (async function () {
        var s = iconSvgCache.get(spec.tpl);
        if (!s) {
          var r = await fetch('assets/icons/' + spec.tpl + '.svg');
          if (!r.ok) throw new Error('아이콘 없음: ' + spec.tpl);
          s = await r.text();
          iconSvgCache.set(spec.tpl, s);
        }
        var body = s
          .replace(/<path d="M0 0h512v512H0z"\/>/, '')
          .replace(/#fff\b/gi, spec.primary);
        var blob = new Blob([body], { type: 'image/svg+xml' });
        var url = URL.createObjectURL(blob);
        try {
          var img = await new Promise(function (res, rej) {
            var i = new Image();
            i.onload = function () { res(i); };
            i.onerror = function () { rej(new Error('svg 래스터화 실패')); };
            i.src = url;
          });
          var grid = document.createElement('canvas');
          grid.width = GRID; grid.height = GRID;
          var gctx = grid.getContext('2d');
          gctx.imageSmoothingEnabled = true;
          gctx.drawImage(img, 0, 0, GRID, GRID);
          var data = gctx.getImageData(0, 0, GRID, GRID);
          for (var i0 = 3; i0 < data.data.length; i0 += 4) {
            data.data[i0] = data.data[i0] > SNAP ? 255 : 0;
          }
          // 고아 픽셀 제거 — 스냅 스펙클 노이즈(§17.38)
          function alphaAt(x, y) { return data.data[(y * GRID + x) * 4 + 3]; }
          for (var y = 0; y < GRID; y++) {
            for (var x = 0; x < GRID; x++) {
              if (alphaAt(x, y) === 0) continue;
              var attached = [[1, 0], [-1, 0], [0, 1], [0, -1]].some(function (d) {
                var nx = x + d[0], ny = y + d[1];
                return nx >= 0 && nx < GRID && ny >= 0 && ny < GRID && alphaAt(nx, ny) !== 0;
              });
              if (!attached) data.data[(y * GRID + x) * 4 + 3] = 0;
            }
          }
          gctx.putImageData(data, 0, 0);
          var c = document.createElement('canvas');
          c.width = size; c.height = size;
          var ctx = c.getContext('2d');
          ctx.imageSmoothingEnabled = false;
          ctx.drawImage(grid, 0, 0, GRID, GRID, 0, 0, size, size);
          return c.toDataURL('image/png');
        } finally {
          URL.revokeObjectURL(url);
        }
      })();
      p.catch(function () { iconCache.delete(key); });
      iconCache.set(key, p);
    }
    return p;
  }

  window.TF_LOOK = {
    hash32: hash32, mulberry32: mulberry32,
    lookPartOptions: lookPartOptions, buildLook: buildLook, charSheetUrl: charSheetUrl,
    renderItemIcon: renderItemIcon,
  };
})();
