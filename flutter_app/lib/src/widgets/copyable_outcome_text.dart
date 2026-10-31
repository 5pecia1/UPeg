import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

enum ResultInputRelation { current, previous, unknown }

ResultInputRelation resultInputRelation(ToolArgs? executed, ToolArgs? current) {
  if (executed == null || current == null) return ResultInputRelation.unknown;
  return executed == current
      ? ResultInputRelation.current
      : ResultInputRelation.previous;
}

String copySuccessKeyForRelation(ResultInputRelation relation) =>
    switch (relation) {
      ResultInputRelation.current => 'clipboard.copied',
      ResultInputRelation.previous => 'clipboard.copied_previous_result',
      ResultInputRelation.unknown => 'clipboard.copied_input_unknown',
    };

/// Text represented by the result's primary output. File values have a
/// separate Save action; their displayed summary is not the file contents.
String? copyableOutcomeText(CanonicalToolResult? outcome) {
  if (outcome == null || !outcome.ok) return null;
  final value = outcome.primaryOutput?.value;
  if (value == null ||
      value is CanonicalOutputValue_File ||
      value is CanonicalOutputValue_EmbeddedView) {
    return null;
  }
  final text = value.displayText;
  return text.isEmpty ? null : text;
}
