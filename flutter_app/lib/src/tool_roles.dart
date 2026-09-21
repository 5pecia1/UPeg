/// Metadata-derived tool roles.
///
/// Behaviour branches on typed `ToolDto` metadata (pin kind, invoker,
/// source, output field kinds) — never on a literal tool-id or toolkit
/// string. Adding a new tool that matches a role gets the behaviour for
/// free, and no id list drifts out of sync (project rule: generic over
/// example).
library;

import 'package:upeg/src/rust/api/tools.dart';

/// Catalog key of the message shown when the user tries to run a pin
/// that has no configured provider (see [toolNeedsProviderConfig]).
/// The En/Ko copy lives in the Rust catalog
/// (upeg-pegboard-ui/src/i18n.rs) and renders through `t()`/`tRead()`.
const String providerNotConfiguredMessageKey =
    'pin.provider_not_configured.message';

/// Whether [tool] advertises live external data but has no configured
/// provider, so it cannot actually run.
///
/// Signal: a `Live` pin whose invoker is `http` (it wants to fetch from
/// a network provider) yet whose source is `static` (a placeholder — no
/// endpoint / API key wired up). Such a pin must show a "needs setup"
/// (provider not configured) state instead of a runnable affordance, and
/// activating it yields a clear message rather than a generic dispatch
/// failure.
///
/// `eth.gas` is today's only match, but the rule keys off invoker/source
/// metadata so any future "live http, no provider" tool is honest by
/// construction.
bool toolNeedsProviderConfig(ToolDto tool) {
  return tool.pinKind == PinKindDto.live &&
      tool.invoker == InvokerDto.http &&
      tool.source is SourceDto_Static;
}
