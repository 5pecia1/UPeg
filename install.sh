#!/bin/sh
# Install the latest stable public CLI without a Rust toolchain or sudo.
# Keep installation inside main: an incomplete curl | sh must not run it.
main() {
    set -eu
    umask 077

    fail() { printf 'UPeg install: %s\n' "$*" >&2; exit 1; }
    [ "$#" -eq 0 ] || fail 'This installer takes no options.'
    for tool in curl tar uname id find awk grep mktemp chmod mv rm mkdir; do
        command -v "$tool" >/dev/null 2>&1 || fail "Required command is missing: $tool"
    done
    if command -v sha256sum >/dev/null 2>&1; then
        checksum() { sha256sum "$1" | awk '{print $1}'; }
    elif command -v shasum >/dev/null 2>&1; then
        checksum() { shasum -a 256 "$1" | awk '{print $1}'; }
    else
        fail 'SHA-256 verification needs sha256sum or shasum.'
    fi

    platform="$(uname -s)/$(uname -m)"
    if [ "$platform" = Darwin/x86_64 ] && command -v sysctl >/dev/null 2>&1; then
        translated="$(sysctl -n sysctl.proc_translated 2>/dev/null)" || translated=0
        if [ "$translated" = 1 ]; then
            platform=Darwin/arm64
        fi
    fi
    case "$platform" in
        Darwin/arm64|Darwin/aarch64) target=aarch64-apple-darwin ;;
        Linux/x86_64|Linux/amd64) target=x86_64-unknown-linux-gnu ;;
        Linux/aarch64|Linux/arm64) target=aarch64-unknown-linux-gnu ;;
        Darwin/*) fail 'The CLI archive supports Apple Silicon Macs; use the macOS app for Intel Macs: https://github.com/5pecia1/UPeg/releases/latest' ;;
        *) fail "Unsupported platform: $platform. Supported: Apple Silicon macOS, Linux x86_64 and Linux ARM64." ;;
    esac
    case "${HOME:-}" in /*) ;; *) fail 'An absolute HOME directory is required.' ;; esac
    [ -d "$HOME" ] || fail 'HOME must be an existing directory.'

    bindir="$HOME/.local/bin"
    mkdir -p "$bindir" || fail "Cannot create $bindir; no sudo is needed."
    # Resolve intentional parent/home symlinks; stage beside the final file.
    bindir="$(CDPATH='' cd "$bindir" && pwd -P)" || fail 'Cannot resolve the installation directory.'
    destination="$bindir/upeg"
    user_id="$(id -u)"
    check_destination() {
        if [ -L "$destination" ] || { [ -e "$destination" ] && [ ! -f "$destination" ]; }; then
            fail "Preserving $destination: it is a symlink or is not a regular file."
        fi
        if [ -f "$destination" ]; then
            owned="$(find "$destination" -type f -links 1 -user "$user_id" -print)"
            if [ "$owned" != "$destination" ] || [ ! -w "$destination" ]; then
                fail "Preserving $destination: it is not an unshared, writable file owned by this user."
            fi
        fi
    }
    check_destination
    work="$(mktemp -d "$bindir/.upeg-install.XXXXXX")" || fail 'Cannot create a temporary installation directory.'
    trap 'rm -rf "$work"' 0
    trap 'exit 1' 1 2 15

    repository=https://github.com/5pecia1/UPeg
    # Resolve latest once. Every following request uses this exact release.
    release_url="$(curl -q --head --fail --location --silent --show-error \
        --proto '=https' --proto-redir '=https' --retry 3 \
        --connect-timeout 15 --max-time 180 --output /dev/null \
        --write-out '%{url_effective}' "$repository/releases/latest")" \
        || fail 'Cannot resolve the latest public release; the existing CLI was kept.'
    case "$release_url" in "$repository/releases/tag/"*) ;; *) fail 'Unexpected latest-release URL; the existing CLI was kept.' ;; esac
    tag="${release_url#"$repository/releases/tag/"}"
    printf '%s\n' "$tag" | LC_ALL=C grep -Eq '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(\+[0-9A-Za-z]+([.-][0-9A-Za-z]+)*)?$' \
        || fail 'The latest release has no supported stable version tag; the existing CLI was kept.'
    archive="upeg-$tag-$target.tar.gz"
    base_url="$repository/releases/download/$tag"
    for file in "$archive" "$archive.sha256"; do
        curl -q --fail --location --silent --show-error \
            --proto '=https' --proto-redir '=https' --retry 3 \
            --connect-timeout 15 --max-time 180 --output "$work/$file" "$base_url/$file" \
            || fail 'A release download failed; the existing CLI was kept.'
    done
    expected="$(LC_ALL=C awk -v name="$archive" '
        NR == 1 && length($1) == 64 && $1 !~ /[^0-9a-f]/ && $0 == $1 "  " name { hash = $1 }
        END { if (NR != 1 || hash == "") exit 1; print hash }
    ' "$work/$archive.sha256")" || fail 'Invalid release checksum file; the existing CLI was kept.'
    [ "$(checksum "$work/$archive")" = "$expected" ] \
        || fail 'Release checksum mismatch; the existing CLI was kept.'

    member="upeg-$tag-$target/upeg"
    listing="$(tar -tzf "$work/$archive" "$member")" \
        || fail 'The release archive is missing its CLI; the existing CLI was kept.'
    [ "$listing" = "$member" ] || fail 'Unexpected archive members; the existing CLI was kept.'
    details="$(LC_ALL=C tar -tvzf "$work/$archive" "$member")" \
        || fail 'Cannot inspect the CLI archive; the existing CLI was kept.'
    case "$details" in
        -*" $member") ;;
        *) fail 'The archived CLI is not a regular file; the existing CLI was kept.' ;;
    esac
    tar -xOzf "$work/$archive" "$member" > "$work/upeg" \
        || fail 'Cannot extract the CLI; the existing CLI was kept.'
    chmod 755 "$work/upeg"
    version="$("$work/upeg" --version 2>"$work/version-error")" \
        || fail 'The downloaded CLI cannot run on this system (Linux may need a newer glibc); the existing CLI was kept.'
    [ "$version" = "upeg ${tag#v}" ] \
        || fail 'The downloaded CLI reports an unexpected version; the existing CLI was kept.'
    check_destination
    mv -f "$work/upeg" "$destination" || fail 'Cannot replace the CLI; the existing CLI was kept.'
    printf 'Installed UPeg %s to %s\n' "$tag" "$destination"
    case ":${PATH:-}:" in
        *":$bindir:"*|*":$HOME/.local/bin:"*) ;;
        *)
            # shellcheck disable=SC2016 # Print a command for the user's shell profile.
            printf 'Add this directory to PATH in your shell profile:\n  export PATH="$HOME/.local/bin:$PATH"\n'
            ;;
    esac
}

main "$@"
