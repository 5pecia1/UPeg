---
title: Installation
description: "Build upeg from source — the CLI, the desktop app, the PWA, and the Chrome extension."
---

# Installation

There are no published binaries or installers yet — upeg is built from
source. Packaging recipes (`just package-*`, below) exist but produce
unsigned artifacts for local use; nothing is published for download.

## CLI, TUI, MCP, and HTTP — the `upeg` binary

One Rust toolchain covers all of these; they ship in a single binary. You
need:

- **Git**
- **A C toolchain** for the linker — `build-essential` on Debian/Ubuntu,
  Xcode Command Line Tools on macOS, MSVC Build Tools on Windows
- **Rust 1.92+** — `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

Clone the repository and install the binary from the repo root:

```bash
git clone https://github.com/5pecia1/UPeg.git
cd UPeg
cargo install --locked --path upeg-cli   # installs `upeg` into ~/.cargo/bin
upeg doctor
```

`cargo install` puts `upeg` on your `PATH` (rustup already adds
`~/.cargo/bin`). To build without installing, run
`cargo build --release -p upeg-cli` and call the binary as
`./target/release/upeg`.

Optional but recommended:

```bash
# Shell completions — pick your shell
mkdir -p ~/.local/share/bash-completion/completions
upeg completions bash > ~/.local/share/bash-completion/completions/upeg
# zsh: upeg completions zsh > <a directory in fpath>/_upeg
# fish: upeg completions fish > ~/.config/fish/completions/upeg.fish
```

Regenerate completions after installing new Toolkits — the Tool list is
embedded when the script is generated.

## Desktop app and PWA (Flutter)

The Desktop and PWA surfaces are Flutter apps over a Rust bridge
(`flutter_rust_bridge`). Building them additionally requires:

- **Flutter SDK** — <https://docs.flutter.dev/get-started/install>
- **`just`** — `cargo install just --locked`
- **Linux desktop dependencies** for `flutter run -d linux` /
  `flutter-build-linux`: `libgtk-3-dev`, `pkg-config`,
  `libayatana-appindicator3-dev`, `libkeybinder-3.0-dev` (runtime:
  `libkeybinder-3.0-0`)
- **FRB codegen** when regenerating bindings:
  `cargo install flutter_rust_bridge_codegen --locked --version 2.12.0`

```bash
cd flutter_app
flutter pub get
flutter run -d linux            # or: -d chrome, -d web-server
```

CMake invokes the Rust build for `upeg_frb` itself — no separate
`cargo build` step. For the web/PWA build the `wasm32` bridge must be built
first; the `just` recipes handle it:

```bash
just flutter-run-web            # FRB wasm + flutter run -d chrome
just package-web                # static PWA bundle → target/packages/web/
```

## Chrome extension

No toolchain at all — the extension is dependency-free MV3 JavaScript:

```bash
./chrome-ext/build.sh           # stages chrome-ext/dist/
```

Then `chrome://extensions` → enable "Developer mode" → "Load unpacked" →
select `chrome-ext/dist/`. The extension talks to a local upeg HTTP host for
real tool results.

## WASM plugins

- **`wasm32-unknown-unknown` target** —
  `rustup target add wasm32-unknown-unknown`
- **`wasm-pack`** — `cargo install wasm-pack --locked`
- Plugin commands (`upeg plugin install` / `upeg plugin list` /
  `upeg wasm`) need the `wasm-plugin` cargo feature, which is on by
  default — only a `--no-default-features` build lacks them.
  `upeg doctor` lists enabled features.

Scaffold a guest crate with `upeg plugin new <name>`; see
`examples/plugins/greet/` for a complete example.

## Packaging (installable artifacts)

```bash
just package                    # everything the host OS can produce
just package-linux-deb          # → target/packages/linux/upeg_<ver>_amd64.deb
just package-linux-appimage     # → target/packages/linux/upeg-<ver>-x86_64.AppImage
just package-macos-dmg          # → target/packages/macos/upeg-<ver>.dmg      (macOS only)
just package-windows-msix       # → target/packages/windows/*.msix           (Windows only)
just package-web                # → target/packages/web/ (PWA bundle, pre-gzipped)
```

| Recipe | Extra tools |
|---|---|
| `package-linux-deb` | `fpm` (`gem install fpm`) |
| `package-linux-appimage` | `appimagetool` ([appimagetool releases](https://github.com/AppImage/appimagetool/releases)) + `appstreamcli` (`appstream`); the recipe sets `APPIMAGE_EXTRACT_AND_RUN=1` so FUSE is not needed |
| `package-macos-dmg` | `create-dmg` (`brew install create-dmg`) — macOS host only |
| `package-windows-msix` | the `msix` dev dependency and `msix_config` in `pubspec.yaml` — Windows host only |
| `package-web` | `gzip` |

All artifacts land in `target/packages/<platform>/`
(`UPEG_PACKAGE_OUT=<dir>` redirects that root for a fresh staging run).
Every package embeds the full notice set — `LICENSE`, `NOTICE`, the OFL
font licenses, and `BUILD-INFO` — via `packaging/release-files.sh`.
Signing and notarization are intentionally out of scope — artifacts are
unsigned.

The `.deb` installs the app under `/opt/upeg` and links
`/usr/bin/upeg-app` — the desktop app's PATH entry. The plain `upeg`
name stays with the CLI binary; the `.desktop` file launches the app by
its absolute path.

The release workflow (`release`, manual dispatch) stages a fixed set of
unsigned release assets, each with a `.sha256` sidecar:
`upeg-v<ver>-x86_64-unknown-linux-gnu.tar.gz`,
`upeg-v<ver>-aarch64-apple-darwin.tar.gz`,
`upeg-v<ver>-x86_64-pc-windows-msvc.zip` (CLI builds),
`upeg_<ver>_amd64.deb` and `upeg-v<ver>-x86_64.AppImage` (Linux desktop),
and `upeg-v<ver>-web.tar.gz` (PWA bundle). macOS `.dmg` and Windows
`.msix` remain local-only recipes — they are not part of the release
set.

## Verify the install

```bash
upeg doctor                 # binary path, features, source dirs, toolbox counts
upeg tool list              # every registered Tool
upeg call num.hex_to_decimal -a input=0xff    # → 255, no network needed
```

If something is off, see [Troubleshooting](troubleshooting.md). For the dev
workflow (`just check`, `just verify`, test tiers) see
[Development](development.md).
