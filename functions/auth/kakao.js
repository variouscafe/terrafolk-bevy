// GET /auth/kakao — start Kakao OAuth from the frontend origin (me.suseona.com).
// Builds the consent URL with redirect_uri = <origin>/auth/kakao/callback and a CSRF state cookie.
// state 는 자기검증 형식(<rnd>.<ts>.<next>.<HMAC sig>) — 쿠키가 유실되는 환경(인앱 브라우저·
// 앱 핸드오프)에서도 콜백이 서명+신선도로 검증 가능. OAUTH_STATE_SECRET 미설정 시 레거시 형식.
// No scope/prompt params — consent items are governed by the Kakao console 동의항목.
const KAKAO_AUTH_URL = 'https://kauth.kakao.com/oauth/authorize';
// 공개 OAuth 클라이언트 ID(REST API 키, 민감 아님). Pages 환경변수 바인딩 지연 폴백용.
const KAKAO_CLIENT_ID_DEFAULT = 'fbbed3a4f6533358409138d823748f17';

export async function onRequestGet(context) {
  const { request, env } = context;
  const clientId = env.KAKAO_CLIENT_ID || KAKAO_CLIENT_ID_DEFAULT;
  if (!clientId) return new Response('Kakao login is not configured', { status: 500 });

  const origin = new URL(request.url).origin;
  const redirectUri = `${origin}/auth/kakao/callback`;
  // ?next=<path> — 로그인 완료 후 돌아갈 SPA 경로(초대 링크 등). 상대경로만 허용.
  const nextParam = new URL(request.url).searchParams.get('next') ?? '';
  const next = nextParam.startsWith('/') && !nextParam.startsWith('//') ? nextParam : '';
  // next 를 state 자체에 실어 보냄 — 카카오가 state 를 변경 없이 돌려주므로 모바일 앱 핸드오프·
  // 인앱 브라우저 등 쿠키가 유실되는 환경에서도 복귀 경로가 보존됨. 쿠키(oauth_state)는 CSRF 검증용.
  const rnd = (crypto.randomUUID() + crypto.randomUUID()).replace(/-/g, '');
  const state = await buildState(rnd, next, env.OAUTH_STATE_SECRET);
  const params = new URLSearchParams({
    client_id: clientId,
    redirect_uri: redirectUri,
    response_type: 'code',
    state,
  });
  const secure = origin.startsWith('https://');
  // Max-Age 1시간 — 카카오 첫 회원가입(이메일 인증 등)이 5분을 넘는 경우가 있어 여유 확보.
  const cookie = `oauth_state=${state}; Path=/auth; HttpOnly; SameSite=Lax; Max-Age=3600${secure ? '; Secure' : ''}`;
  const headers = { Location: `${KAKAO_AUTH_URL}?${params.toString()}`, 'Set-Cookie': cookie };
  return new Response(null, { status: 302, headers });
}

/** base64url(인코딩된 ASCII) — state 세그먼트용. */
function b64url(s) {
  return btoa(encodeURIComponent(s)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/** state 조립: secret 있으면 '<rnd>.<ts>.<nextSeg|->.<sig>'(자기검증), 없으면 레거시 형식. */
async function buildState(rnd, next, secret) {
  const nextSeg = next ? b64url(next) : '-';
  if (!secret) return next ? `${rnd}.${nextSeg}` : rnd;
  const ts = Math.floor(Date.now() / 1000);
  return `${rnd}.${ts}.${nextSeg}.${await hmacHex(`${rnd}.${ts}.${nextSeg}`, secret)}`;
}

/** HMAC-SHA256(msg, secret) → hex. Pages Functions(Web Crypto)용. */
async function hmacHex(msg, secret) {
  const key = await crypto.subtle.importKey(
    'raw', new TextEncoder().encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign'],
  );
  const sig = await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(msg));
  return [...new Uint8Array(sig)].map((b) => b.toString(16).padStart(2, '0')).join('');
}
