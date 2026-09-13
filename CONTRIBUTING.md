# Contributing to UPeg

This repository is a one-way source mirror. Contributions are welcome through
fork pull requests. Accepted changes are reviewed and imported by the maintainer
into the source repository, then included in a subsequent public snapshot.
The original pull request is closed with a link to the published commit.

Public snapshot commits use a bot author and committer. Contributor attribution
remains in the original pull request and applicable source copyright notices;
private commit messages and author trailers are not copied into snapshot commits.

## 기여 절차

1. Fork에서 변경을 작성하고 이 저장소로 PR을 보냅니다.
2. 검토가 끝나면 관리자가 변경을 소스 저장소에 반영하고 CI를 실행합니다.
3. 검증된 공개 스냅샷이 게시되면 원래 PR에 해당 커밋을 연결하고 PR을 닫습니다.
   공개 main에 직접 merge하지 않으므로 PR은 Closed로 표시됩니다.

커밋은 [Developer Certificate of Origin](https://developercertificate.org/)에
따라 `git commit -s`로 서명합니다. 기여 출처와 필요한 저작권 고지를 보존하세요.

## 검증

공개 CI와 로컬은 `scripts/verify_public.sh`를 공유합니다. Rust 1.92.0,
Flutter 3.44.0, cargo-deny 0.18.2, just 1.51.0과 WASM 빌드 도구를 사용합니다.
네이티브 패키지 의존성과 설치 절차는 `.github/actions/verify/action.yml`에 있습니다.

```bash
just verify
# 변경 영역을 먼저 확인할 때:
bash scripts/verify_public.sh rust
bash scripts/verify_public.sh wasm
bash scripts/verify_public.sh flutter
bash scripts/verify_public.sh licenses
```

Rust lane은 workspace 빌드·fmt·clippy·테스트, WASM lane은 대상별 clippy,
Flutter lane은 잠금 파일·analyze·테스트·Linux/web 빌드를 확인합니다.
라이선스 검사는 배포물의 실제 고지 동봉 검토와 함께 사용합니다.

테스트 함수 이름은 자연스러운 한국어로 작성합니다. 전문 용어의 영어는 허용합니다.
기능의 책임을 나누고 복잡도를 낮추며, Rust 타입으로 불가능한 상태를 표현하지 않도록 합니다.

## 라이선스

UPeg 자체 코드는 [Apache-2.0](LICENSE)입니다. `upeg-plugin-api`와
`upeg-plugin-macros`는 해당 디렉터리의 MIT 또는 Apache-2.0을 선택할 수 있습니다.
기여는 변경하는 코드와 같은 라이선스로 받습니다. 서드파티 코드·데이터·폰트에는
각자의 조건이 적용되며 [NOTICE](NOTICE)와 원문 고지를 보존해야 합니다.
