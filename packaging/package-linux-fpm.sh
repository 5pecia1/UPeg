#!/usr/bin/env bash
# Package the Flutter Linux bundle with the same layout for DEB and RPM.
set -euo pipefail

format="${1:-}"
case "$format" in deb|rpm) ;; *) echo 'usage: package-linux-fpm.sh {deb|rpm}' >&2; exit 2 ;; esac
SOURCE_ROOT="${UPEG_SOURCE_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
cd "$SOURCE_ROOT"
command -v fpm >/dev/null 2>&1 || { echo 'error: fpm not installed (gem install fpm)' >&2; exit 1; }
if [ "$format" = rpm ]; then
  command -v rpmbuild >/dev/null 2>&1 || { echo 'error: rpmbuild not installed (install rpm)' >&2; exit 1; }
fi

out="${UPEG_PACKAGE_OUT:-target/packages}/linux"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p \
  "$stage/opt/upeg" \
  "$stage/usr/bin" \
  "$stage/usr/share/applications" \
  "$stage/usr/share/icons/hicolor/512x512/apps" \
  "$stage/usr/share/metainfo"
cp -a flutter_app/build/linux/x64/release/bundle/. "$stage/opt/upeg/"
ln -s /opt/upeg/upeg "$stage/usr/bin/upeg-app"
sed 's|^Exec=upeg %u$|Exec=/opt/upeg/upeg %u|' packaging/linux/io.github._5pecia1.upeg.desktop \
  > "$stage/usr/share/applications/io.github._5pecia1.upeg.desktop"
cp packaging/linux/io.github._5pecia1.upeg.metainfo.xml "$stage/usr/share/metainfo/io.github._5pecia1.upeg.metainfo.xml"
cp flutter_app/web/icons/Icon-512.png "$stage/usr/share/icons/hicolor/512x512/apps/upeg.png"
bash packaging/release-files.sh stage "$stage/usr/share/doc/upeg"
version="$(bash packaging/release-files.sh version)"
version="${version#v}"
case "$version" in
  [0-9]*) ;;
  *) version="0.0.0+$version" ;;
esac

if [ "$format" = deb ]; then
  rm -f "$out"/upeg_*.deb
  package="$out/upeg_${version}_amd64.deb"
  architecture=amd64
  dependencies=(
    libgtk-3-0
    libkeybinder-3.0-0
    libayatana-appindicator3-1
    libwebkit2gtk-4.1-0
  )
else
  package="$out/upeg-${version}-1.x86_64.rpm"
  rm -f "$package"
  architecture=x86_64
  dependencies=(
    gtk3
    keybinder3
    libayatana-appindicator-gtk3
    webkit2gtk4.1
  )
fi
dependency_args=()
for dependency in "${dependencies[@]}"; do
  dependency_args+=(--depends "$dependency")
done
fpm_args=(-s dir -t "$format" -n upeg -v "$version" --architecture "$architecture")
if [ "$format" = rpm ]; then
  fpm_args+=(--iteration 1)
fi
fpm "${fpm_args[@]}" "${dependency_args[@]}" \
  --description 'Universal Pegboard' \
  --license Apache-2.0 \
  --maintainer 'upeg contributors' \
  --url https://github.com/5pecia1/UPeg \
  -C "$stage" --package "$package" .
if [ "$format" = deb ] && command -v dpkg-deb >/dev/null 2>&1; then
  dpkg-deb -c "$package" | grep 'usr/share/doc/upeg/NOTICE' \
    || { echo "error: $package lacks staged notices" >&2; exit 1; }
elif [ "$format" = rpm ] && command -v rpm >/dev/null 2>&1; then
  rpm -qpl "$package" 2>/dev/null | grep -F '/usr/share/doc/upeg/NOTICE' \
    || { echo "error: $package lacks staged notices" >&2; exit 1; }
fi
ls -la "$out"
