/// Manifest footprint helper for [PegboardUnitsDto].
///
/// Single shared mapping from the manifest units enum to the cell
/// footprint `(cols, rows)` — u1 = 1x1, u2 = 2x1, u2T = 1x2. Lives in
/// its own tiny library (not a widget file) so state machines and
/// widgets share one source instead of re-deriving the pair.
library;

import 'package:upeg/src/rust/api/tools.dart';

extension PegboardUnitsFootprint on PegboardUnitsDto {
  /// Cell footprint declared by the tool manifest.
  ({int cols, int rows}) get footprint => switch (this) {
    PegboardUnitsDto.u1 => (cols: 1, rows: 1),
    PegboardUnitsDto.u2 => (cols: 2, rows: 1),
    PegboardUnitsDto.u2T => (cols: 1, rows: 2),
  };
}
