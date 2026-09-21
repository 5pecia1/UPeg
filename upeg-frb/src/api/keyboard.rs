//! Keyboard command resolution exposed to Flutter.
//!
//! Wraps `upeg_core::resolve_key` so the Dart `Shortcuts`/`Actions`
//! tree can ask Rust the question "what command does this stroke
//! produce in this context?". Keeping the key binding policy on the
//! Rust side guarantees TUI / Desktop / Flutter all stay in lock-step
//! on which keys map to which commands.
//!
//! The wrapper translates Dart's [`LogicalKeyboardKey`] vocabulary
//! ("KeyK", "ArrowUp", "F1", "Escape", " "/"k"/"K"…) into the surface-
//! neutral [`upeg_core::Key`] enum, applies the requested keyboard
//! scope (default: Board), and returns a sealed [`KeyboardCommandDto`]
//! that mirrors [`upeg_core::KeyboardCommand`].

use upeg_core::{
    BoardSlot, ChordModifier, Key, KeyModifiers, KeyboardCommand, KeyboardContext, KeyboardScope,
    NavDirection, OrderDirection, PageDirection, binding_catalog, key_stroke_from_label,
    resolve_key,
};

/// Dart-mirrored view of [`upeg_core::KeyboardScope`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum KeyboardScopeDto {
    Board,
    Detail,
    Form,
    Settings,
    BoardEditor,
    ConfirmDelete,
    ToolPicker,
    FilterBar,
    RightPane,
    Moving,
    Resize,
}

impl From<KeyboardScope> for KeyboardScopeDto {
    fn from(scope: KeyboardScope) -> Self {
        match scope {
            KeyboardScope::Board => Self::Board,
            KeyboardScope::Detail => Self::Detail,
            KeyboardScope::Form => Self::Form,
            KeyboardScope::Settings => Self::Settings,
            KeyboardScope::BoardEditor => Self::BoardEditor,
            KeyboardScope::ConfirmDelete => Self::ConfirmDelete,
            KeyboardScope::ToolPicker => Self::ToolPicker,
            KeyboardScope::FilterBar => Self::FilterBar,
            KeyboardScope::RightPane => Self::RightPane,
            KeyboardScope::Moving => Self::Moving,
            KeyboardScope::Resize => Self::Resize,
        }
    }
}

impl From<KeyboardScopeDto> for KeyboardScope {
    fn from(scope: KeyboardScopeDto) -> Self {
        match scope {
            KeyboardScopeDto::Board => Self::Board,
            KeyboardScopeDto::Detail => Self::Detail,
            KeyboardScopeDto::Form => Self::Form,
            KeyboardScopeDto::Settings => Self::Settings,
            KeyboardScopeDto::BoardEditor => Self::BoardEditor,
            KeyboardScopeDto::ConfirmDelete => Self::ConfirmDelete,
            KeyboardScopeDto::ToolPicker => Self::ToolPicker,
            KeyboardScopeDto::FilterBar => Self::FilterBar,
            KeyboardScopeDto::RightPane => Self::RightPane,
            KeyboardScopeDto::Moving => Self::Moving,
            KeyboardScopeDto::Resize => Self::Resize,
        }
    }
}

/// Direction enum mirror for nav/move/page/reorder commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum DirectionDto {
    Up,
    Down,
    Left,
    Right,
}

impl From<NavDirection> for DirectionDto {
    fn from(d: NavDirection) -> Self {
        match d {
            NavDirection::Up => Self::Up,
            NavDirection::Down => Self::Down,
            NavDirection::Left => Self::Left,
            NavDirection::Right => Self::Right,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum OrderDirectionDto {
    Previous,
    Next,
}

impl From<OrderDirection> for OrderDirectionDto {
    fn from(d: OrderDirection) -> Self {
        match d {
            OrderDirection::Previous => Self::Previous,
            OrderDirection::Next => Self::Next,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum PageDirectionDto {
    Up,
    Down,
}

impl From<PageDirection> for PageDirectionDto {
    fn from(d: PageDirection) -> Self {
        match d {
            PageDirection::Up => Self::Up,
            PageDirection::Down => Self::Down,
        }
    }
}

/// Sealed enum mirror of [`upeg_core::KeyboardCommand`]. Each variant
/// carries its payload as plain owned data so flutter_rust_bridge emits
/// a Dart sealed class Dart can pattern-match on with full
/// exhaustiveness.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum KeyboardCommandDto {
    Move {
        direction: DirectionDto,
    },
    FocusNext,
    FocusPrevious,
    Page {
        direction: PageDirectionDto,
    },
    Home,
    End,
    Open,
    Run,
    Search,
    Close,
    Quit,
    OpenSettings,
    CycleBoardFilter,
    CycleTagFilter,
    ClearBoardFilter,
    /// `slot` is in the closed range `1..=9` per [`BoardSlot`].
    SwitchBoard {
        slot: u8,
    },
    NewBoard,
    RenameBoard,
    DeleteBoard,
    OpenToolPicker,
    TogglePin,
    Reorder {
        direction: OrderDirectionDto,
    },
    /// Cmd/Ctrl + `[` — move the focused pin one slot earlier.
    /// Inventory row F12. See [`upeg_core::KeyboardCommand::MovePinPrev`].
    MovePinPrev,
    /// Cmd/Ctrl + `]` — symmetric counterpart.
    MovePinNext,
    StartMove,
    /// `e` with a focused pin — enter resize mode for that pin.
    /// See [`upeg_core::KeyboardCommand::StartResize`].
    StartResize,
    /// Span delta emitted in the Resize scope; the direction→delta
    /// mapping already happened in the shared resolver, so Dart applies
    /// `cols`/`rows` verbatim and clamps to the legal span range.
    ResizeBy {
        cols: i32,
        rows: i32,
    },
    /// `0` in the Resize scope — reset the pin to its manifest
    /// footprint (`clear_pin_span`).
    ResetSpan,
    Commit,
    Confirm,
    Cancel,
    /// `?` in board scope — open the keyboard-cheatsheet overlay that
    /// renders [`keyboard_binding_catalog`].
    ShowCheatsheet,
    Text {
        ch: String,
    },
    Backspace,
    /// `c` with a focused pin — open the pin color dialog.
    /// See [`upeg_core::KeyboardCommand::EditPinColor`].
    EditPinColor,
    /// Ctrl+U — clear the focused text buffer. Never returned by
    /// [`keyboard_command_for`] (Flutter text fields get native editing
    /// shortcuts); exists so the cheatsheet catalog can display the
    /// shared binding.
    ClearInput,
    /// F2 — copy the current output. Never returned by
    /// [`keyboard_command_for`] (GUI copy is modal-local); exists so the
    /// cheatsheet catalog can display the shared binding.
    Copy,
}

/// Convert the shared resolver's command into its Dto mirror.
///
/// Total: every shared command has a Dto so the cheatsheet catalog can
/// display it. Whether a command participates in event *resolution* is a
/// separate concern — [`keyboard_command_for`] filters the
/// display-only commands (see [`resolves_in_gui`]) back to `None` so
/// the event keeps propagating exactly as before.
fn keyboard_command_dto_from(cmd: KeyboardCommand) -> KeyboardCommandDto {
    match cmd {
        KeyboardCommand::Move(d) => KeyboardCommandDto::Move {
            direction: DirectionDto::from(d),
        },
        KeyboardCommand::FocusNext => KeyboardCommandDto::FocusNext,
        KeyboardCommand::FocusPrevious => KeyboardCommandDto::FocusPrevious,
        KeyboardCommand::Page(d) => KeyboardCommandDto::Page {
            direction: PageDirectionDto::from(d),
        },
        KeyboardCommand::Home => KeyboardCommandDto::Home,
        KeyboardCommand::End => KeyboardCommandDto::End,
        KeyboardCommand::Open => KeyboardCommandDto::Open,
        KeyboardCommand::Run => KeyboardCommandDto::Run,
        KeyboardCommand::Search => KeyboardCommandDto::Search,
        KeyboardCommand::Close => KeyboardCommandDto::Close,
        KeyboardCommand::Quit => KeyboardCommandDto::Quit,
        KeyboardCommand::OpenSettings => KeyboardCommandDto::OpenSettings,
        KeyboardCommand::CycleBoardFilter => KeyboardCommandDto::CycleBoardFilter,
        KeyboardCommand::CycleTagFilter => KeyboardCommandDto::CycleTagFilter,
        KeyboardCommand::ClearBoardFilter => KeyboardCommandDto::ClearBoardFilter,
        KeyboardCommand::SwitchBoard(slot) => {
            KeyboardCommandDto::SwitchBoard { slot: slot.as_u8() }
        }
        KeyboardCommand::NewBoard => KeyboardCommandDto::NewBoard,
        KeyboardCommand::RenameBoard => KeyboardCommandDto::RenameBoard,
        KeyboardCommand::DeleteBoard => KeyboardCommandDto::DeleteBoard,
        KeyboardCommand::OpenToolPicker => KeyboardCommandDto::OpenToolPicker,
        KeyboardCommand::TogglePin => KeyboardCommandDto::TogglePin,
        KeyboardCommand::Reorder(d) => KeyboardCommandDto::Reorder {
            direction: OrderDirectionDto::from(d),
        },
        KeyboardCommand::MovePinPrev => KeyboardCommandDto::MovePinPrev,
        KeyboardCommand::MovePinNext => KeyboardCommandDto::MovePinNext,
        KeyboardCommand::StartMove => KeyboardCommandDto::StartMove,
        KeyboardCommand::StartResize => KeyboardCommandDto::StartResize,
        KeyboardCommand::ResizeBy { cols, rows } => KeyboardCommandDto::ResizeBy {
            cols: i32::from(cols),
            rows: i32::from(rows),
        },
        KeyboardCommand::ResetSpan => KeyboardCommandDto::ResetSpan,
        KeyboardCommand::Commit => KeyboardCommandDto::Commit,
        KeyboardCommand::Confirm => KeyboardCommandDto::Confirm,
        KeyboardCommand::Cancel => KeyboardCommandDto::Cancel,
        KeyboardCommand::ShowCheatsheet => KeyboardCommandDto::ShowCheatsheet,
        KeyboardCommand::Text(ch) => KeyboardCommandDto::Text { ch: ch.to_string() },
        KeyboardCommand::Backspace => KeyboardCommandDto::Backspace,
        KeyboardCommand::EditPinColor => KeyboardCommandDto::EditPinColor,
        KeyboardCommand::ClearInput => KeyboardCommandDto::ClearInput,
        KeyboardCommand::Copy => KeyboardCommandDto::Copy,
    }
}

/// Whether a resolved command should be *returned* to the Dart event
/// loop. `ClearInput` (Flutter text fields already get native platform
/// editing shortcuts) and `Copy` (GUI copy is modal-local F2, not
/// routed through this keyboard policy) stay display-only:
/// [`keyboard_command_for`] maps them back to `None`, which Dart treats
/// as "let the event propagate".
const fn resolves_in_gui(cmd: &KeyboardCommandDto) -> bool {
    !matches!(
        cmd,
        KeyboardCommandDto::ClearInput | KeyboardCommandDto::Copy
    )
}

/// Resolve a stroke into a [`KeyboardCommandDto`].
///
/// `key` is a Dart-side label (e.g. `"k"`, `"K"`, `"ArrowUp"`,
/// `"Escape"`, `"F1"`). `ctrl/meta/shift/alt` are the modifier states.
/// `scope` defaults to [`KeyboardScopeDto::Board`] when omitted by the
/// caller; `has_tool_focus` gates the per-pin commands (TogglePin,
/// EditPinColor, StartMove, Reorder) that need a focused tool to act on.
/// Board-management commands (NewBoard, RenameBoard, DeleteBoard,
/// OpenToolPicker) are always live now that the surface is modeless.
///
/// Returns `None` when no command applies — Dart treats `None` as
/// "let the event propagate" so default text input behaviour still
/// works inside text fields.
#[allow(
    clippy::too_many_arguments,
    reason = "FRB sealed-enum support is limited; passing a struct here would force the Dart side into an opaque handle. The argument count matches the keyboard-policy inputs canonically (key, four modifier flags, scope, focus flag)."
)]
#[flutter_rust_bridge::frb(sync)]
pub fn keyboard_command_for(
    key: String,
    ctrl: bool,
    meta: bool,
    shift: bool,
    alt: bool,
    scope: Option<KeyboardScopeDto>,
    has_tool_focus: bool,
) -> Option<KeyboardCommandDto> {
    let modifiers = KeyModifiers {
        control: ctrl,
        meta,
        alt,
        shift,
    };
    let stroke = key_stroke_from_label(&key, modifiers)?;
    let scope = scope
        .map(KeyboardScope::from)
        .unwrap_or(KeyboardScope::Board);
    let context = KeyboardContext {
        scope,
        has_tool_focus,
    };
    resolve_key(context, stroke)
        .map(keyboard_command_dto_from)
        .filter(resolves_in_gui)
}

/// `BoardSlot` validator exposed for the Dart side. Used by integration
/// code that mirrors the slot validation in `KeyboardCommand::SwitchBoard`.
#[flutter_rust_bridge::frb(sync)]
pub fn board_slot_is_valid(slot: u8) -> bool {
    BoardSlot::new(slot).is_some()
}

/// Dart-mirrored view of [`upeg_core::keyboard::Key`] for cheatsheet
/// display. Sealed so Dart can map every key cap to a glyph
/// exhaustively.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum CatalogKeyDto {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
    Tab,
    BackTab,
    PageUp,
    PageDown,
    Home,
    End,
    Backspace,
    Space,
    Char { ch: String },
    F { number: u8 },
}

impl From<Key> for CatalogKeyDto {
    fn from(key: Key) -> Self {
        match key {
            Key::Up => Self::Up,
            Key::Down => Self::Down,
            Key::Left => Self::Left,
            Key::Right => Self::Right,
            Key::Enter => Self::Enter,
            Key::Esc => Self::Esc,
            Key::Tab => Self::Tab,
            Key::BackTab => Self::BackTab,
            Key::PageUp => Self::PageUp,
            Key::PageDown => Self::PageDown,
            Key::Home => Self::Home,
            Key::End => Self::End,
            Key::Backspace => Self::Backspace,
            Key::Space => Self::Space,
            Key::Char(ch) => Self::Char { ch: ch.to_string() },
            Key::F(number) => Self::F { number },
        }
    }
}

/// Mirror of [`upeg_core::ChordModifier`]: `Primary` renders as the
/// platform's command modifier (⌘ / Ctrl), `Control` is literally Ctrl.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum ChordModifierDto {
    None,
    Primary,
    Control,
}

impl From<ChordModifier> for ChordModifierDto {
    fn from(modifier: ChordModifier) -> Self {
        match modifier {
            ChordModifier::None => Self::None,
            ChordModifier::Primary => Self::Primary,
            ChordModifier::Control => Self::Control,
        }
    }
}

/// One displayable chord of a catalog binding.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct CatalogChordDto {
    pub modifier: ChordModifierDto,
    pub key: CatalogKeyDto,
}

/// A chord plus the command it resolves to — carried so Dart surfaces
/// (pin context menu key hints, tests) can look bindings up by command
/// instead of by label string.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct CatalogBindingDto {
    pub chord: CatalogChordDto,
    pub command: KeyboardCommandDto,
}

/// One cheatsheet row: chords + i18n label key (`keys.cmd.*`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct BindingEntryDto {
    pub label_key: String,
    pub requires_tool_focus: bool,
    pub bindings: Vec<CatalogBindingDto>,
}

/// One cheatsheet section: a keyboard scope, its i18n label key
/// (`keys.scope.*`), and its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct ScopeBindingsDto {
    pub scope: KeyboardScopeDto,
    pub label_key: String,
    pub entries: Vec<BindingEntryDto>,
}

/// The shared binding catalog (`upeg_core::binding_catalog`) mirrored
/// for Dart. Powers the `?` cheatsheet overlay and the pin context-menu
/// key hints, so the GUI never hand-writes a shortcut table that could
/// drift from the resolver. Display-only commands (`Copy`,
/// `ClearInput`) are included — the cheatsheet teaches them even though
/// [`keyboard_command_for`] never returns them.
#[flutter_rust_bridge::frb(sync)]
pub fn keyboard_binding_catalog() -> Vec<ScopeBindingsDto> {
    binding_catalog()
        .into_iter()
        .map(|scope_bindings| ScopeBindingsDto {
            scope: KeyboardScopeDto::from(scope_bindings.scope),
            label_key: scope_bindings.label_key.to_string(),
            entries: scope_bindings
                .entries
                .into_iter()
                .map(|entry| BindingEntryDto {
                    label_key: entry.label_key.to_string(),
                    requires_tool_focus: entry.requires_tool_focus,
                    bindings: entry
                        .bindings
                        .into_iter()
                        .map(|binding| CatalogBindingDto {
                            chord: CatalogChordDto {
                                modifier: ChordModifierDto::from(binding.chord.modifier),
                                key: CatalogKeyDto::from(binding.chord.key),
                            },
                            command: keyboard_command_dto_from(binding.command),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_command_for_maps_cmd_k_to_search() {
        let cmd = keyboard_command_for("k".to_string(), false, true, false, false, None, false);
        assert_eq!(cmd, Some(KeyboardCommandDto::Search));
    }

    #[test]
    fn keyboard_command_for_maps_ctrl_k_to_search() {
        let cmd = keyboard_command_for("k".to_string(), true, false, false, false, None, false);
        assert_eq!(cmd, Some(KeyboardCommandDto::Search));
    }

    #[test]
    fn keyboard_command_for_maps_esc_to_close() {
        let cmd = keyboard_command_for(
            "Escape".to_string(),
            false,
            false,
            false,
            false,
            None,
            false,
        );
        assert_eq!(cmd, Some(KeyboardCommandDto::Close));
    }

    #[test]
    fn keyboard_command_for_maps_f1_to_run() {
        let cmd = keyboard_command_for("F1".to_string(), false, false, false, false, None, false);
        assert_eq!(cmd, Some(KeyboardCommandDto::Run));
    }

    #[test]
    fn keyboard_command_for_maps_space_label_to_run() {
        let cmd =
            keyboard_command_for("Space".to_string(), false, false, false, false, None, false);
        assert_eq!(cmd, Some(KeyboardCommandDto::Run));
    }

    #[test]
    fn keyboard_command_for_maps_b_to_cycle_board_filter() {
        let cmd = keyboard_command_for("b".to_string(), false, false, false, false, None, false);
        assert_eq!(cmd, Some(KeyboardCommandDto::CycleBoardFilter));
    }

    #[test]
    fn keyboard_command_for_maps_t_to_cycle_tag_filter() {
        let cmd = keyboard_command_for("t".to_string(), false, false, false, false, None, false);
        assert_eq!(cmd, Some(KeyboardCommandDto::CycleTagFilter));
    }

    #[test]
    fn keyboard_command_for_maps_digit_3_to_switch_board_3() {
        let cmd = keyboard_command_for("3".to_string(), false, false, false, false, None, false);
        assert_eq!(cmd, Some(KeyboardCommandDto::SwitchBoard { slot: 3 }));
    }

    #[test]
    fn keyboard_command_for_maps_n_to_new_board() {
        // Modeless: board-management keys are always active, even
        // without focus.
        let cmd = keyboard_command_for("n".to_string(), false, false, false, false, None, false);
        assert_eq!(cmd, Some(KeyboardCommandDto::NewBoard));
    }

    #[test]
    fn keyboard_command_for_rejects_p_without_focus() {
        // The only gate left after Modeless: the pin-target key (p)
        // cannot target any pin without focus, so it falls to None.
        let without_focus =
            keyboard_command_for("p".to_string(), false, false, false, false, None, false);
        assert_eq!(without_focus, None);

        let with_focus =
            keyboard_command_for("p".to_string(), false, false, false, false, None, true);
        assert_eq!(with_focus, Some(KeyboardCommandDto::TogglePin));
    }

    #[test]
    fn keyboard_command_for_maps_arrow_up_to_move_up() {
        let cmd = keyboard_command_for(
            "ArrowUp".to_string(),
            false,
            false,
            false,
            false,
            None,
            false,
        );
        assert_eq!(
            cmd,
            Some(KeyboardCommandDto::Move {
                direction: DirectionDto::Up,
            })
        );
    }

    #[test]
    fn board_slot_is_valid_allows_1_through_9() {
        assert!(!board_slot_is_valid(0));
        assert!(board_slot_is_valid(1));
        assert!(board_slot_is_valid(9));
        assert!(!board_slot_is_valid(10));
    }

    #[test]
    fn keyboard_command_for_maps_cmd_left_bracket_to_move_pin_prev() {
        let cmd = keyboard_command_for(
            "[".to_string(),
            false,
            true, // meta = Cmd
            false,
            false,
            None,
            false,
        );
        assert_eq!(cmd, Some(KeyboardCommandDto::MovePinPrev));
    }

    #[test]
    fn keyboard_command_for_maps_cmd_right_bracket_to_move_pin_next() {
        let cmd = keyboard_command_for(
            "]".to_string(),
            false,
            true, // meta = Cmd
            false,
            false,
            None,
            false,
        );
        assert_eq!(cmd, Some(KeyboardCommandDto::MovePinNext));
    }

    #[test]
    fn keyboard_command_for_maps_question_mark_to_show_cheatsheet() {
        // '?' entered via Shift+/ — Dart sends character '?' with
        // shift=true.
        let cmd = keyboard_command_for("?".to_string(), false, false, true, false, None, false);
        assert_eq!(cmd, Some(KeyboardCommandDto::ShowCheatsheet));
    }

    #[test]
    fn keyboard_command_for_does_not_return_display_only_commands() {
        // F2 (Copy) is handled modal-locally, so the event must keep
        // propagating.
        let cmd = keyboard_command_for("F2".to_string(), false, false, false, false, None, false);
        assert_eq!(cmd, None);
        // Ctrl+U (ClearInput) also yields to the text field's native
        // editing.
        let cmd = keyboard_command_for(
            "u".to_string(),
            true,
            false,
            false,
            false,
            Some(KeyboardScopeDto::Form),
            false,
        );
        assert_eq!(cmd, None);
    }

    #[test]
    fn binding_catalog_mirrors_all_scopes() {
        let catalog = keyboard_binding_catalog();
        let core = upeg_core::binding_catalog();
        assert_eq!(catalog.len(), core.len());
        for (dto, core_scope) in catalog.iter().zip(&core) {
            assert_eq!(dto.scope, KeyboardScopeDto::from(core_scope.scope));
            assert_eq!(dto.label_key, core_scope.label_key);
            assert_eq!(dto.entries.len(), core_scope.entries.len());
        }
    }

    #[test]
    fn binding_catalog_board_scope_includes_cheatsheet_binding() {
        let catalog = keyboard_binding_catalog();
        let board = catalog
            .iter()
            .find(|s| s.scope == KeyboardScopeDto::Board)
            .expect("board scope must exist");
        let cheatsheet = board
            .entries
            .iter()
            .find(|e| {
                e.bindings
                    .iter()
                    .any(|b| b.command == KeyboardCommandDto::ShowCheatsheet)
            })
            .expect("cheatsheet entry must exist");
        assert_eq!(cheatsheet.label_key, "keys.cmd.cheatsheet");
        assert_eq!(
            cheatsheet.bindings[0].chord.key,
            CatalogKeyDto::Char { ch: "?".into() },
        );
    }

    #[test]
    fn binding_catalog_also_exposes_display_only_copy_binding() {
        // Filtered out on the resolution path (F2 → None) but must
        // still appear on the cheatsheet.
        let catalog = keyboard_binding_catalog();
        let board = catalog
            .iter()
            .find(|s| s.scope == KeyboardScopeDto::Board)
            .expect("board scope must exist");
        assert!(board.entries.iter().any(|e| {
            e.bindings
                .iter()
                .any(|b| b.command == KeyboardCommandDto::Copy)
        }),);
    }

    #[test]
    fn keyboard_command_for_maps_ctrl_left_bracket_to_move_pin_prev() {
        let cmd = keyboard_command_for(
            "[".to_string(),
            true, // ctrl (non-mac primary)
            false,
            false,
            false,
            None,
            false,
        );
        assert_eq!(cmd, Some(KeyboardCommandDto::MovePinPrev));
    }
}
