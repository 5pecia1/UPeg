# Security Policy

**English summary:** Please report suspected vulnerabilities privately
through [GitHub Security Advisories](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability)
on `5pecia1/UPeg` ("Security" tab -> "Report a vulnerability"), not through a
public issue. There is no bug bounty. We aim to give an initial response
within 7 days. In scope: the HTTP/MCP server surfaces, the Chrome extension,
and tool execution. Only the latest published release is supported with
security fixes.

## 취약점 보고 방법

보안 취약점으로 의심되는 문제를 발견하셨다면, **공개 이슈(GitHub Issues)로 올리지 마시고**
GitHub의 [Private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability)
기능을 통해 비공개로 신고해 주세요.

1. `5pecia1/UPeg` 저장소의 "Security" 탭으로 이동합니다.
2. "Report a vulnerability" 버튼을 클릭합니다.
3. 문제를 재현할 수 있는 최소한의 절차, 영향 범위, 가능하다면 영향을 받는 버전을 함께 적어
   주세요.

이 절차를 이용할 수 없는 경우, 저장소 관리자(메인테이너)의 GitHub 프로필에 공개된 연락 수단으로
직접 문의해 주세요. 공개 채널(이슈, 토론, PR)에 취약점 상세 내용을 올리지 말아 주시기 바랍니다.

## 응답 목표

- 신고 접수에 대한 최초 응답: 영업일 기준 **7일 이내**를 목표로 합니다.
- 별도의 버그 바운티(금전적 보상) 프로그램은 운영하지 않습니다.

## 신고 대상 범위 (Scope)

다음 영역에서 발견된 보안 문제를 신고 대상으로 환영합니다.

- HTTP 서버 및 MCP(Model Context Protocol) 서버 표면(surface)
- Chrome 확장 프로그램(`chrome-ext/`)
- 툴(tool) 실행 경로 (예: 임의 코드 실행, 샌드박스 탈출, 경로 탈출 등)

빌드 스크립트나 CI 설정 자체의 취약점도 관심 대상이지만, 위 세 영역이 우선순위입니다.

## 지원 버전

보안 수정은 **최신 배포 릴리스(latest release)**에 대해서만 제공됩니다. 이전 버전에 대한
백포트(backport)는 보장하지 않습니다. 가능하면 항상 최신 릴리스를 사용해 주세요.
