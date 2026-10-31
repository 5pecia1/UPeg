"""Regenerate raster consumers and website SVG copies from gen.py.

Run from the repository root: python3 brand/src/render.py
Development dependency: CairoSVG. Includes monochrome tray templates and copies.
"""
from pathlib import Path
import re
import shutil
import struct
import xml.etree.ElementTree as ET

import cairosvg
import gen

REPO = Path(__file__).resolve().parents[2]
BRAND = REPO / "brand"
TRAY_SWAY_DELTAS = (6, 1.1, -2.2, -1.3, 0.5, 0.7, 0, -0.2, 0)


def sway_png(source, size, delta, template=False):
    """Rotate only the U around one stationary attachment from the rest asset.

    These temporary render SVGs may use transforms; delivered canonical SVGs
    remain flat. No per-frame silhouette centering or pin movement is applied.
    """
    text = (BRAND / source).read_text()
    if template:
        spec = gen.MONO[44]
        u = gen.optical_u(44, spec, mono=True, motion=True)
        _, pts = gen.mark_geometry(u, gen.TILT)
        px, py = pts[gen.PIN]
    else:
        orange = next(node for node in ET.fromstring(text) if node.attrib.get('fill') == gen.ORANGE)
        if orange.tag.endswith('circle'):
            px, py = float(orange.attrib['cx']), float(orange.attrib['cy'])
        else:
            px = float(orange.attrib['x']) + float(orange.attrib['width']) / 2
            py = float(orange.attrib['y']) + float(orange.attrib['height']) / 2
    text, count = re.subn(r'<path ', f'<path transform="rotate({delta} {gen.f(px)} {gen.f(py)})" ', text)
    assert count == 1, source
    return cairosvg.svg2png(bytestring=text.encode(), output_width=size, output_height=size)


def png(source, size, white=False):
    text = (BRAND / source).read_text()
    if white:
        text = text.replace('fill="#000000"', 'fill="#FFFFFF"')
    return cairosvg.svg2png(bytestring=text.encode(), output_width=size, output_height=size)


def write_png(target, source, size, white=False):
    (REPO / target).write_bytes(png(source, size, white))


def write_ico(target, sizes, tray=False, delta=None):
    # Embed one independently hinted PNG per frame; never downscale another frame.
    frames = []
    for n in sizes:
        source = f"src/tray-color-{n}.svg" if tray else f"src/mark-{n}.svg" if n <= 48 else "mark.svg"
        frames.append(png(source, n) if delta is None else sway_png(source, n, delta))
    offset = 6 + 16 * len(sizes)
    directory = []
    for n, data in zip(sizes, frames):
        directory.append(struct.pack("<BBBBHHII", n % 256, n % 256, 0, 0, 1, 32, len(data), offset))
        offset += len(data)
    (REPO / target).write_bytes(struct.pack("<HHH", 0, 1, len(sizes)) + b"".join(directory) + b"".join(frames))


def write_sway_frames():
    base = REPO / 'flutter_app/assets/tray/sway'
    for platform in ('linux', 'macos', 'windows'):
        (base / platform).mkdir(parents=True, exist_ok=True)
    for frame, delta in enumerate(TRAY_SWAY_DELTAS):
        (base / 'linux' / f'frame_{frame}.png').write_bytes(sway_png('src/tray-color-22.svg', 22, delta))
        (base / 'macos' / f'frame_{frame}.png').write_bytes(sway_png('src/mono-44.svg', 44, delta, template=True))
        write_ico(f'flutter_app/assets/tray/sway/windows/frame_{frame}.ico', (16, 20, 24, 32), tray=True, delta=delta)


def main():
    gen.main()
    for n in (16, 20, 22, 24, 32):
        write_png(f"brand/tray/color/tray_icon_{n}.png", f"src/tray-color-{n}.svg", n)
    for n in (16, 22, 32):
        source = "mark-mono.svg" if n == 22 else f"src/mono-{n}.svg"
        write_png(f"brand/tray/black/tray_icon_{n}.png", source, n)
        write_png(f"brand/tray/white/tray_icon_{n}.png", source, n, white=True)
    write_png("brand/tray/trayTemplate.png", "mark-mono.svg", 22)
    write_png("brand/tray/trayTemplate@2x.png", "src/mono-44.svg", 44)
    for n in (192, 512):
        write_png(f"flutter_app/web/icons/Icon-{n}.png", "mark.svg", n)
        write_png(f"flutter_app/web/icons/Icon-maskable-{n}.png", "src/mark-maskable.svg", n)
    write_png("flutter_app/web/favicon.png", "src/mark-32.svg", 32)
    for n in (16, 32, 64, 128, 256, 512, 1024):
        source = f"src/mark-macos-{n}.svg" if n <= 64 else "src/mark-macos.svg"
        write_png(f"flutter_app/macos/Runner/Assets.xcassets/AppIcon.appiconset/app_icon_{n}.png", source, n)
    for n in (16, 48, 128):
        source = f"src/mark-{n}.svg" if n <= 48 else "mark.svg"
        write_png(f"chrome-ext/icons/icon-{n}.png", source, n)
    write_ico("brand/tray/tray_icon.ico", (16, 20, 24, 32), tray=True)
    write_ico("flutter_app/windows/runner/resources/app_icon.ico", (16, 20, 24, 32, 48, 256))
    shutil.copyfile(BRAND / "tray/color/tray_icon_22.png", REPO / "flutter_app/assets/tray_icon.png")
    shutil.copyfile(BRAND / "tray/tray_icon.ico", REPO / "flutter_app/assets/tray/tray_icon.ico")
    shutil.copyfile(BRAND / "tray/trayTemplate@2x.png", REPO / "flutter_app/assets/tray/trayTemplate@2x.png")
    shutil.copyfile(BRAND / "src/mark-24.svg", REPO / "oss/website/overrides/assets/upeg/favicon.svg")
    logo = (BRAND / "src/mark-24.svg").read_text()
    assert logo.count('aria-label="upeg"') == 1
    logo = logo.replace('aria-label="upeg"', 'aria-label="{{ config.site_name }}" width="24" height="24"')
    (REPO / "oss/website/overrides/partials/logo.html").write_text(
        '{#- Generated from brand/src/mark-24.svg by brand/src/render.py. -#}\n' + logo)
    write_sway_frames()


if __name__ == "__main__":
    main()
