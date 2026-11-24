---
title: Installation
description: "Install the CLI or desktop app from a release, or build upeg from source."
type: Guide
tags: [installation]
---

# Installation

Choose a CLI or desktop package from the [latest release](https://github.com/5pecia1/UPeg/releases/latest).
The latest link follows new published releases; filenames below use `<version>`
for the selected release's numeric version. Building from source is optional.

## Install or update the CLI with curl

On **macOS Apple Silicon**, **GNU/Linux x86_64**, or **GNU/Linux ARM64**, run:

```sh
curl -fsSL https://raw.githubusercontent.com/5pecia1/UPeg/main/install.sh | sh
```

This installs the latest published CLI at `~/.local/bin/upeg` without `sudo` or
Rust and checks the archive's SHA256 before installation.
Run the same command again to update that CLI. It does not install or
update the desktop app. Windows and Intel Mac CLI installation are not supported
by this script; use the choices below.

If your shell cannot find `upeg`, add the following line to your shell's startup
file, such as `~/.zshrc` or `~/.bashrc`, then open a new terminal:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

`command -v upeg` shows which CLI your shell selects. Use `upeg --version` and
`upeg doctor` to check that installation.

### Linux CLI compatibility

The Linux CLI archives target GNU/glibc, not musl/Alpine. The **v0.7.9** x86_64
and ARM64 CLI archives were checked and require **glibc 2.39 or newer**; this is
a requirement of those archives, not a fixed requirement for every release.
Older glibc or musl systems may not run the published archive. Where the build
dependencies are available, use [Build from source](#build-from-source).

## Choose a download

GUI packages open a desktop window. CLI packages provide the terminal, TUI,
MCP, and HTTP commands. Installing a GUI package does not put the CLI on your
`PATH`; install the CLI separately if you need those terminal commands.

| OS / CPU | Desktop GUI | Manual CLI download |
|---|---|---|
| GNU/Linux x86_64 (glibc) | `upeg_<version>_amd64.deb`, `upeg-<version>-1.x86_64.rpm`, or `upeg-v<version>-x86_64.AppImage` | `upeg-v<version>-x86_64-unknown-linux-gnu.tar.gz` |
| GNU/Linux ARM64 (glibc) | No desktop package | `upeg-v<version>-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Apple Silicon | `upeg-v<version>.dmg` (universal) | `upeg-v<version>-aarch64-apple-darwin.tar.gz` |
| macOS Intel | The same universal DMG | No Intel Mac CLI archive |
| Windows x86_64 | `upeg-v<version>-x86_64.msix` (see its signing limitation below) | `upeg-v<version>-x86_64-pc-windows-msvc.zip` |
| Web deployment | `upeg-v<version>-web.tar.gz`, for site operators | Not a desktop or CLI installer |

Download the selected package and its same-named `.sha256` file. You do not
need to download `toolkits-*` files manually: the app or CLI selects the current
OS/CPU catalog and fetches a versioned pack when its Tool is first used. The
verified cache can be used offline afterwards. `release-manifest.json` is the
complete release inventory of filenames, sizes, and SHA256 digests.

## Check a manual download

Keep the package and its `.sha256` file together. In the commands below,
replace `downloaded-file-name` with the package's actual filename.

On macOS or Linux:

```sh
cd "$HOME/Downloads"
FILE='downloaded-file-name'
shasum -a 256 -c "$FILE.sha256"       # macOS
# On Linux, use: sha256sum -c "$FILE.sha256"
```

In Windows PowerShell:

```powershell
Set-Location "$HOME\Downloads"
$file = 'downloaded-file-name'
$expected = ((Get-Content -LiteralPath "$file.sha256" -Raw).Trim() -split '\s+')[0]
$actual = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash
if ($actual -ne $expected) { throw 'SHA256 mismatch' }
```

Continue only when the checksum matches.

## Manual CLI installation

### macOS Apple Silicon and Linux

The curl installer above is the usual path. To install an archive manually,
verify it first, then extract it and copy its CLI into a directory on your `PATH`:

```sh
archive='downloaded-cli-archive.tar.gz'
tar -xzf "$archive"
mkdir -p "$HOME/.local/bin"
install -m 755 "${archive%.tar.gz}/upeg" "$HOME/.local/bin/upeg"
export PATH="$HOME/.local/bin:$PATH"
upeg --version
```

Use the release's original archive filename, which is also the extracted
directory name. See the PATH instructions above to retain the setting.

### Windows CLI

Verify the x86_64 CLI ZIP, then extract it into a new folder. In PowerShell:

```powershell
$archive = 'downloaded-cli-archive.zip'
Expand-Archive -LiteralPath $archive -DestinationPath '.\upeg-cli'
$cli = Get-ChildItem -LiteralPath '.\upeg-cli' -Recurse -File -Filter upeg.exe
& $cli.FullName --version
$env:Path = "$($cli.DirectoryName);$env:Path"
upeg doctor
```

Keep that extracted directory. To retain the command in future terminals, add
the directory containing this CLI's `upeg.exe` to your **user Path** in Windows
Environment Variables, then open a new terminal. The MSIX desktop package is a
separate download; it does not replace this CLI ZIP.

## Install a desktop package

### macOS desktop

Verify the universal DMG, open it, and drag `upeg.app` onto the Applications
shortcut in the DMG window, or into Applications in Finder if the shortcut is
absent. Open `upeg` from Applications. The same DMG supports Apple Silicon and
Intel Macs; the ARM64 CLI archive is a separate package.

For additional verification of a notarized DMG and its installed app:

```sh
DMG="$HOME/Downloads/downloaded-file-name.dmg"
xcrun stapler validate "$DMG"
spctl --assess --type open --context context:primary-signature -v "$DMG"
codesign --verify --deep --strict /Applications/upeg.app
spctl --assess --type execute -v /Applications/upeg.app
```

For the separately installed macOS CLI, use
`codesign --verify --strict --check-notarization -R='notarized' "$HOME/.local/bin/upeg"`.
This explicitly requires a notarization ticket and may retrieve it online.
The v0.7.8 macOS download predates this release-signing workflow.

### Linux desktop

Use a package for your distribution. After verifying the download, choose one:

```sh
sudo apt install ./downloaded-file-name.deb       # Debian / Ubuntu
sudo dnf install ./downloaded-file-name.rpm       # RPM-based systems with dnf
chmod +x ./downloaded-file-name.AppImage
./downloaded-file-name.AppImage                   # AppImage alternative
```

The `.deb` and `.rpm` install the app under `/opt/upeg`, add a desktop entry,
and link `/usr/bin/upeg-app`. Launch it from the application menu or run
`upeg-app`; `upeg` remains the separate CLI command. Before installing a deb or
rpm over a local source build, remove that build's desktop-entry override at
`~/.local/share/applications/io.github._5pecia1.upeg.desktop`.

### Windows desktop

The currently published MSIX is unsigned, so standard Windows installation may
reject it. SHA256 verification confirms the downloaded bytes; it does not add a
trusted signature. The Windows CLI ZIP above remains the manual terminal option.

For an MSIX with a valid signature trusted by Windows, open it in App Installer,
or install it with `Add-AppxPackage -Path .\downloaded-file-name.msix` in
PowerShell. Do not bypass Windows security policy to install an unsigned asset.
See Microsoft's [MSIX signing overview](https://learn.microsoft.com/en-us/windows/msix/package/signing-package-overview)
for the package-signing requirements.

### Desktop updates

When UPeg finds a newer stable release, open the update icon and review the
version. Selecting **Later** dismisses that version. Updates start only after
you approve them.

- **macOS** verifies the signed, notarized DMG before replacing and reopening
  a writable, signed installation. Other installs open the tagged release page.
- **Linux** verifies the matching release asset against its SHA-256 checksum.
  UPeg replaces a writable, user-owned AppImage in place. For a system
  installation it asks for administrator approval and installs the matching
  DEB with APT or RPM with DNF. UPeg then restarts. Systems without the required
  update tools open the tagged release page.
- **Windows** opens the tagged release page to download the installer manually.

## Web deployment

The web archive is for operators deploying a PWA, rather than a desktop
installer. Extract `upeg-v<version>-web.tar.gz` and serve its contents over HTTPS
(or localhost) with the [required isolation headers](#web-hosting-requirements).
You can deploy the published archive without building Rust or Flutter. The
source build instructions below also cover the PWA and Chrome extension.

## Verify the install

For the CLI:

```sh
upeg --version
upeg doctor
upeg tool list
upeg call --local num.hex_to_decimal -a input=0xff    # expected: 255
```

The first Tool call may need internet access to download its pack. The verified
cache supports later offline use. `--local` selects in-process execution; it is
not an offline mode. GUI users can follow the
[desktop first-use steps](quick-start.md#desktop-first-use) without installing the CLI.

## Build from source

These tools are for development or a source build, not prerequisites for the
prebuilt CLI installer or desktop downloads. Local `just package-*` outputs
remain unsigned development artifacts.

### CLI, TUI, MCP, and HTTP — the `upeg` binary

One Rust toolchain covers all of these; they ship in a single binary. You
need:

- **Git**
- **A C toolchain** for the linker — `build-essential` on Debian/Ubuntu,
  Xcode Command Line Tools on macOS, MSVC Build Tools on Windows
- **Rust 1.92+** — install with the OS-specific instructions at
  <https://rustup.rs/>. On macOS/Linux: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`;
  on Windows, run `rustup-init.exe` from that site.

Clone the repository and install the binary from the repo root:

```bash
git clone https://github.com/5pecia1/UPeg.git
cd UPeg
cargo install --locked --path upeg-cli   # installs `upeg` into ~/.cargo/bin
upeg doctor
```

`cargo install` places `upeg` in `~/.cargo/bin`; ensure that directory is on
your `PATH` (rustup normally adds it). To build without installing, run
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

### Desktop app and PWA (Flutter)

The Desktop and PWA surfaces are Flutter apps over a Rust bridge
(`flutter_rust_bridge`). Building them additionally requires:

- **Flutter SDK** — <https://docs.flutter.dev/get-started/install>
- **`just`** — `cargo install just --locked`
- **Linux desktop dependencies** for `flutter run -d linux` /
  `flutter-build-linux`: `libgtk-3-dev`, `pkg-config`,
  `libayatana-appindicator3-dev`, `libkeybinder-3.0-dev` (runtime:
  `libkeybinder-3.0-0`)

```bash
cd flutter_app
flutter pub get
flutter run -d linux --release
```

On Linux, CMake invokes the Rust build for `upeg_frb` itself — no separate
`cargo build` step. The first Rust and Flutter build can take several minutes.
Use release mode when judging startup time and memory: debug uses Flutter JIT
and its `libupeg_frb.so` is about 991 MB on disk in this build. Alternatively,
run `just flutter-build-linux` at the repository root, then launch
`flutter_app/build/linux/x64/release/bundle/upeg`.

On a Linux desktop host, after the first release build, run
`just flutter-desktop-entry-linux` once to install a local desktop entry.
It supplies the app icon, launch notification, and `upeg://` link handler,
particularly on Wayland. Run it on the desktop host, not inside a container.
The recipe fails if the release bundle does not exist. If `flutter clean`
removes the bundle, rebuild before launching through GNOME; GNOME ignores an
entry whose executable is missing. Rerun the recipe if the bundle path moves.
When switching this source build to a deb or rpm, follow the desktop-entry
override note under [Linux desktop](#linux-desktop).

On Linux, launching the same storage root a second time brings the existing
window forward. A terminal relaunch prints one stderr line:
`upeg: already running; activated the existing window`.

On macOS, use `cd flutter_app && flutter run -d macos --release` for a local
release run, or `just flutter-build-macos` from the repository root to build
the release `.app`. The DMG made by `just package-macos-dmg` contains that app
bundle. Its app icon and menu bar template icon are bundled; no separate icon
registration step is needed. Local builds remain unsigned. Official releases
sign the app, then sign, notarize and staple its DMG. For downloaded packages,
use the [macOS installation steps](#macos-desktop).

From the repository root, the web/PWA recipes build the `wasm32` bridge first:

```bash
just flutter-run-web            # FRB wasm + flutter run -d chrome
just flutter-run-web-server     # FRB wasm + flutter run -d web-server
just package-web                # static PWA bundle → target/packages/web/
```

The web recipes automatically prepare the supported Rust bridge toolchain:
`nightly-2025-12-08` with `rust-src`, plus `wasm-pack 0.13.1`. The bridge
uses shared WebAssembly memory in a Worker.

<a id="web-hosting-requirements"></a>

Serve a built or downloaded PWA over
HTTPS (or `localhost`) with all of these response headers:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
Cross-Origin-Resource-Policy: same-origin
```

The PWA smoke harness's Python server adds those headers; a bare
`python -m http.server` does not. GitHub Pages documentation publishing is
not application hosting and does not provide this PWA deployment contract.

`just package-web` also builds the selected web Toolkit catalog and its
versioned JS/WASM packs into the same static bundle. A catalog or artifact
digest mismatch stops packaging. Native distributed Toolkits use the catalog
configured for the current OS. Source builds without a configured catalog URL
use the matching GitHub Release catalog; official downloads use their embedded
Pages catalog URL. First use may download a pack, and the verified cache is
usable offline afterwards.

### Chrome extension

The extension is dependency-free MV3 JavaScript: it needs no compiler or npm
dependencies, but its staging script needs Bash (Git Bash or WSL on Windows).

```bash
./chrome-ext/build.sh           # stages chrome-ext/dist/
```

Then `chrome://extensions` → enable "Developer mode" → "Load unpacked" →
select `chrome-ext/dist/`. The extension talks to a local upeg HTTP host for
real tool results. Reuse a running host or start one with
`upeg http --addr 127.0.0.1:7173`, then run `upeg http status --pairing`.
Open the popup, save its endpoint and token in the gear menu, then return to
the Board list; see the
[extension guide](https://github.com/5pecia1/UPeg/blob/main/chrome-ext/README.md)
for the full pairing and manual test flow.

### WASM plugins

Plugin commands (`upeg plugin install` / `upeg plugin list` /
`upeg wasm`) are included in the default CLI and desktop builds.
The FRB bridge enables `wasm-plugin` by default too, so its standard tests
exercise the shipped configuration. A `--no-default-features` build lacks
these commands.
`upeg doctor` lists enabled features. Building your own plugin needs the
`wasm32-unknown-unknown` target and `wasm-pack`; see the
[tool author guide](tool-author.md#wasm-plugins-and-mcp-imports) and
[Development](development.md) for the toolchain.

### Packaging (installable artifacts)

```bash
just package                    # Linux desktop packages + web bundle (Linux host)
just package-linux-deb          # → target/packages/linux/upeg_<ver>_amd64.deb
just package-linux-rpm          # → target/packages/linux/upeg-<ver>-1.x86_64.rpm
just package-linux-appimage     # → target/packages/linux/upeg-<ver>-x86_64.AppImage
just package-macos-dmg          # → target/packages/macos/upeg-<ver>.dmg      (macOS only)
just package-windows-msix       # → target/packages/windows/*.msix           (Windows only)
just package-web                # → target/packages/web/ (PWA bundle, pre-gzipped)
```

| Recipe | Extra tools |
|---|---|
| `package-linux-deb` | `fpm` (`gem install fpm`) |
| `package-linux-rpm` | `fpm` (`gem install fpm`) and `rpmbuild`/`rpm` (`apt install rpm` on Debian) |
| `package-linux-appimage` | `appimagetool` ([appimagetool releases](https://github.com/AppImage/appimagetool/releases)) + `appstreamcli` (`appstream`); the recipe sets `APPIMAGE_EXTRACT_AND_RUN=1` so FUSE is not needed |
| `package-macos-dmg` | `create-dmg` (`brew install create-dmg`) — macOS host only |
| `package-windows-msix` | the `msix` dev dependency and `msix_config` in `pubspec.yaml` — Windows host only |
| `package-web` | `gzip` |

All artifacts land in `target/packages/<platform>/`
(`UPEG_PACKAGE_OUT=<dir>` redirects that root). `just package` builds the
Linux desktop packages and web bundle; use the explicit macOS or Windows
recipe on those hosts. The MSIX recipe requires exactly one new package from
the current run and refuses an existing destination filename. Remove or move
that specific prior file before rebuilding the same version.
Every package embeds the full notice set — `LICENSE`, `NOTICE`, the OFL
font licenses, and `BUILD-INFO` — via `packaging/release-files.sh`.

The public `release` workflow verifies builds from the exported source. Official
publication distributes finalized producer packages, with checksums made after
signing/notarization where applicable. The [download table](#choose-a-download)
lists those packages. Official releases also include the native Toolkit catalogs
and packs for each supported target, plus a complete `release-manifest.json`.
