# Terrafolk Rust

스스로 생각하고 살아가는 AI 주민들의 원시 세계 시뮬레이션 게임 Terrafolk의 Rust/Bevy 클라이언트.

**목표 — "AI 시뮬레이션 게임 1위"**([`../terrafolk` 북극성과 동일](../terrafolk/CLAUDE.md)): 이 클라이언트의 모든 작업(그래픽·UX·성능)은 AI 주민의 생명감과 관전 몰입을 최우선으로 판단한다.

## 역할 분담

- **이 repo = client 담당**: Bevy 웹 클라이언트 — **웹버전 배포를 타겟**으로 개발한다 (배포 https://terrafolk-rust.pages.dev).
- **backend·룸·브레인은 `../terrafolk` repo 담당** — API·권위적 월드 틱·LLM 호출은 그쪽에서 구현하며, 데이터 계약도 그쪽 문서(`../terrafolk/docs/data-contract.md`, §17.82) 기준을 따른다.
- 작업 중 워크스페이스의 repo를 오가면 **현재 repo의 역할에 맞게** 구현할 것.

## 개발·배포 절차

실행·구조·배포 상세는 `README.md`를 참조할 것.

**PRD 테스트 URL**: 자동화 테스트(헤드리스·Playwright 등)는 https://terrafolk-rust.pages.dev 로 접속할 것. 커스텀 도메인(terrafolk.suseona.com)은 Cloudflare 봇 방어가 자동화 접속을 챌린지(403)로 차단한다 — 사람 브라우저는 통과, 자동 검증은 pages.dev 사용. 단 suseona.com 존의 API(tf-api)도 헤드리스 지문을 차단할 수 있어 라이브 백엔드 연동 E2E가 필요하면 headed 모드(`headless: false` + `--disable-blink-features=AutomationControlled` + 실제 UA)로 실행할 것.
