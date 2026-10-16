# Terrafolk Rust

스스로 생각하고 살아가는 AI 주민들의 원시 세계 시뮬레이션 게임 Terrafolk의 Rust/Bevy 클라이언트.

**북극성 — "AI 캐릭터가 주도하는 시뮬레이션 게임 1위"**: 이 클라이언트의 모든 작업(그래픽·UX·성능)은 AI 주민의 생명감과 관전 몰입을 최우선으로 판단한다. 프로토타입 문구·임시 룩은 정식 제품 기준으로 다듬는다(예: 페이지 타이틀 "A Living World of AI Villagers").

## 역할 분담

- **이 repo = client 담당**: Bevy 웹 클라이언트 — **웹버전 배포를 타겟**으로 개발한다 (배포 https://terrafolk-rust.pages.dev).
- **backend·룸·브레인은 `../terrafolk` repo 담당** — API·권위적 월드 틱·LLM 호출은 그쪽에서 구현하며, 데이터 계약도 그쪽 문서(`../terrafolk/docs/data-contract.md`, §17.82) 기준을 따른다.
- 작업 중 워크스페이스의 repo를 오가면 **현재 repo의 역할에 맞게** 구현할 것.

## 개발·배포 절차

실행·구조·배포 상세는 `README.md`를 참조할 것.
