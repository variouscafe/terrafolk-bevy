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

## 배포

- GitHub Pages: `deploy-github-page` 워크플로(수동 디스패치) → trunk 릴리스 빌드 →
  wasm-opt → gh-pages 브랜치. Pages 설정이 gh-pages 소스를 가리키면
  `https://<user>.github.io/terrafolk-rust/` 에서 서비스된다.
- 태그(`v0.1.0`) 푸시 → 전 플랫폼 릴리스(모바일은 dev 빌드).

## 로드맵

1. Phase 2 — Colyseus 연결: 자체 스키마 디코더 + 관전 티켓 REST → 실 월드 렌더
2. UI — DOM 오버레이(기존 React 채팅·정보탭 재사용) 브리지
3. 안드로이드 — 템플릿 mobile/ 래퍼(xbuild) + 터치 카메라
