# Contributing to UPeg

Contributions are welcome through ordinary GitHub pull requests against
this repository.

## How contributions land

UPeg is developed in a private source repository; this repository is its
public mirror. Two pull-request types appear here:

- **Your PR** (a contribution): reviewed here, then **imported into the
  source repository** and published back through a later export (below).
- **Export PRs** (publishing, maintainer-only): the source repository's
  export workflow pushes an export branch and opens a PR against `main`;
  that PR merges like a normal PR after its checks pass.

In detail, for a contribution:

1. Open a pull request against this repository — fork, branch, PR. CI runs
   on it like any other PR.
2. A maintainer reviews the change. When it is accepted, the maintainer
   **imports it into the source repository** — path-by-path, since the
   source tree has a different layout — and runs full verification there.
   Importing is a manual maintainer step; a contributor PR is never merged
   into `main` directly and is never auto-imported.
3. The maintainer then publishes an update from the source repository's
   Actions tab with **Publish UPeg**. This input-free workflow verifies the
   export, pushes an export branch here, and opens an ordinary PR that merges
   into `main` normally.
4. After the export PR containing your change merges, the maintainer closes
   your original PR with a link to that export PR. Your PR shows as
   **Closed**, not Merged — that is the expected outcome for contributor PRs,
   not a rejection.

The development history itself is not mirrored — each export carries one
squashed, verified commit.

The exported commit is authored and committed by the publication bot and
records its private source revision in its message. Your public contributor
PR retains its authorship record. Sign your commits with
`git commit -s` — the
[Developer Certificate of Origin](https://developercertificate.org/) —
and keep any copyright notices your change requires.

To ease review and import:

- Keep PRs focused; a mirror rewards small, self-contained changes.
- Don't rely on source-repository-internal paths, filenames, or test
  names in your description — describe behavior and intent.

## Verifying locally

Local verification and public CI share `scripts/verify_public.sh`
(Rust 1.92.0, Flutter 3.44.0, Node.js 24, FRB codegen 2.12.0,
cargo-expand 1.0.126, cargo-deny 0.18.9, just 1.51.0, plus the WASM
toolchain). Native package dependencies and install steps are in
`.github/actions/verify/action.yml`.

```bash
just verify
# or one lane at a time:
bash scripts/verify_public.sh rust       # needs a Chrome/Chromium binary for E2E
bash scripts/verify_public.sh wasm
bash scripts/verify_public.sh flutter
bash scripts/verify_public.sh licenses
```

The `rust` lane covers packaging shell tests, workspace build, fmt, clippy,
tests, interface inventory, toolkit schema, and Chrome extension Node tests;
`wasm` runs per-target clippy; `flutter` checks the lockfile, FRB binding
drift, analyze, tests, and Linux/web builds; `licenses` runs cargo-deny.
`just check` is the cheaper pre-commit gate. See the
[development guide](https://github.com/5pecia1/UPeg/blob/main/docs/guides/development.md).

## Releases

The `release` workflow has two explicit modes. `native` builds and publishes
from the selected public commit after all four public verification lanes pass.
It requires `PUBLIC_RELEASE_APP_ID` and `PUBLIC_RELEASE_APP_PRIVATE_KEY` for
an App installed only on this repository with `contents: write`; the default
workflow token cannot create protected tags. That designated App must also be
an allowed bypass actor for the `oss-tags-app-only` tag ruleset. `mirror` verifies the exact bytes
and manifest already attached to a public draft release, runs the same four
lanes against `public_sha`, and only then publishes that draft. It has no
credential for, or dependency on, the private source repository.

## Conventions

- **Write test names in descriptive English** — a test name should say
  what it proves (e.g. `rejects_urlsafe_base64_in_file_wire`). This applies
  to tests you add or modify.
- Split responsibilities to keep complexity low, and use the type system
  so unrepresentable states stay unrepresentable.
- Hand-written Rust and Dart files stay within the 1000-line file-size
  budget.
- Documentation and code comments are in English; user-visible vocabulary
  follows `docs/LEXICON.md`.
- `docs/TOOL_MANIFEST.md` and `fixtures/` baselines are generated — don't
  hand-edit; regenerate with the `just` recipes in the development guide.

## License

UPeg's own code is [Apache-2.0](https://github.com/5pecia1/UPeg/blob/main/LICENSE). `upeg-plugin-api` and
`upeg-plugin-macros` may use the MIT or Apache-2.0 license in their own
directories. Contributions are accepted under the same license as the
code they change. Third-party code, data, and fonts keep their own terms —
preserve [NOTICE](https://github.com/5pecia1/UPeg/blob/main/NOTICE) and the original notices.
