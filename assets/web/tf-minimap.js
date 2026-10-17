// Terrafolk 미니맵 — DOM 캔버스(게임 줌과 무관, FE WorldMiniMap 포트).
// 지형은 TF_MAP_JSON(worldsim.js가 최초 1회 노출)로 1px/타일 베이크하고,
// 주민 점·자원 색·뷰포트 박스(Rust onOverlay → TF_VIEW)를 500ms 리드로우.
// 클릭·탭 → tf.panTo(x, y) 월드 점프(추적 해제, src/world/overlay.rs).
(function () {
  'use strict';
  function UI() { return window.TF_UI || null; }
  window.TF_VIEW = { rect: null };
  var viewRect = null;   // [cx, cy, w, h] 타일 단위 — 폴링 시점 스냅숏
  var MM_DESK = 148, MM_MOB = 112;
  var TERRAIN_COLORS = {
    1: '#63c5cf', 2: '#eaa56c', 3: '#8dc435', 4: '#4f9448', 5: '#9a948a', 6: '#c8a06a',
  };

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
      // tf 지역변수가 없었다 — window.terrafolk 브리지로(리뷰 #2, 점프 사망)
      var bridge = window.terrafolk;
      if (!mmBake || !bridge || !bridge.panTo) return;
      var r = mmHost.getBoundingClientRect();
      var x = (e.clientX - r.left) / r.width * mmBake.width;
      var y = (e.clientY - r.top) / r.height * mmBake.height;
      bridge.panTo(x, y);
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
  // 뷰포트 박스 갱신 — 500ms 드로우 직전 최신값 반영
  setInterval(function () { viewRect = window.TF_VIEW.rect; }, 200);
})();
