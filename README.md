# Terrafolk Bevy — 웹 클라이언트 (2.5D 아이소)

Terrafolk의 Rust/Bevy 클라이언트. **웹 우선**(wasm+trunk), 안드로이드·iOS·데스크톱
타깃은 템플릿 인프라로 보존. Phase 1 = 그래픽 렌더 프로토타입.

- 베이스: [NiklasEi/bevy_game_template](https://github.com/NiklasEi/bevy_game_template) (CC0) —
  CI/CD·trunk·아이콘·모바일 래퍼 인프라 사용, 게임 코드는 전면 교체
- 데이터 계약: `../terrafolk/docs/data-contract.md`(§17.82) — Phase 2에서 Colyseus
  어댑터(자체 스키마 디코더)가 `WorldData` 리소스를 대체한다
- 아트: `../terrafolk-godot/assets`와 동일 원천(팔레트 잠금·감사 통과 PNG) —
  주민 시트는 프레임별 PNG로 분할해 사용

## 실행

```bash
cargo run                    # 네이티브
trunk serve                  # 웹(:8080, 핫리로드)
```

조작: `WASD/화살표` 팬 · 휠 줌 · `T` 시간 배속(밤 확인).

## 구조

| 모듈 | 역할 |
|---|---|
| `src/world/data.rs` | 데모 월드(시드 결정론 PRNG·랩 값노이즈). GID 어휘 = client-meta |
| `src/world/terrain.rs` | 그리드 → 단일 Mesh(상판+단위단계 절벽, 아틀라스 UV) |
| `src/world/billboard.rs` | 고정 절대각 빌보드(스태틱 정렬) + 모닥불 애님·포인트 라이트 |
| `src/world/daynight.rs` | 게임 시각 순수 함수 → 태양·앰비언트·하늘색 |
| `src/world/water.rs` + `assets/shaders/water.wgsl` | 절차 수면(해시 노이즈·반짝임·프레넬) |
| `src/world/villager.rs` | 4방향×4프레임 보행·배회(W계 미러) |
| `src/world/camera_rig.rs` | 45° 저FOV 아이소 카메라 |
| `src/world/pick.rs` | 주민 클릭/탭 픽킹 + 발밑 선택 링 + JS 브리지(`tf.selectAgent`⇄`tf.onPickAgent`, 유물 `tf.uiPick`) |
| `src/world/overlay.rs` | 월드 앵커 투영 피드(`tf.onOverlay` — 말풍선·라벨·배지·미니맵 공용) + 미니맵 점프 `tf.panTo` + 자원 호버 `tf.onHoverRes` |
| `src/world/bld.rs` | 건축 발판(§17.75) — 다중 타일 말뚝·밧줄·공사 링 펄스·진행 바·재료 더미, 완공 창불(§17.78) |
| `src/world/propsfx.rs` | 유물 부유+자전·광맥 점멸 스파크(Bloom 점멸) |
| `src/world/cam_follow.rs` | 선택 주민 카메라 추적(FE camera-follow) — 수동 팬 입력 시 해제 |

## DOM UI 오버레이(☰ 메뉴·팝업)

FE(React) worldsim 화면의 바닐라 JS 포트 — 빌드 도구 없이 trunk가 정적 복사.

- `assets/web/tf-ui.js` — 상단 바(시계·DayDial·날씨·인원·☰) + 중앙 팝업 6종
  (캐릭터·자원·문명·기록·설정·계정) + 크레딧·외형 편집 모달. 선택 상세
  (욕구·소지품·생활기록·관계·기억)는 공개 REST, 참여·외형 저장은 인증 REST.
- `assets/web/tf-i18n.js` — 4개국어 사전·이벤트 템플릿 렌더(ev.*/goal.*)·태양력
  달력·표시 MT(`/api/worldsim/translate` 디바운스 배치). FE i18n.ts 포트.
- `assets/web/tf-look.js` — LPC 외형 합성(uuid 결정적 부품 선택·팔레트 리컬러·
  walk 시트 캔버스) + 발명 아이콘 픽셀화. 부품 PNG: `assets/lpc/parts`.
- `assets/web/tf-data.js` — **자동 생성**(수동 편집 금지): `node tools/gen-web-data.mjs`
  가 `../terrafolk` 정본(i18n 사전·vocab·이벤트 템플릿·LPC 풀·크레딧)에서 생성.
  FE 사전이 바뀌면 재실행(`--skip-lpc`로 PNG 복사 생략 가능).
- `src/net/apply.rs` — 자원 동기 확장: 동물 배위 보간(RoamTo, §17.64)·
  나무 성장 4단계(maturity)·건축 발판(w×h·stage·progress, §17.75).
- `assets/web/worldsim.js` — 룸 접속(colyseus.js) + OAuth 인증(fragment 캡처·
  rt 갱신) + 스냅숏 상세(needs/goal/inventory/inventions)을 Rust sink와
  TF_UI.feed로 이중 분배. 백오프 재접속·탭 복귀 즉시 재시도.
- `assets/web/ui.js` — 채팅 바·대화 로그 독(선택 시 노출, §17.55 로컬 에코) +
  §17.76 우측 세로 정보 레일(상태·소지품·기억·생활기록·관계 — 소지품 무게 바
  §17.81 포함, TF_UI 상태를 500ms 폴로 표현).
- `assets/web/tf-world-ui.js` — 월드 앵커 오버레이(FE scene/agents.ts 포트):
  이름 라벨(저줌 숨김)·말풍선(§17.44)·활동 진행 링+아이콘·수면 zZz·욕구 ❗·
  하이라이트 토스트(feed.event wrap→highlightOf)·미니맵(클릭 점프)·
  계절 틴트·번개 섬꼉(storm)·사망 🕯 연출·자원 툴팁(§17.78 누구 집·묘지
  비문·유물 설명 — 마우스 호버/터치 탭 고정 2.5s).
- `functions/auth/{google,kakao}*` — OAuth Pages 함수(FE repo에서 복사).
  배포 OAuth 콘솔에 `terrafolk-rust.pages.dev/auth/*/callback` 리다이렉트
  URI 등록이 필요하다.

조작 추가: 주민 클릭/탭 선택(링+추적) · 목록 클릭 카메라 추적 · 유물 클릭 카드.

## 배포

- Cloudflare Pages(운영): `trunk build --release` 후
  `wasm-opt --enable-bulk-memory-opt --enable-nontrapping-float-to-int -Os --strip-debug --strip-producers`
  로 최적화(≈25.6MB) → `dist/index.html`의 wasm `integrity="sha384-…"` 갱신 →
  `npx wrangler pages deploy dist --commit-dirty=true`.
  **주의 — `-Oz`는 이 빌드를 잘못 컴파일해 백지 월드가 된다**(앱은 동작, 렌더 안 됨).
  `-Os`+strip 조합만 검증 완료. 배포 후 부트 E2E로 렌더 확인 필수.
- GitHub Pages: `deploy-github-page` 워크플로(수동 디스패치) → trunk 릴리스 빌드 →
  wasm-opt → gh-pages 브랜치. Pages 설정이 gh-pages 소스를 가리키면
  `https://<user>.github.io/terrafolk-rust/` 에서 서비스된다.
- 태그(`v0.1.0`) 푸시 → 전 플랫폼 릴리스(모바일은 dev 빌드).

## 로드맵

1. Phase 2 — Colyseus 연결: 자체 스키마 디코더 + 관전 티켓 REST → 실 월드 렌더 ✅
2. UI — DOM 오버레이(기존 React 채팅·정보탭 재사용) 브리지 ✅(☰ 메뉴·팝업·채팅 바)
3. 안드로이드 — 템플릿 mobile/ 래퍼(xbuild) + 터치 카메라
