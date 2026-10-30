# upeg brand assets

`src/gen.py` is the source for the mark and wordmark SVGs. Run `python3 brand/src/gen.py` from the repository root to regenerate them. Run `python3 brand/src/render.py` with CairoSVG installed to regenerate all colour and monochrome PNG/ICO consumers, tray templates and website SVG copies as well. CairoSVG is a development tool, not an app dependency. ICO frames are rendered independently from their matching size hints.

| Source | Use |
|---|---|
| `mark.svg`, `src/mark-{16,20,22,24,32,48}.svg` | Main mark and integer-grid hints for small sizes |
| `src/tray-color-{16,20,22,24,32}.svg` | Tilted colour tray hints, separate from small app icons |
| `src/mark-macos-{16,32,64}.svg`, `src/mark-macos.svg` | macOS icon sizes |
| `mark-mono.svg`, `src/mono-*.svg` | Monochrome tray marks |
| `wordmark.svg` | Lowercase `upeg` wordmark |
| `tray/color/`, `tray/black/`, `tray/white/` | Color and monochrome tray rasters |
| `tray/trayTemplate@2x.png`, `tray/tray_icon.ico` | macOS and Windows tray formats |

Use each hinted SVG at its named size; do not downscale the 22px glyph to 16px. The 44px monochrome menu bar SVG uses the 22px solid U geometry at exactly twice the pixel size, without a pin ring. Ink is `#14171C`, Bone is `#F4F3EE`, and Orange is `#FF7A45`.

All U marks lean counterclockwise 40.5° (−40.5°) around the upper-right attachment. The main 512px mark retains its original U size and stroke. Small app, website, extension and tray hints use the same resting angle, with a uniform fit only where needed to avoid clipping. Static app hints fit their resting silhouette; tray hints reserve the full ±6° swing around a fixed attachment. Resting silhouette bounds are centred once. The three visible holes occupy the middle-left, bottom-centre and bottom-right grid points; small hints omit them. Wordmark letters remain upright. The user selected this pose on 2026-10-17 to prioritize the U hanging beneath the pin; see the [current production proof](../docs/plans/logo-tilt-20261016/balance-final-production-20261017.png).

Tray colour marks at 16/20/22/24/32px have no board holes. Their 2px square pins at 16/20/22px move 1px inward along the rotated arm; animation uses that displayed pin as its fixed pivot. The 24/32px round pins remain at the transformed grid points. The macOS 16px app hint snaps its 1px square pin to the nearest pixel centre on the rotated arm to retain a clear orange pixel, as shown in the native-size proof. Monochrome tray and symbolic marks use a solid U without holes or a pin, rotating about a fixed hidden upper-right attachment. The 22px and 44px monochrome SVGs have identical normalized geometry, so the 44px template is exactly twice the physical size at 22 logical points. See the [motion and centroid proof](../docs/plans/logo-tilt-20261016/balance-final-technical-20261017.png).

The rendered mark supplies the web icons, macOS AppIcon, and Windows ICO. `flutter_app/assets/tray_icon.png` is byte-identical to `tray/color/tray_icon_22.png`; `flutter_app/assets/tray/*` holds the macOS and Windows tray formats. The macOS tray code uses `trayTemplate@2x.png` with `isTemplate: true` and a 22-point icon size. The Linux release build runs CMake, which installs `flutter_app/web/icons/Icon-192.png` as `data/app_icon.png` for the window icon. The local desktop entry installed by `just flutter-desktop-entry-linux` uses the absolute path to `flutter_app/web/icons/Icon-512.png`.

`src/gen.py` also generates `flutter_app/lib/src/brand/upeg_logo_geometry.g.dart` for the startup painter. Its path includes the resting rotation and board holes. Motion rotates only this path by a temporary angle around the generated pin centre; the plate and orange pin stay fixed. Resting bounds are centred once, never separately for animation frames.

`src/render.py` generates nine tray motion frames in `flutter_app/assets/tray/sway/{linux,macos,windows}/`. Linux uses the 22px colour hint, macOS the 44px monochrome template, and Windows independently renders 16/20/24/32px ICO frames. Their short angle sequence ends at the static asset; its Linux and macOS pixels and Windows ICO bytes match the corresponding static files. These frame assets supply a single brief swing when the app requests one.
