---
name: nsynergy-build
description: "nsynergy Rust 워크스페이스 빌드 및 테스트 스킬. cargo build, cargo test, npx vite build 실행. 빌드 에러 해결, 테스트 실패 분석. Rust 코드를 수정했거나 빌드/테스트가 필요할 때 사용."
---

# nsynergy Build & Test

nsynergy Rust 워크스페이스의 빌드와 테스트를 수행한다.

## 환경 준비

Rust 명령 실행 전 환경을 로드한다:

```bash
source "$HOME/.cargo/env"
```

## 빌드

### 데스크톱 빌드

```bash
cd /Users/obst/personal_project/nsynergy
source "$HOME/.cargo/env" && cargo build 2>&1
```

빌드 전 프론트엔드가 필요한 경우 (Tauri generate_context! 매크로):

```bash
cd /Users/obst/personal_project/nsynergy
npm run build  # npx vite build
```

### 전체 테스트

```bash
source "$HOME/.cargo/env" && cargo test --workspace 2>&1
```

### 개별 크레이트 테스트

```bash
source "$HOME/.cargo/env" && cargo test -p nsynergy-core 2>&1
source "$HOME/.cargo/env" && cargo test -p nsynergy-net 2>&1
source "$HOME/.cargo/env" && cargo test -p nsynergy-server 2>&1
source "$HOME/.cargo/env" && cargo test -p nsynergy-client 2>&1
```

## 빌드 에러 대응

### crates.io SSL 타임아웃

```bash
CARGO_REGISTRIES_CRATES_IO_PROTOCOL=git cargo update
cargo build
```

### Tauri frontendDist 에러

generate_context!() 매크로가 frontendDist를 컴파일 타임에 요구한다. 반드시 `npm run build` (vite build)를 먼저 실행한다.

### edition 2024 이슈

- extern 블록: `unsafe extern "C"` 사용 (not `extern "C"`)
- 새로운 키워드 예약어 충돌 확인

## 테스트 작성 규칙

- port 0으로 OS 임시 포트 할당 (테스트 간 충돌 방지)
- tempfile 크레이트로 임시 디렉토리 (설정 파일 테스트)
- rdev::listen은 블로킹이므로 테스트에서 직접 호출하지 않음
- enigo::Enigo는 !Send이므로 Send 바운드 테스트 금지
