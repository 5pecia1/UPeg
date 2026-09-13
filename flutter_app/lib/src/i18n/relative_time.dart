/// Pure relative-time bucketing for the pin "last run" badge.
///
/// Maps an elapsed [Duration] to the matching `pin.last_run.*` catalog
/// key plus its placeholder args; the caller renders it through `t()`.
/// Kept as a pure function (no clock, no ref) so the bucket boundaries
/// are unit-testable without widgets.
library;

/// A translation request: catalog [key] plus `{placeholder}` [args].
typedef RelativeTimeLabel = ({String key, Map<String, String> args});

/// Bucket [elapsed] into the coarsest human-friendly unit:
/// under a minute → "just now", then minutes, hours, days. Negative
/// durations (clock skew) clamp to "just now" instead of predicting
/// the future.
RelativeTimeLabel lastRunAgoLabel(Duration elapsed) {
  if (elapsed.inMinutes < 1) {
    return (key: 'pin.last_run.just_now', args: const <String, String>{});
  }
  if (elapsed.inHours < 1) {
    return (
      key: 'pin.last_run.minutes_ago',
      args: <String, String>{'minutes': '${elapsed.inMinutes}'},
    );
  }
  if (elapsed.inDays < 1) {
    return (
      key: 'pin.last_run.hours_ago',
      args: <String, String>{'hours': '${elapsed.inHours}'},
    );
  }
  return (
    key: 'pin.last_run.days_ago',
    args: <String, String>{'days': '${elapsed.inDays}'},
  );
}
