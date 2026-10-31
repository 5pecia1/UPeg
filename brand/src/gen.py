"""upeg final logo generator — SSOT for every SVG in this folder.

    python3 src/gen.py            # (re)writes ../*.svg and src/*.svg

Flat SVG only: rect / circle / path, fill + evenodd. No stroke, transform,
opacity, gradient, filter, mask, text or style. Root has viewBox only.

Construction (512 canvas): a 3x3 board-hole grid with pitch 104 centred on
(256,256). The lowercase u is the part of that board left after removing the
middle column's top two cells; its stroke centre-lines run through the grid
points. The orange pin occupies the upper-right point; holes occupy the middle
left, bottom centre and spur. Every U mark rotates counterclockwise 40.5 degrees.
Small hints fit their static or fixed-attachment motion bounds only where needed.
Wordmark letters stay upright. Monochrome tray marks have no holes or pin.
"""
import math
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent

INK = "#14171C"     # app dark `surface` token (backdrop)
BONE = "#F4F3EE"    # app light `bg` token (board)
ORANGE = "#FF7A45"  # pin (inherited from the web favicon)
BLACK, WHITE = "#000000", "#FFFFFF"
SYMBOLIC = "#2E3436"  # GNOME symbolic convention (recoloured by GTK)

# ---- main mark spec (512 units) ------------------------------------------
W, P, RC = 96, 104, 32          # stroke, grid pitch, arm-top & spur corner radius
T = 256 - P - W / 2             # 104: top of arms; u box 104..408
HOLE_R, PIN_R = 18, 26          # board-hole and pin radius (rims 30 / 22 >= 512/24)
HOLES = ("ml", "bc", "br")      # upper-right point holds the pin
PIN = "tr"
TILT = -40.5
BG_RX = 112


def f(v):
    s = f"{v:.3f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def circle_sub(cx, cy, r):
    return (f"M{f(cx - r)} {f(cy)}a{f(r)} {f(r)} 0 1 0 {f(2 * r)} 0"
            f"a{f(r)} {f(r)} 0 1 0 {f(-2 * r)} 0Z")


def rect_sub(x, y, w, h):
    return f"M{f(x)} {f(y)}h{f(w)}v{f(h)}h{f(-w)}Z"


class U:
    """Lowercase u in arbitrary units.
    x0,y0: top-left of the u box; s: stroke; c: counter width; rc: corner radius.
    Square box (2s+c). Round bowl on the left/bottom, square spur bottom-right."""

    def __init__(self, x0, y0, s, c, rc):
        self.x0, self.y0, self.s, self.c, self.rc = x0, y0, s, c, rc
        self.size = 2 * s + c
        self.L, self.R = x0, x0 + self.size
        self.T, self.B = y0, y0 + self.size
        self.Ro, self.Ri = self.size / 2, c / 2
        self.cx = x0 + self.Ro
        self.cy = self.B - self.Ro
        self.p = (s + c) / 2            # distance arm centre-line -> u centre

    def pts(self):
        xl, xr, cx, cy, p = self.x0 + self.s / 2, self.R - self.s / 2, self.cx, self.cy, self.p
        return {"tl": (xl, self.T + self.s / 2), "tr": (xr, self.T + self.s / 2), "ml": (xl, cy),
                "bc": (cx, cy + p), "br": (xr, cy + p)}

    def outline(self):
        L, T, s, rc, R, B = self.L, self.T, self.s, self.rc, self.R, self.B
        cy, Ri, Ro = self.cy, self.Ri, self.Ro
        yc = cy + math.sqrt(Ro * Ro - Ri * Ri)   # bowl meets the spur stem
        arc = lambda r, x, y, sweep=1: f"A{f(r)} {f(r)} 0 0 {sweep} {f(x)} {f(y)}" if r else ""
        return "".join([
            f"M{f(L)} {f(T + rc)}", arc(rc, L + rc, T), f"H{f(L + s - rc)}", arc(rc, L + s, T + rc),
            f"V{f(cy)}", f"A{f(Ri)} {f(Ri)} 0 0 0 {f(R - s)} {f(cy)}",
            f"V{f(T + rc)}", arc(rc, R - s + rc, T), f"H{f(R - rc)}", arc(rc, R, T + rc),
            f"V{f(B - rc)}", arc(rc, R - rc, B), f"H{f(R - s)}", f"V{f(yc)}",
            f"A{f(Ro)} {f(Ro)} 0 0 1 {f(L)} {f(cy)}", "Z"])


def svg(w, h, body, label="upeg"):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {f(w)} {f(h)}" '
            f'role="img" aria-label="{label}">\n{body}</svg>\n')


def main_u(scale=1.0, ox=0.0, oy=0.0):
    k = scale
    return U(ox + T * k, oy + T * k, W * k, (2 * P - W) * k, RC * k)


def rotated_bounds(u, angle):
    """Exact bounds of the circular U outline rotated about its TR grid point."""
    a = math.radians(angle)
    pivot = u.pts()[PIN]

    def rotate(point):
        x, y = point[0]-pivot[0], point[1]-pivot[1]
        return pivot[0]+x*math.cos(a)-y*math.sin(a), pivot[1]+x*math.sin(a)+y*math.cos(a)

    L, R, T, B, s, rc = u.L, u.R, u.T, u.B, u.s, u.rc
    yc = u.cy + math.sqrt(u.Ro*u.Ro-u.Ri*u.Ri)
    arcs = ((L+rc, T+rc, rc, 180, 270), (L+s-rc, T+rc, rc, 270, 360),
            (u.cx, u.cy, u.Ri, 0, 180),
            (R-s+rc, T+rc, rc, 180, 270), (R-rc, T+rc, rc, 270, 360),
            (R-rc, B-rc, rc, 0, 90),
            (u.cx, u.cy, u.Ro, math.degrees(math.acos(u.Ri/u.Ro)), 180))
    points = [(R-s, B), (R-s, yc)]
    for cx, cy, radius, start, end in arcs:
        angles = [start, end] + [(v-angle) % 360 for v in (0, 90, 180, 270)
                                if start <= (v-angle) % 360 <= end]
        points.extend((cx+radius*math.cos(math.radians(v)), cy+radius*math.sin(math.radians(v))) for v in angles)
    xs, ys = zip(*(rotate(p) for p in points))
    return min(xs), min(ys), max(xs), max(ys)


def mark_geometry(u, angle):
    """Bake a rotation and centre the rest silhouette once, never per frame."""
    if not angle:
        return u.outline(), u.pts()
    a = math.radians(angle)
    pivot = u.pts()[PIN]
    def rotate(point):
        x, y = point[0]-pivot[0], point[1]-pivot[1]
        return pivot[0]+x*math.cos(a)-y*math.sin(a), pivot[1]+x*math.sin(a)+y*math.cos(a)
    left, top, right, bottom = rotated_bounds(u, angle)
    dx, dy = u.cx-(left+right)/2, u.cy-(top+bottom)/2

    def position(point):
        x, y = rotate(point)
        return x+dx, y+dy

    xy = lambda point: " ".join(f(v) for v in position(point))
    tokens = iter(re.findall(r"[MHVAZ]|-?\d+(?:\.\d+)?", u.outline()))
    x = y = 0
    out = []
    for command in tokens:
        if command == "M":
            x, y = float(next(tokens)), float(next(tokens))
            out.append("M" + xy((x, y)))
        elif command in ("H", "V"):
            if command == "H":
                x = float(next(tokens))
            else:
                y = float(next(tokens))
            out.append("L" + xy((x, y)))
        elif command == "A":
            rx, ry, axis, large, sweep, x, y = (float(next(tokens)) for _ in range(7))
            assert rx == ry and axis == 0, "Only circular arcs are supported"
            out.append(f"A{f(rx)} {f(ry)} 0 {int(large)} {int(sweep)} " + xy((x, y)))
        elif command == "Z":
            out.append("Z")
        else:
            raise ValueError(command)
    return "".join(out), {key: position(point) for key, point in u.pts().items()}


def optical_u(N, spec, *, inset=0, pin_inset=0, angle=None, mono=False, motion=False):
    """Fit small hints only where their static or +/-6deg motion bounds need room.

    Preserve the U's proportions and the pin's hinted pixel size. Fit the whole
    motion envelope once; each frame subsequently uses identical geometry.
    The 22/44px template contract uses proportional margins and identical paths.
    """
    angle = TILT if angle is None else angle
    s, c, x0, rc = (spec[key] for key in ('s', 'c', 'x0', 'rc'))
    centre = x0+(2*s+c)/2
    margin = inset + ((N/44) if mono and N in (22,44) else .5) + .01*N/22
    factor = 1.
    for _ in range(5):
        size = (2*s+c)*factor
        u = U(centre-size/2, centre-size/2, s*factor, c*factor, rc*factor)
        rest = rotated_bounds(u, angle)
        shift = (centre-(rest[0]+rest[2])/2, centre-(rest[1]+rest[3])/2)
        a = math.radians(angle)
        vx, vy = -math.sin(a)*pin_inset, math.cos(a)*pin_inset
        radial = 0
        for step in (range(-24,25) if motion else (0,)):
            delta=step/4; da=math.radians(delta)
            correction=(vx-vx*math.cos(da)+vy*math.sin(da), vy-vx*math.sin(da)-vy*math.cos(da))
            bounds=rotated_bounds(u, angle+delta)
            for index, value in enumerate(bounds):
                axis=index%2
                radial=max(radial, abs(value+shift[axis]+correction[axis]-centre))
        allowed=min(centre-margin, N-margin-centre)
        if radial <= allowed+1e-9:
            break
        factor *= allowed/radial
    size=(2*s+c)*factor
    u=U(centre-size/2,centre-size/2,s*factor,c*factor,rc*factor)
    k=512/N
    return U(u.x0*k,u.y0*k,u.s*k,u.c*k,u.rc*k)


def color_body(scale=1.0, ox=0.0, oy=0.0, bg="rounded", bg_box=None):
    """Backdrop + bone u with board holes + orange pin; transform baked into coords."""
    k = scale
    u = main_u(k, ox, oy)
    outline, pts = mark_geometry(u, TILT)
    if bg == "rounded":
        x, y, w, r = (bg_box or (ox, oy, 512 * k, BG_RX * k))
        t = f'  <rect x="{f(x)}" y="{f(y)}" width="{f(w)}" height="{f(w)}" rx="{f(r)}" fill="{INK}"/>\n'
    elif bg == "bleed":
        t = f'  <rect width="{f(512 * k)}" height="{f(512 * k)}" fill="{INK}"/>\n'
    else:
        t = ""
    d = outline + "".join(circle_sub(*pts[h], HOLE_R * k) for h in HOLES)
    bx, by = pts[PIN]
    return (t + f'  <path fill="{BONE}" fill-rule="evenodd" d="{d}"/>\n'
            f'  <circle cx="{f(bx)}" cy="{f(by)}" r="{f(PIN_R * k)}" fill="{ORANGE}"/>\n')


# ---- size-specific hinted colour masters (pixel units -> 512 units) -------
# N: stroke s, counter c, left/top x0, board holes?, hole Ø, pin (w,h), corner rc — all in px.
# u is ~70-75% of the canvas (vs 59% in the main mark): small sizes get a bigger glyph.
# Even s/c keep the pin centre on a pixel corner, so 2px squares and 4/6px discs land on whole pixels.
HINT = {
    16: dict(s=4, c=4, x0=2, holes=False, pin=(2, 2), rc=0.5),
    20: dict(s=4, c=6, x0=3, holes=False, pin=(2, 2), rc=0.5),
    22: dict(s=4, c=6, x0=4, holes=False, pin=(2, 2), rc=0.5),   # Linux colour tray
    24: dict(s=6, c=6, x0=3, holes=False, pin=(4, 4), rc=0.75),
    32: dict(s=8, c=8, x0=4, holes=False, pin=(4, 4), rc=1),
    48: dict(s=12, c=12, x0=6, holes=True, hole=4, pin=(6, 6), rc=1.5),
}


def hinted_body(N, spec, inset=0, bg_rx=None, tray=False, *, angle=None, fit=True):
    k = 512 / N
    s, c, x0, rc = spec["s"], spec["c"], spec["x0"], spec["rc"]
    angle = TILT if angle is None else angle
    pin_inset = 1 if tray and spec['pin'][0] < 4 else 0
    u = optical_u(N, spec, inset=inset, pin_inset=pin_inset, angle=angle, motion=tray) if fit else U(x0*k,x0*k,s*k,c*k,rc*k)
    outline, pts = mark_geometry(u, angle)
    w = (N - 2 * inset) * k
    rx = (bg_rx if bg_rx is not None else BG_RX / 512 * (N - 2 * inset)) * k
    body = f'  <rect x="{f(inset * k)}" y="{f(inset * k)}" width="{f(w)}" height="{f(w)}" rx="{f(rx)}" fill="{INK}"/>\n'
    d = outline
    if spec["holes"]:
        d += "".join(circle_sub(*pts[h], spec["hole"] / 2 * k) for h in HOLES)
    body += f'  <path fill="{BONE}" fill-rule="evenodd" d="{d}"/>\n'
    pw, ph = spec["pin"]
    bx, by = pts[PIN]
    if tray and pw < 4:
        # 16/20/22px square pins move 1px down the rotated arm for a clear Bone cap.
        a = math.radians(angle)
        bx -= math.sin(a)*k
        by += math.cos(a)*k
    if pw == ph == 1:
        # The native 16px macOS hint needs one orange pixel, rather than a
        # fractional square spread over four pale pixels after rotation.
        bx = (math.floor(bx/k)+.5)*k
        by = (math.floor(by/k)+.5)*k
    if pw >= 4:         # round pin
        body += f'  <circle cx="{f(bx)}" cy="{f(by)}" r="{f(pw / 2 * k)}" fill="{ORANGE}"/>\n'
    else:               # 1/2px: square pin stays crisp
        body += f'  <rect x="{f(bx - pw / 2 * k)}" y="{f(by - ph / 2 * k)}" width="{f(pw * k)}" height="{f(ph * k)}" fill="{ORANGE}"/>\n'
    return body


# macOS app-icon sizes: body inset like Apple's 824/1024 grid, glyph hinted inside.
MAC = {
    16: dict(inset=1, s=3, c=4, x0=3, holes=False, pin=(1, 1), rc=0.5), # Keep a visible 1px pin; rotation/fitting makes its position fractional.
    32: dict(inset=3, s=6, c=6, x0=7, holes=False, pin=(4, 4), rc=0.75),
    64: dict(inset=6, s=10, c=12, x0=16, holes=True, hole=4, pin=(6, 6), rc=1.5),
}


# ---- monochrome glyphs (pixel grids) --------------------------------------
# N: (s, c, x0, pin_ring) ; pin_ring = (hole_px, dot_px) knock-out ring + dot at the pin point
MONO = {
    16: dict(s=3, c=6, x0=2, rc=0.5, ring=None),
    22: dict(s=5, c=8, x0=2, rc=1, ring=None),
    32: dict(s=7, c=12, x0=3, rc=1, ring=None),
    44: dict(s=10, c=16, x0=4, rc=2, ring=None),
}


def mono_body(N, spec, color, unit=None, upright=False, fit=True):
    """Solid tilted tray mark; upright is reserved for historical review controls."""
    k = (512 / N) if unit is None else unit
    s, c, x0, rc = spec["s"], spec["c"], spec["x0"], spec["rc"]
    u = optical_u(N, spec, mono=True, motion=True) if fit and not upright else U(x0*k,x0*k,s*k,c*k,rc*k)
    if unit is not None and fit and not upright:
        u = U(u.x0/512*N*unit,u.y0/512*N*unit,u.s/512*N*unit,u.c/512*N*unit,u.rc/512*N*unit)
    d = u.outline() if upright else mark_geometry(u, TILT)[0]
    out = ""
    if spec["ring"]:
        hole, dot = spec["ring"]
        bx, by = u.pts()["bc"]
        d += circle_sub(bx, by, hole / 2 * k)
        out = f'  <circle cx="{f(bx)}" cy="{f(by)}" r="{f(dot / 2 * k)}" fill="{color}"/>\n'
    return f'  <path fill="{color}" fill-rule="evenodd" d="{d}"/>\n' + out


# ---- wordmark letters (geometric monoline, same u construction) ------------
def ring(cx, cy, Ro, Ri):
    return circle_sub(cx, cy, Ro) + circle_sub(cx, cy, Ri)


def band(cx, cy, Ro, Ri, a0, a1):
    pt = lambda R, a: f"{f(cx + R * math.cos(math.radians(a)))} {f(cy + R * math.sin(math.radians(a)))}"
    large = 1 if (a1 - a0) > 180 else 0
    return (f"M{pt(Ro, a0)}A{f(Ro)} {f(Ro)} 0 {large} 1 {pt(Ro, a1)}"
            f"L{pt(Ri, a1)}A{f(Ri)} {f(Ri)} 0 {large} 0 {pt(Ri, a0)}Z")


def rrect(x, y, w, h, r_tl, r_tr, r_br, r_bl):
    d = [f"M{f(x)} {f(y + r_tl)}"]
    if r_tl: d.append(f"A{f(r_tl)} {f(r_tl)} 0 0 1 {f(x + r_tl)} {f(y)}")
    d.append(f"H{f(x + w - r_tr)}")
    if r_tr: d.append(f"A{f(r_tr)} {f(r_tr)} 0 0 1 {f(x + w)} {f(y + r_tr)}")
    d.append(f"V{f(y + h - r_br)}")
    if r_br: d.append(f"A{f(r_br)} {f(r_br)} 0 0 1 {f(x + w - r_br)} {f(y + h)}")
    d.append(f"H{f(x + r_bl)}")
    if r_bl: d.append(f"A{f(r_bl)} {f(r_bl)} 0 0 1 {f(x)} {f(y + h - r_bl)}")
    d.append("Z")
    return "".join(d)


def letters(x, t, xh=216, sw=48, desc=96, rc=12, gaps=(44, 30, 30)):
    """Geometric monoline 'upeg'. One path per stroke part: parts overlap, and
    evenodd inside a single path would punch the overlaps out."""
    Ro = xh / 2
    out = [U(x, t, sw, xh - 2 * sw, rc).outline()]                     # u (same spur as the mark)
    x += xh + gaps[0]
    out += [rrect(x, t, sw, xh + desc, rc, 0, rc, rc),                  # p stem
            ring(x + Ro, t + Ro, Ro, Ro - sw)]                          # p bowl
    x += xh + gaps[1]
    cx, cy, Ri, bar = x + Ro, t + Ro, Ro - sw, sw * 0.8
    out += [band(cx, cy, Ro, Ri, 40, 360),                              # e bowl, 40deg aperture
            rrect(cx - Ri - 2, cy - bar / 2, Ri + Ro, bar, 0, 0, 0, 0)]  # e bar
    x += xh + gaps[2]
    cx, cy, yd = x + Ro, t + Ro, t + xh + desc - Ro
    out += [ring(cx, cy, Ro, Ro - sw),                                  # g bowl
            rrect(x + xh - sw, t, sw, yd - t + 1, 0, rc, 0, 0),         # g stem
            band(cx, yd, Ro, Ro - sw, 0, 150)]                          # g tail
    return out, x + xh


def wordmark(text_color):
    xh, gap = 216, 80
    k = xh / (2 * P + W)                      # unrotated icon U height == x-height
    icon = 512 * k
    text_w = 4 * xh + 44 + 30 + 30
    ox = (1600 - (icon + gap + text_w)) / 2
    oy = (512 - icon) / 2
    body = color_body(k, ox, oy)
    t = oy + T * k                            # x-height top == unrotated icon U top
    paths, _ = letters(ox + icon + gap, t)
    body += "".join(f'  <path fill="{text_color}" fill-rule="evenodd" d="{d}"/>\n' for d in paths)
    return svg(1600, 512, body)


def write(path, text):
    path.write_text(text)
    print("wrote", path.relative_to(ROOT))


def flutter_geometry():
    """Generate the startup painter's path directly from this mark's geometry."""
    outline, pts = mark_geometry(main_u(), TILT)
    tokens = iter(re.findall(r"[MLAZ]|-?\d+(?:\.\d+)?", outline))
    commands = []
    for command in tokens:
        if command in ("M", "L"):
            x, y = next(tokens), next(tokens)
            commands.append(f"    ..{'moveTo' if command == 'M' else 'lineTo'}({x}, {y})")
        elif command == "A":
            rx, ry, axis, large, sweep, x, y = (next(tokens) for _ in range(7))
            assert rx == ry and float(axis) == 0
            commands.append("    ..arcToPoint(\n"
                            f"      const Offset({x}, {y}),\n"
                            f"      radius: const Radius.circular({rx}),\n"
                            f"      largeArc: {'true' if int(large) else 'false'},\n"
                            f"      clockwise: {'true' if int(sweep) else 'false'},\n"
                            "    )")
        elif command == "Z":
            commands.append("    ..close()")
    for hole in HOLES:
        x, y = pts[hole]
        commands.append("    ..addOval(\n"
                        f"      Rect.fromCircle(center: const Offset({f(x)}, {f(y)}), radius: {HOLE_R}),\n"
                        "    )")
    x, y = pts[PIN]
    return ("// Generated by brand/src/gen.py. Regenerate; do not edit.\n"
            "import 'dart:ui';\n\n"
            "const double upegLogoViewBoxSize = 512;\n"
            f"const double upegLogoPlateRadius = {BG_RX};\n"
            f"const double upegLogoRestAngleDegrees = {f(TILT)};\n"
            f"const int upegLogoInkArgb = 0xFF{INK[1:]};\n"
            f"const int upegLogoBoneArgb = 0xFF{BONE[1:]};\n"
            f"const int upegLogoOrangeArgb = 0xFF{ORANGE[1:]};\n"
            f"const Offset upegLogoPinCenter = Offset({f(x)}, {f(y)});\n"
            f"const double upegLogoPinRadius = {PIN_R};\n\n"
            "// Rest pose is already baked in. Rotate only this path around the fixed\n"
            "// pin by a transient delta; keep the plate and orange pin stationary.\n"
            "Path buildUpegLogoPath() {\n"
            "  return Path()\n    ..fillType = PathFillType.evenOdd\n" + "\n".join(commands) + ";\n}\n")


def main():
    # deliverables (top level)
    write(ROOT / "mark.svg", svg(512, 512, color_body()))
    write(ROOT / "mark-mono.svg", svg(512, 512, mono_body(22, MONO[22], BLACK)))
    write(ROOT / "mark-mono-white.svg", svg(512, 512, mono_body(22, MONO[22], WHITE)))
    write(ROOT / "wordmark.svg", wordmark(INK))
    # variants / size-specific sources
    write(HERE / "wordmark-dark.svg", wordmark(BONE))
    write(HERE / "mark-maskable.svg", svg(512, 512, color_body(bg="bleed")))
    # macOS: 1024 canvas, 824 body inset 100 (Apple icon grid), corner ~22.5%
    write(HERE / "mark-macos.svg", svg(1024, 1024, color_body(824 / 512, 100, 100, bg_box=(100, 100, 824, 185))))
    for n, spec in HINT.items():
        write(HERE / f"mark-{n}.svg", svg(512, 512, hinted_body(n, spec)))
    for n in (16, 20, 22, 24, 32):
        write(HERE / f"tray-color-{n}.svg", svg(512, 512, hinted_body(n, HINT[n], tray=True)))
    for n, spec in MAC.items():
        write(HERE / f"mark-macos-{n}.svg", svg(512, 512, hinted_body(n, spec, inset=spec["inset"])))
    for n, spec in MONO.items():
        if n != 22:
            write(HERE / f"mono-{n}.svg", svg(512, 512, mono_body(n, spec, BLACK)))
    # Linux symbolic: native 16 grid, viewBox 0 0 16 16
    write(HERE / "upeg-symbolic.svg", svg(16, 16, mono_body(16, MONO[16], SYMBOLIC, unit=1)))
    dart = ROOT.parent / "flutter_app/lib/src/brand/upeg_logo_geometry.g.dart"
    dart.parent.mkdir(parents=True, exist_ok=True)
    dart.write_text(flutter_geometry())
    print("wrote", dart.relative_to(ROOT.parent))


if __name__ == "__main__":
    main()
