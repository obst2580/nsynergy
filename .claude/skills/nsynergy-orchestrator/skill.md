---
name: nsynergy-orchestrator
description: "nsynergy 크로스 플랫폼 키보드/마우스 공유 앱의 전체 구현을 조율하는 오케스트레이터. 서버/클라이언트 루프 완성, React GUI 강화, Android 모바일 지원 추가를 병렬 에이전트 팀으로 수행. 'nsynergy 구현', '키보드 공유 앱 완성', '모바일 지원 추가' 등의 요청 시 사용."
---

# nsynergy Orchestrator

nsynergy 에이전트 팀을 조율하여 Mac/Windows/Android 간 키보드/마우스/클립보드 공유 앱을 완성한다.

## 실행 모드: 에이전트 팀

## 에이전트 구성

| 팀원 | 에이전트 타입 | 역할 | 출력 |
|------|-------------|------|------|
| rust-backend | rust-backend | 서버/클라이언트 루프, 입력 캡처/주입 | crates/\*\*/\*.rs |
| react-frontend | react-frontend | GUI 강화, 모바일 반응형 | src/\*\*/\*.tsx |
| android-engineer | android-engineer | Tauri 모바일, Kotlin 브릿지 | src-tauri/gen/android/, platform/ |
| integration-qa | integration-qa | 크로스 모듈 검증 | _workspace/qa_report_\*.md |

## 워크플로우

### Phase 1: 준비

1. 현재 프로젝트 상태 확인:
   - `cargo test --workspace` 로 기존 테스트 통과 확인
   - `npm run build` 로 프론트엔드 빌드 확인
2. `_workspace/` 디렉토리에 중간 산출물 저장 준비

### Phase 2: 팀 구성

1. 팀 생성:
   ```
   TeamCreate(
     team_name: "nsynergy-build",
     description: "nsynergy cross-platform app completion team"
   )
   ```

2. 팀원 스폰 (Agent 도구):
   - rust-backend (model: opus, agent_type: rust-backend)
   - react-frontend (model: opus, agent_type: react-frontend)
   - android-engineer (model: opus, agent_type: android-engineer)
   - integration-qa (model: opus, agent_type: integration-qa)

3. 작업 등록 (TaskCreate):

   **rust-backend 작업:**
   - T1: 서버 메인 루프 구현 (TCP/UDP 수신, 클라이언트 관리)
   - T2: 클라이언트 메인 루프 구현 (서버 연결, 이벤트 송수신)
   - T3: rdev 입력 캡처 실제 연동 (마우스/키보드 이벤트 캡처)
   - T4: enigo 입력 주입 실제 연동 (캡처된 이벤트 재생)
   - T5: mDNS 디스커버리 통합 (서버/클라이언트 루프에 연결)
   - T6: 플랫폼 추상화 레이어 (desktop/mobile cfg 분리)

   **react-frontend 작업:**
   - T7: 디바이스 연결/페어링 UI 구현
   - T8: 실시간 연결 상태 표시 (Tauri 이벤트 수신)
   - T9: Settings 화면 강화 (네트워크, 보안, 고급 설정)
   - T10: 모바일 반응형 디자인 적용

   **android-engineer 작업 (T1-T6 완료 후):**
   - T11: Tauri v2 Android 프로젝트 초기화
   - T12: 플랫폼 추상화 Android 구현 (접근성 서비스)
   - T13: Kotlin 플러그인 구현 (NsynergyPlugin)
   - T14: 터치-마우스 좌표 매핑

   **integration-qa 작업 (incremental):**
   - T15: Tauri 커맨드 ↔ React invoke 계약 검증
   - T16: 크레이트 간 API 경계면 검증
   - T17: cargo test/build 전체 통과 확인
   - T18: 최종 통합 검증 리포트

   **의존성:**
   - T5 depends on T1, T2
   - T6 depends on T3, T4
   - T11, T12, T13, T14 depend on T1, T2, T6
   - T15 depends on T7, T8, T9
   - T16 depends on T1, T2, T3, T4
   - T17 depends on T15, T16
   - T18 depends on T17

### Phase 3: 코어 구현 (병렬)

**실행 방식:** rust-backend + react-frontend 병렬, integration-qa incremental

팀원들이 공유 작업 목록에서 작업을 claim하고 독립적으로 수행한다.

**팀원 간 통신 규칙:**
- rust-backend는 Tauri 커맨드 시그니처 변경 시 react-frontend에게 SendMessage
- react-frontend는 새 invoke 호출이 필요하면 rust-backend에게 SendMessage
- integration-qa는 모듈 완성 알림을 받으면 즉시 해당 경계면 검증
- 리더는 진행 상황을 TaskGet으로 모니터링

**산출물 저장:**

| 팀원 | 출력 경로 |
|------|----------|
| rust-backend | crates/ 하위 Rust 소스 |
| react-frontend | src/ 하위 React 소스 |
| integration-qa | _workspace/qa_report_core.md |

### Phase 4: Android 구현

1. rust-backend의 T1-T6 완료 확인
2. android-engineer가 T11-T14 순차 수행
3. integration-qa가 Android 빌드 검증

**산출물:**

| 팀원 | 출력 경로 |
|------|----------|
| android-engineer | src-tauri/gen/android/, crates/nsynergy-core/src/platform/ |
| integration-qa | _workspace/qa_report_android.md |

### Phase 5: 최종 검증 및 정리

1. integration-qa가 T17, T18 수행 (전체 빌드/테스트 + 최종 리포트)
2. 모든 팀원의 작업 완료 대기
3. 최종 산출물 확인:
   - `cargo test --workspace` 전체 통과
   - `cargo build` 데스크톱 빌드 성공
   - React UI 정상 렌더링
4. 팀원들에게 종료 요청 (SendMessage shutdown_request)
5. 사용자에게 결과 요약 보고

## 데이터 흐름

```
[리더] → TeamCreate → [rust-backend] ←SendMessage→ [react-frontend]
                          │                              │
                          ↓ (T1-T6 완료)                 ↓ (T7-T10)
                    [android-engineer]              invoke 계약
                          │                              │
                          ↓ (T11-T14)                    ↓
                    Android 코드 ──────→ [integration-qa]
                                              │
                                              ↓
                                     qa_report_*.md
                                              │
                                              ↓
                                   [리더: 최종 보고]
```

## 에러 핸들링

| 상황 | 전략 |
|------|------|
| 팀원 1명 실패/중지 | 리더가 감지 → SendMessage로 상태 확인 → 재시작 |
| cargo build 실패 | integration-qa가 에러 분석 후 해당 팀원에게 수정 요청 |
| Tauri 커맨드 시그니처 충돌 | rust-backend + react-frontend 양쪽에 동기화 요청 |
| Android 빌드 실패 | android-engineer에게 Gradle 로그 분석 요청 |
| 타임아웃 | 현재까지 완성된 부분만으로 Phase 5 진행 |

## 테스트 시나리오

### 정상 흐름
1. Phase 1에서 기존 124개 테스트 통과 확인
2. Phase 2에서 4명 팀원 + 18개 작업 등록
3. Phase 3에서 rust-backend(T1-T6) + react-frontend(T7-T10) 병렬 수행
4. Phase 4에서 android-engineer(T11-T14) 순차 수행
5. Phase 5에서 전체 빌드/테스트 통과, 최종 리포트 생성

### 에러 흐름
1. Phase 3에서 react-frontend가 Tauri 커맨드 불일치 발견
2. integration-qa가 경계면 이슈 리포트
3. rust-backend에게 커맨드 시그니처 수정 요청
4. 수정 후 재빌드/테스트 통과
5. Phase 4로 정상 진행
