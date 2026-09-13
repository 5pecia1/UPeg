/// Metadata-derived roles for the memos feature.
///
/// Like [tool_roles], these branch on typed `ToolDto` metadata (pin kind,
/// invoker, source, output field kinds) rather than a literal tool id or
/// toolkit string, so the feature stays generic-over-example: any tool
/// that declares the same shape gets the same behaviour.
library;

import 'package:upeg/src/rust/api/tools.dart';

/// Whether [tool] is an inline, editable, persistent notepad pin.
///
/// Signal: a `Live` pin driven by the local `function` invoker with a
/// `userInput` source (the user edits it in place — no polling, no
/// network) whose primary output is `markdown` (free-form persisted
/// text). `memo.scratch` is today's match; a live ticker (`time.epoch`,
/// Timer source) or a live-http pin (`eth.gas`) is excluded because its
/// source/invoker differ.
bool isMemoNotepadTool(ToolDto tool) {
  return tool.pinKind == PinKindDto.live &&
      tool.invoker == InvokerDto.function &&
      tool.source is SourceDto_UserInput &&
      tool.outputFields.any(
        (field) => field.fieldType is OutputFieldType_Markdown,
      );
}

/// Whether [tool] is a keyboard-triggered "create a new memo" action.
///
/// Signal: an `Action` pin whose source is a keyboard `shortcut` (it is
/// invoked by its declared chord, e.g. Cmd+Shift+N, and produces no
/// output of its own). `memo.create` is today's match.
bool isMemoCreateAction(ToolDto tool) {
  return tool.pinKind == PinKindDto.action && tool.source is SourceDto_Shortcut;
}
