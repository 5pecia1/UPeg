---
type: Domain Contract
title: I/O 타입 시스템
description: 모든 surface가 공유하는 닫힌 입력/출력 타입 집합과 CLI 직렬화 규칙.
tags: [architecture, domain, io]
status: stable
---

# 원칙

모든 Tool의 입력과 출력은 닫힌 타입 집합 중 하나로 선언되어야 한다.

- 7개 surface 전체에서 일관되게 렌더링 가능해야 한다.
- CLI stdout으로 텍스트 직렬화 가능해야 한다.
- 렌더링은 extensible 레이어다 — 새 타입을 추가하면 렌더링 계층만 갱신한다.

# 타입

`IoType` (Rust) / `inputSchema`·`outputSchema` 내부 (JSON) / `input_spec`·`output_spec` 내부 (TOML).

| 타입 | 설명 | CLI 표현 |
|---|---|---|
| `String` | 텍스트 | 그대로 |
| `Number` / `Integer` | 수치 | 그대로 |
| `Boolean` | 참/거짓 | `true` / `false` |
| `Options` | 단일 선택 | 선택된 값 |
| `MultiOptions` | 다중 선택 | 콤마 구분 |
| `Markdown` | 마크다운 텍스트 | 그대로 |
| `Json` | 구조화 데이터 | pretty-printed JSON |
| `Datetime` | 날짜+시간 | ISO 8601 |
| `Url` | URL | 그대로 |
| `FilePath` | 파일 경로 | 절대 경로 |
| `File` | 파일 본문 (단일 변형, `is_dir`로 디렉터리 구분) | `--out <PATH>`로 기록 |
| `EmbeddedView` | 출력 전용 — 외부 URL을 Pin 본문에 임베드 | 비-GUI surface에서는 URL |

# 인라인 제약

선언 지점에서 붙일 수 있는 제약이며 폼 렌더링과 검증에 함께 쓰인다.

| 변형 | 인라인 파라미터 |
|---|---|
| `Number`, `Integer` | `min=`, `max=`, `default=` |
| `String` | `regex=`, `placeholder=`, `default=` |
| `Options`, `MultiOptions` | 첫 인수 자리에 `["value1", "value2", ...]` |
| 그 외 | 없음 |

# 폴백

닫힌 집합에 없는 타입은 지원하지 않는다. 그런 도구는 Inline 핀으로 표시되지 않고 iframe
또는 바로가기로 폴백한다.

`EmbeddedView`처럼 GUI에서만 의미 있는 출력을 가진 도구는 `surfaces`로 GUI surface에
명시적으로 제한한다.
