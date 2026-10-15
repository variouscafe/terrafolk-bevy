// GET /auth/google/callback — Google redirects here with ?code=&state=.
// Verify the CSRF state cookie, then ask the auth-worker to exchange the code + issue tokens,
// and finally redirect to the SPA root (or the next path carried inside state) with the tokens
// in the URL fragment.
export async function onRequestGet(context) {
  const { request, env } = context;
  const url = new URL(request.url);
  const code = url.searchParams.get('code');
  const state = url.searchParams.get('state');
  const cookieState = getCookie(request.headers.get('cookie'), 'oauth_state');
  // 1차: 쿠키 완전 일치(기존 CSRF 경로). 2차: 쿠키 유실(인앱 브라우저·앱 핸드오프에서 시작/복귀가
  // 서로 다른 브라우저 컨텍스트에서 일어난 경우) 시 자기검증 state — HMAC 서명 + 1시간 신선도로 통과.
  const stateOk =
    (cookieState && state === cookieState) || (await verifySignedState(state, env.OAUTH_STATE_SECRET));
  if (!code || !state || !stateOk) {
    return new Response('Invalid OAuth state', { status: 400 });
  }

  const origin = new URL(request.url).origin;
  const redirectUri = `${origin}/auth/google/callback`;
  // auth-worker origin(민감 아님). Pages 환경변수 바인딩 지연 폴백용.
  const apiUrl = (env.AUTH_API_URL || 'https://suseona-api.suseona.com').replace(/\/$/, '');
  if (!apiUrl) return new Response('AUTH_API_URL not configured', { status: 500 });

  // next 는 state 세그먼트('<rnd>.<base64url(next)')에서 복원 — 쿠키가 유실돨 수 있는
  // 모바일 앱 핸드오프·인앱 브라우저 환경에서도 복귀 경로가 보존됨.
  const next = decodeNextFromState(state);

  let res;
  try {
    res = await fetch(`${apiUrl}/auth/google/exchange`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ code, redirectUri }),
    });
  } catch {
    return new Response('Google login service unavailable', { status: 502 });
  }
  if (!res.ok) return new Response('Google login failed', { status: 400 });

  const tokens = await res.json();
  const loc = next ? new URL(origin + next) : new URL(origin);
  // provider — 프론트(main.tsx)가 umami login 이벤트에 기록.
  loc.hash = `at=${encodeURIComponent(tokens.accessToken)}&rt=${encodeURIComponent(tokens.refreshToken)}&provider=google`;
  const secure = origin.startsWith('https://');
  return new Response(null, {
    status: 302,
    headers: {
      Location: loc.toString(),
      'Set-Cookie': `oauth_state=; Path=/auth; HttpOnly; SameSite=Lax; Max-Age=0${secure ? '; Secure' : ''}`,
    },
  });
}

/** state에서 next 복원 — 신규 '<rnd>.<ts>.<nextSeg|->.<sig>', 레거시 '<rnd>.<b64url>'. 상대경로만. */
function decodeNextFromState(state) {
  const parts = state.split('.');
  const seg = parts.length === 4 ? parts[2] : parts.length === 2 ? parts[1] : '';
  if (!seg || seg === '-') return '';
  try {
    const b = seg.replace(/-/g, '+').replace(/_/g, '/');
    const decoded = decodeURIComponent(atob(b + '='.repeat((4 - (b.length % 4)) % 4)));
    return decoded.startsWith('/') && !decoded.startsWith('//') ? decoded : '';
  } catch {
    return '';
  }
}

/** 자기검증 state 검증: '<rnd>.<ts>.<nextSeg>.<sig>' — 서명 일치 + 발급 후 1시간 이내만 통과. */
async function verifySignedState(state, secret) {
  if (!secret || !state) return false;
  const parts = state.split('.');
  if (parts.length !== 4 || !/^\d+$/.test(parts[1])) return false;
  if (Math.abs(Math.floor(Date.now() / 1000) - Number(parts[1])) > 3600) return false;
  const expect = await hmacHex(`${parts[0]}.${parts[1]}.${parts[2]}`, secret);
  return timingSafeEqualHex(expect, parts[3]);
}

/** HMAC-SHA256(msg, secret) → hex. Pages Functions(Web Crypto)용. */
async function hmacHex(msg, secret) {
  const key = await crypto.subtle.importKey(
    'raw', new TextEncoder().encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign'],
  );
  const sig = await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(msg));
  return [...new Uint8Array(sig)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

/** 상수시간 hex 비교 — 서명 검증용(길이 불일치/빈 값은 즉시 false). */
function timingSafeEqualHex(a, b) {
  if (!a || !b || a.length !== b.length || !/^[0-9a-f]+$/.test(a) || !/^[0-9a-f]+$/.test(b)) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}

function getCookie(header, name) {
  if (!header) return undefined;
  for (const part of header.split(';')) {
    const i = part.indexOf('=');
    const k = i >= 0 ? part.slice(0, i).trim() : part.trim();
    if (k === name) return decodeURIComponent(part.slice(i + 1).trim());
  }
  return undefined;
}
