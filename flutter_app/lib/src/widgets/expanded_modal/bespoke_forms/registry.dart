/// Registry of bespoke (per-tool) form widgets.
///
/// **Qualification rule:** a tool gets a bespoke form IFF it needs a
/// *live preview* — output that updates as the user types, with no run
/// step. Everything else uses [GenericFormWidget], which renders any
/// tool's declared inputs (kinds, descriptions, placeholders, defaults,
/// ranges, choice labels) and pairs with the host's Run button, result
/// panel and copy affordance.
///
/// One entry qualifies today: `num.hex_to_decimal` (decode-as-you-type).
/// The former one-click-regeneration entries (`id.uuid_v7`,
/// `id.uuid_v4`, `id.nanoid`, `security.password_generate`) were deleted
/// once the generic path grew declared defaults and integer constraints
/// — "press a button, read the result, copy it" is what the generic form
/// plus its host already do.
///
/// `ExpandedModalPage` picks bespoke first; if no registry entry exists
/// for the tool id, it falls back to [GenericFormWidget].
library;

import 'package:flutter/widgets.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/widgets/expanded_modal/bespoke_forms/hex_to_dec_form.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

/// Builder signature for bespoke per-tool forms. The widget owns its
/// own form state; on submit it hands the rendered args back via
/// `onSubmit` so the host can call `dispatchTool` without knowing the
/// tool-specific argument shape. `lastStdout` carries the host's latest
/// dispatch output for forms that keep a local "copy last result" UX.
/// `initialInput` is the deep-link / pin pre-fill (mirrors the generic
/// form's `controller.seed`); forms that pre-populate a field consume it.
typedef BespokeFormBuilder =
    Widget Function(
      BuildContext context,
      ToolDto tool,
      void Function(ToolArgs args) onSubmit,
      String? lastStdout,
      ToolArgs? initialInput,
    );

/// Tool id key for the hex → decimal bespoke form. Pulled out as a
/// constant so the registry, tests, and any deep-link routing all
/// agree on the spelling.
const String hexToDecToolId = 'num.hex_to_decimal';

/// Tool id → bespoke form builder.
final Map<ToolId, BespokeFormBuilder> bespokeRegistry =
    <ToolId, BespokeFormBuilder>{
      ToolId.parse(
        hexToDecToolId,
      ): (context, tool, onSubmit, lastStdout, initialInput) => HexToDecForm(
        tool: tool,
        onSubmit: onSubmit,
        initialInput: initialInput,
      ),
    };

/// Resolve a bespoke builder for `toolId`, or `null` when the tool uses
/// the generic form.
BespokeFormBuilder? bespokeFor(ToolId toolId) => bespokeRegistry[toolId];
