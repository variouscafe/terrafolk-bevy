#!/usr/bin/env node
/** FE(../terrafolk) 정본 → DOM 오버레이용 데이터 번들 생성.
 *
 * 원천(수동 편집 금지 — 이 스크립트로만 재생성):
 *   src/lib/i18n.ts                      → D(기본 사전)
 *   src/lib/i18n/vocab.gen.ts            → VOCAB·ITEM_EMOJI·ITEM_WT·WORN·INVCAT_*
 *   src/lib/i18n/events.gen.ts           → EVENT_DICT(ev.*·goal.* 템플릿)
 *   src/lib/client-meta.gen.ts           → CALENDAR(태양력 상수)
 *   src/lpc/credits-data.ts              → LPC 저작자 표기(/credits)
 *   src/lpc/pool-manifest.ts·palettes.ts → LPC 부품 풀·리컬러 팔레트
 *   src/lpc/rules.json                   → 외형 가중표
 *   public/lpc/parts/**.png              → LPC 부품 PNG(assets/lpc/parts로 복사)
 *
 * 산출: assets/web/tf-data.js — 웹 UI(tf-i18n/tf-ui)가 로드. 네이티브 빌드는
 * 이 파일 없이 게임만 동작(오버레이는 wasm 전용).
 *
 * 사용: node tools/gen-web-data.mjs [--skip-lpc]
 */
import { readFileSync, writeFileSync, cpSync, rmSync, existsSync, mkdirSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const FE = join(HERE, '..', '..', 'terrafolk');
const OUT = join(HERE, '..', 'assets', 'web', 'tf-data.js');
const LPC_DST = join(HERE, '..', 'assets', 'lpc');
const ICONS_DST = join(HERE, '..', 'assets', 'icons');

/** 소스에서 `const NAME…= <리터럴>` 추출 — 중괄호/대괄호 균형(문자열 인식). */
function extract(src, name) {
  const re = new RegExp(`(?:export\\s+)?const\\s+${name}\\b[^=]*=`);
  const m = re.exec(src);
  if (!m) throw new Error(`원천에서 ${name}을(를) 찾을 수 없음`);
  let i = m.index + m[0].length;
  while (src[i] === ' ') i++;
  const open = src[i];
  const close = open === '{' ? '}' : ']';
  let depth = 0, str = null, esc = false;
  let j = i;
  for (; j < src.length; j++) {
    const c = src[j];
    if (str) {
      if (esc) esc = false;
      else if (c === '\\') esc = true;
      else if (c === str) str = null;
      continue;
    }
    if (c === '"' || c === "'" || c === '`') { str = c; continue; }
    if (c === open) depth++;
    else if (c === close) {
      depth--;
      if (depth === 0) { j++; break; }
    }
  }
  let lit = src.slice(i, j)
    .replace(/\s+as\s+const\s*;?$/, '')
    .replace(/;$/, '');
  return Function(`"use strict"; return (${lit});`)();
}

const gen = (() => {
  const i18n = readFileSync(join(FE, 'src/lib/i18n.ts'), 'utf8');
  const vocab = readFileSync(join(FE, 'src/lib/i18n/vocab.gen.ts'), 'utf8');
  const events = readFileSync(join(FE, 'src/lib/i18n/events.gen.ts'), 'utf8');
  const meta = readFileSync(join(FE, 'src/lib/client-meta.gen.ts'), 'utf8');
  const credits = readFileSync(join(FE, 'src/lpc/credits-data.ts'), 'utf8');
  const pool = readFileSync(join(FE, 'src/lpc/pool-manifest.ts'), 'utf8');
  const palettes = readFileSync(join(FE, 'src/lpc/palettes.ts'), 'utf8');
  const rules = JSON.parse(readFileSync(join(FE, 'src/lpc/rules.json'), 'utf8'));

  const data = {
    D: extract(i18n, 'D'),
    VOCAB: extract(vocab, 'VOCAB_DICT'),
    ITEM_EMOJI: extract(vocab, 'ITEM_EMOJI'),
    ITEM_WT: extract(vocab, 'ITEM_WT'),
    WORN_ITEMS: extract(vocab, 'WORN_ITEMS'),
    INVCAT_GROUPS: extract(vocab, 'INVCAT_GROUPS'),
    INVCAT_ALIAS: extract(vocab, 'INVCAT_ALIAS'),
    INV_CAT_EMOJI: extract(vocab, 'INV_CAT_EMOJI'),
    EVENT_DICT: extract(events, 'EVENT_DICT'),
    CALENDAR: extract(meta, 'CALENDAR'),
    CREDITS: {
      authors: extract(credits, 'LPC_AUTHORS'),
      tools: extract(credits, 'LPC_TOOL_CREDITS'),
      spears: extract(credits, 'LPC_SPEAR_CREDITS'),
    },
    LPC: {
      POOL_ITEMS: extract(pool, 'POOL_ITEMS'),
      // FE는 /lpc/parts(루트) — 이 repo는 상대 경로 assets/lpc/parts
      PARTS_BASE: 'assets/lpc/parts/',
      PALETTES: extract(palettes, 'PALETTES'),
      SOURCE_PALETTES: extract(palettes, 'SOURCE_PALETTES'),
      RULES: rules,
    },
  };
  const json = JSON.stringify(data);
  const body = `// 자동 생성 — tools/gen-web-data.mjs (수동 편집 금지). 원천: ../terrafolk (gen ${new Date().toISOString()})
// i18n 사전·이벤트 템플릿·어휘·LPC 풀 — FE 정본과 동일 데이터.
window.TF_DATA = ${json};
`;
  writeFileSync(OUT, body);

  if (!process.argv.includes('--skip-lpc')) {
    const src = join(FE, 'public', 'lpc', 'parts');
    if (!existsSync(src)) throw new Error('LPC 부품 원천 없음: ' + src);
    rmSync(join(LPC_DST, 'parts'), { recursive: true, force: true });
    mkdirSync(join(LPC_DST, 'parts'), { recursive: true });
    cpSync(src, join(LPC_DST, 'parts'), {
      recursive: true,
      filter: (p) => statSync(p).isDirectory() || p.endsWith('.png'),
    });
    // 발명품 아이콘 템플릿(game-icons SVG) — tf-look renderItemIcon이 사용
    const icons = join(FE, 'public', 'assets', 'icons');
    if (existsSync(icons)) {
      rmSync(ICONS_DST, { recursive: true, force: true });
      mkdirSync(ICONS_DST, { recursive: true });
      cpSync(icons, ICONS_DST, {
        recursive: true,
        filter: (p) => statSync(p).isDirectory() || p.endsWith('.svg'),
      });
    }
  }
  return body.length;
})();

console.log(`[gen-web-data] tf-data.js ${(gen / 1024).toFixed(0)}KB`);
