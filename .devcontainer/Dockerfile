# ref: https://github.com/olivierlemasle/devcontainers-features/tree/main/src/tauri
ARG VARIANT="1.92.0-slim-trixie"
FROM rust:${VARIANT}

ENV DEBIAN_FRONTEND=noninteractive

# Install the native toolchain required by `just verify`, Flutter desktop
# builds, and Linux packaging. Keep this aligned with GitHub workflows.
# `file` is retained for general dev convenience (tiny package).
RUN apt-get update -qq \
    && apt-get install -y -qq \
    appstream \
    bats \
    build-essential \
    ca-certificates \
    clang \
    cmake \
    curl \
    file \
    git \
    libayatana-appindicator3-dev \
    libglu1-mesa \
    libgtk-3-dev \
    libjsoncpp-dev \
    libkeybinder-3.0-dev \
    libsecret-1-dev \
    libsqlite3-dev \
    libssl-dev \
    ninja-build \
    pkg-config \
    ruby \
    ruby-dev \
    shellcheck \
    unzip \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Chromium + xvfb for `flutter test --platform=chrome` and headless
# Flutter Web screenshot extraction. swiftshader fallback so software
# WebGL keeps CanvasKit/Skwasm rendering on machines without a GPU.
# Debian's `chromium` package is lighter than google-chrome-stable and
# avoids pulling Google's vendor apt repo. `CHROME_EXECUTABLE` is the
# env var that `flutter run -d chrome` and `flutter test --platform=chrome`
# look for; pin it so users do not have to.
RUN apt-get update -qq \
    && apt-get install -y -qq \
    chromium \
    fonts-liberation \
    libasound2 \
    libnss3 \
    libxss1 \
    xvfb \
    && rm -rf /var/lib/apt/lists/*
ENV CHROME_EXECUTABLE=/usr/bin/chromium

# `just verify` requires wasm, rustfmt, and clippy. Bump JUST_VERSION
# together with its SHA256 pins. The pins are checked against the
# downloaded tarball before extraction; a tampered or replaced release
# asset fails the build.
ARG JUST_VERSION="1.51.0"
ARG JUST_SHA256_AMD64="c8f085ca3e885723c341d06243fc291b5abfdc8bbe3b2c076b117de490387b59"
ARG JUST_SHA256_ARM64="ed7ec466b77709198fd4afed253dba0270203ba5eb1c006bee2b0139090284f5"
RUN rustup target add wasm32-unknown-unknown \
    && rustup component add clippy rustfmt \
    && case "$(dpkg --print-architecture)" in \
        amd64) just_target="x86_64-unknown-linux-musl"; just_sha="${JUST_SHA256_AMD64}" ;; \
        arm64) just_target="aarch64-unknown-linux-musl"; just_sha="${JUST_SHA256_ARM64}" ;; \
        *) echo "unsupported Debian architecture: $(dpkg --print-architecture)" >&2; exit 1 ;; \
    esac \
    && curl -fsSL "https://github.com/casey/just/releases/download/${JUST_VERSION}/just-${JUST_VERSION}-${just_target}.tar.gz" -o /tmp/just.tar.gz \
    && echo "${just_sha}  /tmp/just.tar.gz" | sha256sum -c - \
    && tar -xzf /tmp/just.tar.gz -C /tmp \
    && install -m 0755 /tmp/just /usr/local/cargo/bin/just \
    && rm -f /tmp/just.tar.gz /tmp/just

# actionlint — `just actionlint` (CI lane 1/4) lints every workflow file.
# Installed here, SHA256-pinned like `just` above, so
# `scripts/ci/actionlint.sh`'s `command -v actionlint` fast path hits.
# Without it, every CI run inside this ephemeral container downloads and
# executes an installer script from actionlint's `main` branch — an
# unpinned remote script in the verification path, and a network round
# trip for a lint step. Keep ACTIONLINT_VERSION in sync with
# `scripts/ci/actionlint.sh`; the script rejects a PATH binary whose
# version does not match its own pin, so a drifted pair silently falls
# back to downloading.
ARG ACTIONLINT_VERSION="1.7.12"
ARG ACTIONLINT_SHA256_AMD64="8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8"
ARG ACTIONLINT_SHA256_ARM64="325e971b6ba9bfa504672e29be93c24981eeb1c07576d730e9f7c8805afff0c6"
RUN case "$(dpkg --print-architecture)" in \
        amd64) actionlint_target="linux_amd64"; actionlint_sha="${ACTIONLINT_SHA256_AMD64}" ;; \
        arm64) actionlint_target="linux_arm64"; actionlint_sha="${ACTIONLINT_SHA256_ARM64}" ;; \
        *) echo "unsupported Debian architecture: $(dpkg --print-architecture)" >&2; exit 1 ;; \
    esac \
    && curl -fsSL "https://github.com/rhysd/actionlint/releases/download/v${ACTIONLINT_VERSION}/actionlint_${ACTIONLINT_VERSION}_${actionlint_target}.tar.gz" -o /tmp/actionlint.tar.gz \
    && echo "${actionlint_sha}  /tmp/actionlint.tar.gz" | sha256sum -c - \
    && tar -xzf /tmp/actionlint.tar.gz -C /tmp actionlint \
    && install -m 0755 /tmp/actionlint /usr/local/bin/actionlint \
    && rm -f /tmp/actionlint.tar.gz /tmp/actionlint

# Flutter SDK: pinned to the public CI toolchain.
# Install location matches the reference template at
# https://github.com/5pecia1/.devcontainer/tree/main/flutter so editor
# settings (`dart.flutterSdkPath`) are interchangeable across projects.
#
# Multi-arch policy: Flutter publishes only Linux x86_64 (amd64) tarballs
# at the official release manifest (`releases_linux.json` lists arch=x64
# only). The devcontainer image build in CI targets linux/amd64 +
# linux/arm64 via buildx so the GHCR manifest covers both. On arm64 we
# intentionally skip the Flutter install rather than fail the build —
# Phase 0's CI Linux runners are amd64, downstream lanes pull the amd64
# manifest. arm64 users land in a Flutter-less image; this is acceptable
# until upstream publishes an arm64 SDK.
ENV FLUTTER_SDK_ROOT=/usr/lib/flutter
ENV PATH="${PATH}:${FLUTTER_SDK_ROOT}/bin"
ARG FLUTTER_SDK_VERSION="3.44.0-stable"
ARG FLUTTER_SDK_SHA256_AMD64="e1ec95e6c550458a34de93580cb85dac24da0e9bedb9bb42811f050ac5a0c7d5"
RUN if [ "$(dpkg --print-architecture)" = "amd64" ]; then \
        mkdir -p "${FLUTTER_SDK_ROOT}" \
        && curl -fsSL "https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${FLUTTER_SDK_VERSION}.tar.xz" -o /tmp/flutter.tar.xz \
        && echo "${FLUTTER_SDK_SHA256_AMD64}  /tmp/flutter.tar.xz" | sha256sum -c - \
        && tar xf /tmp/flutter.tar.xz -C /usr/lib/ \
        && rm /tmp/flutter.tar.xz \
        && git config --global --add safe.directory "${FLUTTER_SDK_ROOT}" \
        && flutter --disable-analytics \
        && flutter config --no-cli-animations \
        && flutter precache --linux --web --no-android --no-ios --no-macos --no-windows; \
    else \
        echo "Flutter SDK install skipped: no official Linux $(dpkg --print-architecture) tarball published (Phase 0 supports amd64 only)" >&2; \
    fi

# flutter_rust_bridge_codegen — used by `just frb-codegen` and `just frb-codegen-check`.
# Pinned to the same series as the runtime crate in `Cargo.toml`.
ARG FRB_CODEGEN_VERSION="2.12.0"
RUN cargo install flutter_rust_bridge_codegen \
        --locked \
        --version "${FRB_CODEGEN_VERSION}"

# License policy checks use the same version as the public verification action.
ARG CARGO_DENY_VERSION="0.18.2"
RUN cargo install cargo-deny --locked --version "${CARGO_DENY_VERSION}"

# cargo-expand — required by flutter_rust_bridge_codegen to fully analyze
# `#[upeg::tool]` macro-expanded source. Without it, codegen falls back to
# a less precise parser and produces subtly different Dart bindings,
# which then drift-check against the committed copy and fail
# `just frb-codegen-check` in CI.
RUN cargo install cargo-expand --locked

# `cargo fix` and `flutter test` run as the non-root devcontainer user, so
# the shared cargo home and Flutter SDK must be writable after the image is
# built.
RUN chown -R 1000:1000 /usr/local/cargo \
    && if [ -d /usr/lib/flutter ]; then chown -R 1000:1000 /usr/lib/flutter; fi

# libwebkit2gtk-4.1-dev — Linux runtime dep for the Flutter webview
# plugin (webview_all). Same multi-arch policy as the
# Flutter SDK above: install on amd64+arm64, no-op on unsupported arches
# so cross-arch image builds keep going.
RUN ARCH="$(dpkg --print-architecture)" \
    && if [ "$ARCH" = "amd64" ] || [ "$ARCH" = "arm64" ]; then \
        apt-get update \
        && apt-get install -y --no-install-recommends libwebkit2gtk-4.1-dev \
        && rm -rf /var/lib/apt/lists/*; \
    else \
        echo "libwebkit2gtk-4.1-dev install skipped on $ARCH" >&2; \
    fi

# Note: the xcodeproj gem is no longer required. The previous custom
# `just macos-wire-build-phase` recipe edited `Runner.xcodeproj/project.pbxproj`
# programmatically to insert a Run Script build phase invoking
# `Runner/cargo_build.sh`. cargokit replaces that whole approach: its
# integration is declared in `flutter_app/rust_builder/macos/upeg_frb.podspec`
# (an `s.script_phase`), and CocoaPods auto-injects it into the Xcode
# project at `pod install` time. Ruby remains installed only because Linux
# packaging uses the `fpm` gem to produce `.deb` artifacts.
