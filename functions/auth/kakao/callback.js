// GET /auth/kakao/callback — Kakao redirects here with ?code=&state=.
// Verify the CSRF state cookie, then ask the auth-worker to exchange the code + issue tokens,
// and finally redirect to the SPA root (or the next path carried inside state) with the tokens
// in the URL fragment.
// Kakao-specific: when the account provides no usable email (declined consent / phone-registered),
// the worker returns error code kakao_email_unavailable — surfaced as a friendly HTML page
// localized by Accept-Language (ko/zh/ja, else English — docs §17.51).
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
  const redirectUri = `${origin}/auth/kakao/callback`;
  // auth-worker origin(민감 아님). Pages 환경변수 바인딩 지연 폴백용.
  const apiUrl = (env.AUTH_API_URL || 'https://suseona-api.suseona.com').replace(/\/$/, '');
  if (!apiUrl) return new Response('AUTH_API_URL not configured', { status: 500 });

  // next 는 state 세그먼트에서 복원 — 쿠키가 유실돨 수 있는 모바일 앱 핸드오프·인앱 브라우저
  // 환경에서도 복귀 경로가 보존됨.
  const next = decodeNextFromState(state);

  let res;
  try {
    res = await fetch(`${apiUrl}/auth/kakao/exchange`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ code, redirectUri }),
    });
  } catch {
    return new Response('Kakao login service unavailable', { status: 502 });
  }
  if (!res.ok) {
    const retryHref = next || '/login';
    let errorCode = '';
    try {
      errorCode = ((await res.json().catch(() => null)) || {}).error?.code ?? '';
    } catch {}
    if (errorCode === 'kakao_email_unavailable') {
      return htmlError(
        request,
        ['카카오 로그인을 계속할 수 없어요', 'Kakao login cannot continue', 'Kakao 登录无法继续', 'Kakao ログインを続けられません'],
        [
          '카카오 계정에서 이메일 정보를 확인할 수 없습니다. 카카오 로그인 시 이메일 제공에 동의했는지 확인하고 다시 시도해 주세요.',
          'We could not read the email address on your Kakao account. Check that you agreed to share your email when logging in with Kakao, then try again.',
          '无法确认 Kakao 账号的邮箱信息。请检查使用 Kakao 登录时是否同意提供邮箱，然后重试。',
          'Kakao アカウントのメールアドレスを確認できません。Kakao ログイン時にメール提供に同意したか確認して、もう一度お試しください。',
        ],
        retryHref,
      );
    }
    return htmlError(
      request,
      ['카카오 로그인에 실패했어요', 'Kakao login failed', 'Kakao 登录失败', 'Kakao ログインに失敗しました'],
      ['잠시 후 다시 시도해 주세요.', 'Please try again in a moment.', '请稍后重试。', 'しばらくしてからもう一度お試しください。'],
      retryHref,
    );
  }

  const tokens = await res.json();
  const loc = next ? new URL(origin + next) : new URL(origin);
  // provider — 프론트(main.tsx)가 umami login 이벤트에 기록.
  loc.hash = `at=${encodeURIComponent(tokens.accessToken)}&rt=${encodeURIComponent(tokens.refreshToken)}&provider=kakao`;
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

/** Minimal error page (charset explicit — plain text can mojibake without it).
 *  §17.51 — Accept-Language 감지 4개 국어([ko,en,zh,ja] 튜플), 정본은 en. */
function htmlError(request, titles, bodies, retryHref) {
  const al = (request.headers.get('accept-language') || '').toLowerCase();
  const i = al.startsWith('ko') ? 0 : al.startsWith('zh') ? 2 : al.startsWith('ja') ? 3 : 1;
  const lang = ['ko', 'en', 'zh', 'ja'][i];
  const back = [
    '로그인으로 돌아가기',
    'Back to login',
    '返回登录',
    'ログインに戻る',
  ][i];
  const title = titles[i] ?? titles[1];
  const body = bodies[i] ?? bodies[1];
  const html = `<!doctype html><html lang="${lang}"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>${title}</title>
<style>body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0b1020;color:#e5e7eb;font:15px/1.6 -apple-system,BlinkMacSystemFont,'Malgun Gothic','Apple SD Gothic Neo',sans-serif}
.card{max-width:420px;padding:40px 32px;text-align:center}h1{font-size:19px;margin:0 0 12px}p{margin:0 0 24px;color:#9ca3af}
a{display:inline-block;padding:10px 22px;border-radius:10px;background:#fee500;color:#191919;font-weight:600;text-decoration:none}</style>
<div class="card"><h1>${title}</h1><p>${body}</p><a href="${retryHref}">${back}</a></div>`;
  return new Response(html, { status: 400, headers: { 'content-type': 'text/html; charset=utf-8' } });
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
