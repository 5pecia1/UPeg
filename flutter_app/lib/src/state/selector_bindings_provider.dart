/// Controlled-embed selector bindings seams (J06/J07).
///
/// Two overridable providers split read from write so widget tests
/// can stub either independently:
///
///   * [`selectorBindingsLoaderProvider`] — reads the persisted rows
///     for a given tool id. Production wraps the FRB
///     `selectorBindingsFor(toolId:)` symbol.
///   * [`selectorBindingsSaverProvider`] — persists the new rows.
///     Production wraps the FRB `setSelectorBindings(toolId:, bindings:)`
///     symbol; the FRB binding returns `void` and throws on the rare
///     non-canonical-id error, so the saver mirrors that contract.
///
/// Type system: both seams traffic in the FRB-generated concrete
/// `SelectorBindingDto` struct — no `Map<String, dynamic>` reaches the
/// surface. SoC: the providers live separately from the widget so the
/// section under `widgets/expanded_modal/` stays focused on rendering.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/embed.dart' as frb;

/// Returns the persisted selector-binding rows for [toolId]. The
/// FRB symbol itself is synchronous, but exposing the seam as a
/// plain function (not Future) keeps test overrides one-liners
/// (`(id) => [...]`).
typedef SelectorBindingsLoader =
    List<frb.SelectorBindingDto> Function(ToolId toolId);

/// Persists [bindings] for [toolId]. Returns nothing; surface
/// failure as a thrown exception so widget callers can route the
/// error into a `SnackBar`. The FRB binding throws `FrbError::Internal`
/// when the id is not canonical — that exception propagates up
/// unchanged.
typedef SelectorBindingsSaver =
    void Function(ToolId toolId, List<frb.SelectorBindingDto> bindings);

/// Production binding — wraps `frb.selectorBindingsFor`. Tests
/// override with a static map per fixture so the dylib never loads.
final selectorBindingsLoaderProvider = Provider<SelectorBindingsLoader>(
  (ref) =>
      (toolId) => frb.selectorBindingsFor(toolId: toolId.value),
);

/// Production binding — wraps `frb.setSelectorBindings`. Tests
/// override with a recorder that captures `(toolId, bindings)`.
final selectorBindingsSaverProvider = Provider<SelectorBindingsSaver>(
  (ref) =>
      (toolId, bindings) =>
          frb.setSelectorBindings(toolId: toolId.value, bindings: bindings),
);
