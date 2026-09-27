/// Directional keyboard focus over a board's placements.
///
/// Pure geometry: given the visible placements, the currently focused
/// one and a [DirectionDto], pick the pin the arrow keys should land on.
/// A primary pass looks for a strictly aligned neighbour (row/column
/// ranges overlap); the fallback widens to any placement lying in that
/// direction, ordered by gap, centre distance, then row-major position.
library;

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/keyboard.dart' show DirectionDto;
import 'package:upeg/src/rust/api/pegboard.dart';

PlacementDto? placementForTool(List<PlacementDto> placements, PinId toolId) {
  for (final placement in placements) {
    if (placement.pinId == toolId.value) return placement;
  }
  return null;
}

PlacementDto? spatialPlacementCandidate(
  List<PlacementDto> placements,
  PlacementDto current,
  DirectionDto direction,
) {
  final primary = _primarySpatialCandidate(placements, current, direction);
  return primary ?? _fallbackSpatialCandidate(placements, current, direction);
}

PlacementDto? _primarySpatialCandidate(
  List<PlacementDto> placements,
  PlacementDto current,
  DirectionDto direction,
) {
  final candidates = placements.where((placement) {
    return switch (direction) {
      DirectionDto.left =>
        _rowRangesOverlap(placement, current) &&
            placement.x + placement.w <= current.x,
      DirectionDto.right =>
        _rowRangesOverlap(placement, current) &&
            placement.x >= current.x + current.w,
      DirectionDto.up =>
        _colRangesOverlap(placement, current) &&
            placement.y + placement.h <= current.y,
      DirectionDto.down =>
        _colRangesOverlap(placement, current) &&
            placement.y >= current.y + current.h,
    };
  });
  return switch (direction) {
    DirectionDto.left => _maxByScore(
      candidates,
      (placement) => (placement.x + placement.w, placement.y, 0, 0, 0),
    ),
    DirectionDto.right => _minByScore(
      candidates,
      (placement) => (placement.x, placement.y, 0, 0, 0),
    ),
    DirectionDto.up => _maxByScore(
      candidates,
      (placement) => (placement.y + placement.h, placement.x, 0, 0, 0),
    ),
    DirectionDto.down => _minByScore(
      candidates,
      (placement) => (placement.y, placement.x, 0, 0, 0),
    ),
  };
}

PlacementDto? _fallbackSpatialCandidate(
  List<PlacementDto> placements,
  PlacementDto current,
  DirectionDto direction,
) {
  return _minByScore(
    placements.where(
      (placement) =>
          placement.pinId != current.pinId &&
          _placementIsInDirection(placement, current, direction),
    ),
    (placement) => _fallbackSpatialScore(placement, current, direction),
  );
}

typedef _PlacementScore = (int, int, int, int, int);

PlacementDto? _minByScore(
  Iterable<PlacementDto> placements,
  _PlacementScore Function(PlacementDto placement) scoreFor,
) {
  PlacementDto? best;
  _PlacementScore? bestScore;
  for (final placement in placements) {
    final score = scoreFor(placement);
    if (bestScore == null || _compareScore(score, bestScore) < 0) {
      best = placement;
      bestScore = score;
    }
  }
  return best;
}

PlacementDto? _maxByScore(
  Iterable<PlacementDto> placements,
  _PlacementScore Function(PlacementDto placement) scoreFor,
) {
  PlacementDto? best;
  _PlacementScore? bestScore;
  for (final placement in placements) {
    final score = scoreFor(placement);
    if (bestScore == null || _compareScore(score, bestScore) > 0) {
      best = placement;
      bestScore = score;
    }
  }
  return best;
}

int _compareScore(_PlacementScore left, _PlacementScore right) {
  final a = [left.$1, left.$2, left.$3, left.$4, left.$5];
  final b = [right.$1, right.$2, right.$3, right.$4, right.$5];
  for (var i = 0; i < a.length; i++) {
    final cmp = a[i].compareTo(b[i]);
    if (cmp != 0) return cmp;
  }
  return 0;
}

bool _rowRangesOverlap(PlacementDto a, PlacementDto b) =>
    _rangesOverlap(a.y, a.h, b.y, b.h);

bool _colRangesOverlap(PlacementDto a, PlacementDto b) =>
    _rangesOverlap(a.x, a.w, b.x, b.w);

bool _rangesOverlap(int aStart, int aSpan, int bStart, int bSpan) =>
    aStart < bStart + bSpan && bStart < aStart + aSpan;

bool _placementIsInDirection(
  PlacementDto placement,
  PlacementDto current,
  DirectionDto direction,
) {
  return switch (direction) {
    DirectionDto.left => placement.x + placement.w <= current.x,
    DirectionDto.right => placement.x >= current.x + current.w,
    DirectionDto.up => placement.y + placement.h <= current.y,
    DirectionDto.down => placement.y >= current.y + current.h,
  };
}

_PlacementScore _fallbackSpatialScore(
  PlacementDto placement,
  PlacementDto current,
  DirectionDto direction,
) {
  return switch (direction) {
    DirectionDto.left => (
      current.x - (placement.x + placement.w),
      _centerDistance(_rowCenter2(placement), _rowCenter2(current)),
      current.x - placement.x,
      placement.y,
      0,
    ),
    DirectionDto.right => (
      placement.x - (current.x + current.w),
      _centerDistance(_rowCenter2(placement), _rowCenter2(current)),
      placement.x,
      placement.y,
      0,
    ),
    DirectionDto.up => (
      current.y - (placement.y + placement.h),
      _centerDistance(_colCenter2(placement), _colCenter2(current)),
      current.y - placement.y,
      placement.x,
      0,
    ),
    DirectionDto.down => (
      placement.y - (current.y + current.h),
      _centerDistance(_colCenter2(placement), _colCenter2(current)),
      placement.y,
      placement.x,
      0,
    ),
  };
}

int _colCenter2(PlacementDto placement) => placement.x * 2 + placement.w;

int _rowCenter2(PlacementDto placement) => placement.y * 2 + placement.h;

int _centerDistance(int a, int b) => (a - b).abs();
