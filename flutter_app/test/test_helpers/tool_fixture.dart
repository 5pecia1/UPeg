/// Shared fixture builders for tests that need a `ToolDto` instance
/// without re-typing the now-required `pinKind` / `invoker` /
/// `pegboardUnits` triple at every call site.
///
/// Lives under `test/test_helpers/` so it is excluded from production
/// imports and only paid for at `flutter test` time. Each helper picks
/// the most-common default (`PinKindDto.inline`, `InvokerDto.function_`
/// → `function`, `PegboardUnitsDto.u1`) — tests that exercise a
/// specific variant override one or more fields via named arguments.
///
/// `requiresApproval` / `approvalSurfaces` default to "no approval
/// barrier" — the shape every non-Chain tool has. The UI that reads them
/// has shipped: `widgets/approval_confirm_dialog.dart` puts a
/// confirmation in front of a gated run on the desktop surface. So a
/// fixture that leaves these at their defaults is asserting "this tool
/// runs without a barrier", not "the barrier is unimplemented" — flip
/// `requiresApproval` when the test is about the dialog.
library;

import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';

ToolDto fixtureToolDto({
  required String id,
  String toolkit = 'fixture',
  String label = 'fixture tool',
  String description = '',
  List<String> tags = const <String>[],
  List<InputFieldDto> inputFields = const <InputFieldDto>[],
  List<OutputFieldDto> outputFields = const <OutputFieldDto>[],
  PinKindDto pinKind = PinKindDto.inline,
  InvokerDto invoker = InvokerDto.function,
  PegboardUnitsDto pegboardUnits = PegboardUnitsDto.u1,
  SourceDto source = const SourceDto.userInput(),
  bool requiresApproval = false,
  List<String> approvalSurfaces = const <String>[],
}) {
  return ToolDto(
    id: id,
    toolkit: toolkit,
    label: label,
    description: description,
    tags: tags,
    inputFields: inputFields,
    outputFields: outputFields,
    pinKind: pinKind,
    invoker: invoker,
    pegboardUnits: pegboardUnits,
    source: source,
    requiresApproval: requiresApproval,
    approvalSurfaces: approvalSurfaces,
  );
}
