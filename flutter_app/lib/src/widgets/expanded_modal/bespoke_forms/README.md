# Bespoke per-tool forms

The `ExpandedModalPage` picks a form for the active tool in two steps:

1. **Bespoke lookup.** If `bespokeRegistry[tool.id]` is set, that builder
   wins — the tool fully owns its argument-collection UX. The builder is
   responsible for collecting args and calling `onSubmit(args)`. The host
   also passes the latest dispatch `stdout`; forms that do not need a
   last-result preview should ignore it.
2. **Generic fallback.** Otherwise, `GenericFormWidget(tool: ...)`
   renders one field per `ToolDto.inputFields` entry by pattern-matching
   on the `InputFieldType` sealed enum.

## The qualification rule

**A tool earns a bespoke form IFF it needs a live preview** — output that
updates as the user types, with no run step. That is the one interaction
model `GenericFormWidget` does not have.

Nothing else qualifies. In particular these are *not* reasons:

- a keyboard shortcut — `ExpandedModalPage` already binds `[F1]` run and
  `[F2]` copy for every tool, generic or bespoke;
- a copy button — every generic result block carries a
  `CopyToClipboardButton`;
- a default value, a numeric range, a placeholder, a choice label or a
  helper description — all of those are declared in Rust
  (`Integer(min=…, max=…, default=…)`, `String(regex=…, placeholder=…,
  default=…)`, `Options([…])`) and travel to Dart on `InputFieldDto`, so
  the generic form renders them. If one is missing from the UI, fix the
  declaration or the generic form — do not fork a widget;
- taking no inputs — a zero-input tool gets a Run button from its host
  (the modal's primary button, the inline pin body's Run button).

`registry.dart` therefore registers exactly one entry:
`num.hex_to_decimal` (live decode-as-you-type).

## Adding a bespoke form

1. Drop the new widget into this directory:
   `flutter_app/lib/src/widgets/expanded_modal/bespoke_forms/<tool_id>.dart`.
   Use a snake-cased filename; e.g. `hex_to_dec_form.dart`.
2. Export a builder of type `BespokeFormBuilder` from that file.
3. Register it in `registry.dart`, keyed by the parsed `ToolId`:

   ```dart
   final Map<ToolId, BespokeFormBuilder> bespokeRegistry = <ToolId, BespokeFormBuilder>{
     ToolId.parse(hexToDecToolId):
         (context, tool, onSubmit, lastStdout, initialInput) =>
             HexToDecForm(tool: tool, onSubmit: onSubmit, initialInput: initialInput),
   };
   ```

Bespoke forms own their state — pick `StatefulWidget` + a private state
class, or a `ChangeNotifier`/Riverpod provider when the form needs
cross-widget coordination. Keep one builder per file (SoC); shared
helpers can live in a sibling `_shared/` directory if needed.
