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
TRAY_SWAY_DELTAS = (9, 2, -4, -2.2, 1, 1.1, 0, -0.3, 0)


def tray_png(source, size, delta=0, template=False, unread=False, dark=False):
    """Render a fixed-pivot through-hole and optional unread pin."""
    text = (BRAND / source).read_text()
    root = ET.fromstring(text)
    path = next(node for node in root if node.tag.endswith('path'))
    if template:
        u = gen.optical_u(44, gen.MONO[44], mono=True, motion=True)
        _, pts = gen.mark_geometry(u, gen.TILT)
        px, py = pts[gen.PIN]
        radius = 1.9 * 512 / 44
        if dark:
            path.set('fill', gen.WHITE)
    else:
        pin = next(node for node in root if node.attrib.get('fill') == gen.ORANGE)
        if pin.tag.endswith('circle'):
            px, py, radius = (float(pin.attrib[k]) for k in ('cx', 'cy', 'r'))
        else:
            radius = float(pin.attrib['width']) / 2
            px = float(pin.attrib['x']) + radius
            py = float(pin.attrib['y']) + float(pin.attrib['height']) / 2
        root.remove(pin)
    path.set('d', path.attrib['d'] + gen.circle_sub(px, py, radius))
    path.set('transform', f'rotate({delta} {gen.f(px)} {gen.f(py)})')
    if unread:
        ET.SubElement(root, '{http://www.w3.org/2000/svg}circle', {
            'cx': gen.f(px), 'cy': gen.f(py), 'r': gen.f(radius), 'fill': gen.ORANGE})
    return cairosvg.svg2png(bytestring=ET.tostring(root), output_width=size, output_height=size)


def png(source, size, white=False):
    text = (BRAND / source).read_text()
    if white:
        text = text.replace('fill="#000000"', 'fill="#FFFFFF"')
    return cairosvg.svg2png(bytestring=text.encode(), output_width=size, output_height=size)


def write_png(target, source, size, white=False):
    (REPO / target).write_bytes(png(source, size, white))


def write_ico(target, sizes, tray=False, delta=None, unread=False):
    # Embed one independently hinted PNG per frame; never downscale another frame.
    frames = []
    for n in sizes:
        source = f"src/tray-color-{n}.svg" if tray else f"src/mark-{n}.svg" if n <= 48 else "mark.svg"
        frames.append(tray_png(source, n, delta or 0, unread=unread) if tray else png(source, n))
    offset = 6 + 16 * len(sizes)
    directory = []
    for n, data in zip(sizes, frames):
        directory.append(struct.pack("<BBBBHHII", n % 256, n % 256, 0, 0, 1, 32, len(data), offset))
        offset += len(data)
    (REPO / target).write_bytes(struct.pack("<HHH", 0, 1, len(sizes)) + b"".join(directory) + b"".join(frames))


def write_sway_frames():
    base = REPO / 'flutter_app/assets/tray/sway'
    for platform in ('linux', 'macos', 'windows'):
        for stale in (base / platform).glob('frame_*_unread*'):
            stale.unlink()
    for frame, delta in enumerate(TRAY_SWAY_DELTAS):
        (base / 'linux' / f'frame_{frame}.png').write_bytes(
            tray_png('src/tray-color-22.svg', 22, delta))
        write_ico(f'flutter_app/assets/tray/sway/windows/frame_{frame}.ico',
                  (16, 20, 24, 32), tray=True, delta=delta)
        (base / 'macos' / f'frame_{frame}.png').write_bytes(
            tray_png('src/mono-44.svg', 44, delta, template=True))


def main():
    gen.main()
    for n in (16, 20, 22, 24, 32):
        (BRAND / f"tray/color/tray_icon_{n}.png").write_bytes(tray_png(f"src/tray-color-{n}.svg", n))
    for n in (16, 22, 32):
        source = "mark-mono.svg" if n == 22 else f"src/mono-{n}.svg"
        write_png(f"brand/tray/black/tray_icon_{n}.png", source, n)
        write_png(f"brand/tray/white/tray_icon_{n}.png", source, n, white=True)
    (BRAND / "tray/trayTemplate.png").write_bytes(tray_png("src/mono-44.svg", 22, template=True))
    (BRAND / "tray/trayTemplate@2x.png").write_bytes(tray_png("src/mono-44.svg", 44, template=True))
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
    (REPO / "flutter_app/assets/tray_icon_unread.png").write_bytes(tray_png("src/tray-color-22.svg", 22, unread=True))
    write_ico("flutter_app/assets/tray/tray_icon_unread.ico", (16, 20, 24, 32), tray=True, unread=True)
    for theme in ('light', 'dark'):
        (REPO / f"flutter_app/assets/tray/trayUnread_{theme}@2x.png").write_bytes(
            tray_png("src/mono-44.svg", 44, template=True, unread=True, dark=theme == 'dark'))
    write_sway_frames()


if __name__ == "__main__":
    main()
