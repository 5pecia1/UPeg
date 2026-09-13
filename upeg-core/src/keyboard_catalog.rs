//! Data-driven catalog of the key bindings [`resolve_key`] implements.
//!
//! The cheatsheet overlay (and any other "teach the keys" surface)
//! renders this catalog instead of hand-writing a shortcut table, so the
//! rendered help can never drift from the resolver: the tests in this
//! module fail whenever the catalog and [`resolve_key`] disagree in
//! either direction.
//!
//! Labels are i18n **keys**, never display strings — the catalog carries
//! structure only; the En/Ko text lives in each surface's catalog
//! (`upeg-pegboard-ui/src/i18n.rs` for the Flutter surface).
//!
//! [`resolve_key`]: crate::keyboard::resolve_key

use crate::keyboard::{
    BoardSlot, Key, KeyModifiers, KeyStroke, KeyboardCommand, KeyboardScope, NavDirection,
    OrderDirection, PageDirection,
};

/// Modifier requirement of a catalog chord, in display terms.
///
/// `Primary` is the surface-conventional command modifier (Cmd on macOS,
/// Ctrl elsewhere) — [`CatalogChord::strokes`] expands it to both
/// concrete modifier sets because [`resolve_key`] accepts either.
/// `Control` is literally the Ctrl key on every platform (the `Ctrl+U`
/// clear-input idiom).
///
/// [`resolve_key`]: crate::keyboard::resolve_key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordModifier {
    None,
    Primary,
    Control,
}

/// One displayable key chord: a modifier requirement plus the key cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogChord {
    pub modifier: ChordModifier,
    pub key: Key,
}

impl CatalogChord {
    /// Expand the display chord into the concrete strokes it stands for.
    /// Used by the consistency tests to replay every chord through
    /// [`resolve_key`].
    ///
    /// [`resolve_key`]: crate::keyboard::resolve_key
    pub fn strokes(self) -> Vec<KeyStroke> {
        let with = |control: bool, meta: bool| {
            KeyStroke::modified(
                self.key,
                KeyModifiers {
                    control,
                    meta,
                    ..KeyModifiers::NONE
                },
            )
        };
        match self.modifier {
            ChordModifier::None => vec![KeyStroke::plain(self.key)],
            ChordModifier::Primary => vec![with(true, false), with(false, true)],
            ChordModifier::Control => vec![with(true, false)],
        }
    }
}

/// A chord together with the exact command [`resolve_key`] must produce
/// for it — the unit the consistency tests verify.
///
/// [`resolve_key`]: crate::keyboard::resolve_key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogBinding {
    pub chord: CatalogChord,
    pub command: KeyboardCommand,
}

/// One cheatsheet row: the chords that trigger one user-facing action,
/// labelled by an i18n key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingEntry {
    /// i18n key for the row label (`keys.cmd.*`). Translation lives in
    /// the surface catalogs, keyed En/Ko.
    pub label_key: &'static str,
    /// Board-scope per-pin gate: the binding only resolves while a pin
    /// is focused (`KeyboardContext::has_tool_focus`).
    pub requires_tool_focus: bool,
    pub bindings: Vec<CatalogBinding>,
}

/// All rows of one keyboard scope, labelled by an i18n key
/// (`keys.scope.*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeBindings {
    pub scope: KeyboardScope,
    pub label_key: &'static str,
    pub entries: Vec<BindingEntry>,
}

/// Every keyboard scope in canonical cheatsheet order. Kept next to
/// [`scope_label_key`], whose exhaustive `match` forces this list to be
/// revisited whenever a scope is added.
pub const ALL_KEYBOARD_SCOPES: [KeyboardScope; 11] = [
    KeyboardScope::Board,
    KeyboardScope::Detail,
    KeyboardScope::Form,
    KeyboardScope::Settings,
    KeyboardScope::BoardEditor,
    KeyboardScope::ConfirmDelete,
    KeyboardScope::ToolPicker,
    KeyboardScope::FilterBar,
    KeyboardScope::RightPane,
    KeyboardScope::Moving,
    KeyboardScope::Resize,
];

/// i18n key of a scope's cheatsheet section header. Exhaustive on
/// purpose: adding a [`KeyboardScope`] variant fails compilation here,
/// which drags [`ALL_KEYBOARD_SCOPES`] and [`binding_catalog`] along.
pub const fn scope_label_key(scope: KeyboardScope) -> &'static str {
    match scope {
        KeyboardScope::Board => "keys.scope.board",
        KeyboardScope::Detail => "keys.scope.detail",
        KeyboardScope::Form => "keys.scope.form",
        KeyboardScope::Settings => "keys.scope.settings",
        KeyboardScope::BoardEditor => "keys.scope.board_editor",
        KeyboardScope::ConfirmDelete => "keys.scope.confirm",
        KeyboardScope::ToolPicker => "keys.scope.tool_picker",
        KeyboardScope::FilterBar => "keys.scope.filter_bar",
        KeyboardScope::RightPane => "keys.scope.right_pane",
        KeyboardScope::Moving => "keys.scope.moving",
        KeyboardScope::Resize => "keys.scope.resize",
    }
}

const fn plain(key: Key) -> CatalogChord {
    CatalogChord {
        modifier: ChordModifier::None,
        key,
    }
}

const fn primary(key: Key) -> CatalogChord {
    CatalogChord {
        modifier: ChordModifier::Primary,
        key,
    }
}

const fn control(key: Key) -> CatalogChord {
    CatalogChord {
        modifier: ChordModifier::Control,
        key,
    }
}

const fn bind(chord: CatalogChord, command: KeyboardCommand) -> CatalogBinding {
    CatalogBinding { chord, command }
}

fn entry(label_key: &'static str, bindings: Vec<CatalogBinding>) -> BindingEntry {
    BindingEntry {
        label_key,
        requires_tool_focus: false,
        bindings,
    }
}

fn focus_entry(label_key: &'static str, bindings: Vec<CatalogBinding>) -> BindingEntry {
    BindingEntry {
        label_key,
        requires_tool_focus: true,
        bindings,
    }
}

/// The four arrow/vim directions in the contract's `← ↓ ↑ →` order,
/// mapped through `command` (shared by board focus move, coordinate
/// move, and every other directional row).
fn directional(
    label_key: &'static str,
    command: fn(NavDirection) -> KeyboardCommand,
) -> BindingEntry {
    entry(
        label_key,
        vec![
            bind(plain(Key::Left), command(NavDirection::Left)),
            bind(plain(Key::Down), command(NavDirection::Down)),
            bind(plain(Key::Up), command(NavDirection::Up)),
            bind(plain(Key::Right), command(NavDirection::Right)),
            bind(plain(Key::Char('h')), command(NavDirection::Left)),
            bind(plain(Key::Char('j')), command(NavDirection::Down)),
            bind(plain(Key::Char('k')), command(NavDirection::Up)),
            bind(plain(Key::Char('l')), command(NavDirection::Right)),
        ],
    )
}

fn switch_board_bindings() -> Vec<CatalogBinding> {
    (BoardSlot::MIN..=BoardSlot::MAX)
        .filter_map(|slot| {
            let digit = char::from(b'0' + slot);
            let slot = BoardSlot::new(slot)?;
            Some(bind(
                plain(Key::Char(digit)),
                KeyboardCommand::SwitchBoard(slot),
            ))
        })
        .collect()
}

fn board_scope() -> Vec<BindingEntry> {
    vec![
        directional("keys.cmd.focus_move", KeyboardCommand::Move),
        entry(
            "keys.cmd.focus_cycle",
            vec![
                bind(plain(Key::Tab), KeyboardCommand::FocusNext),
                bind(plain(Key::BackTab), KeyboardCommand::FocusPrevious),
            ],
        ),
        entry(
            "keys.cmd.run",
            vec![
                bind(plain(Key::Enter), KeyboardCommand::Run),
                bind(plain(Key::Space), KeyboardCommand::Run),
                bind(plain(Key::F(1)), KeyboardCommand::Run),
            ],
        ),
        entry(
            "keys.cmd.open",
            vec![bind(plain(Key::Char('o')), KeyboardCommand::Open)],
        ),
        entry(
            "keys.cmd.search",
            vec![
                bind(plain(Key::Char('/')), KeyboardCommand::Search),
                bind(primary(Key::Char('k')), KeyboardCommand::Search),
            ],
        ),
        entry(
            "keys.cmd.copy",
            vec![bind(plain(Key::F(2)), KeyboardCommand::Copy)],
        ),
        entry(
            "keys.cmd.cycle_board_filter",
            vec![bind(
                plain(Key::Char('b')),
                KeyboardCommand::CycleBoardFilter,
            )],
        ),
        entry(
            "keys.cmd.cycle_tag_filter",
            vec![bind(plain(Key::Char('t')), KeyboardCommand::CycleTagFilter)],
        ),
        entry(
            "keys.cmd.clear_board_filter",
            vec![bind(
                plain(Key::Char('0')),
                KeyboardCommand::ClearBoardFilter,
            )],
        ),
        entry("keys.cmd.switch_board", switch_board_bindings()),
        entry(
            "keys.cmd.new_board",
            vec![bind(plain(Key::Char('n')), KeyboardCommand::NewBoard)],
        ),
        entry(
            "keys.cmd.rename_board",
            vec![bind(plain(Key::Char('R')), KeyboardCommand::RenameBoard)],
        ),
        entry(
            "keys.cmd.delete_board",
            vec![bind(plain(Key::Char('D')), KeyboardCommand::DeleteBoard)],
        ),
        entry(
            "keys.cmd.open_tool_picker",
            vec![bind(plain(Key::Char('a')), KeyboardCommand::OpenToolPicker)],
        ),
        focus_entry(
            "keys.cmd.toggle_pin",
            vec![bind(plain(Key::Char('p')), KeyboardCommand::TogglePin)],
        ),
        focus_entry(
            "keys.cmd.edit_pin_color",
            vec![bind(plain(Key::Char('c')), KeyboardCommand::EditPinColor)],
        ),
        focus_entry(
            "keys.cmd.start_move",
            vec![bind(plain(Key::Char('m')), KeyboardCommand::StartMove)],
        ),
        focus_entry(
            "keys.cmd.start_resize",
            vec![bind(plain(Key::Char('e')), KeyboardCommand::StartResize)],
        ),
        focus_entry(
            "keys.cmd.reorder",
            vec![
                bind(
                    plain(Key::Char('[')),
                    KeyboardCommand::Reorder(OrderDirection::Previous),
                ),
                bind(
                    plain(Key::Char(']')),
                    KeyboardCommand::Reorder(OrderDirection::Next),
                ),
            ],
        ),
        entry(
            "keys.cmd.move_pin_slot",
            vec![
                bind(primary(Key::Char('[')), KeyboardCommand::MovePinPrev),
                bind(primary(Key::Char(']')), KeyboardCommand::MovePinNext),
            ],
        ),
        entry(
            "keys.cmd.settings",
            vec![bind(plain(Key::Char('s')), KeyboardCommand::OpenSettings)],
        ),
        entry(
            "keys.cmd.cheatsheet",
            vec![bind(plain(Key::Char('?')), KeyboardCommand::ShowCheatsheet)],
        ),
        entry(
            "keys.cmd.close",
            vec![bind(plain(Key::Esc), KeyboardCommand::Close)],
        ),
        entry(
            "keys.cmd.quit",
            vec![
                bind(plain(Key::Char('q')), KeyboardCommand::Quit),
                bind(primary(Key::Char('q')), KeyboardCommand::Quit),
            ],
        ),
    ]
}

fn detail_scope() -> Vec<BindingEntry> {
    vec![
        entry(
            "keys.cmd.run",
            vec![
                bind(plain(Key::Enter), KeyboardCommand::Run),
                bind(plain(Key::F(1)), KeyboardCommand::Run),
                bind(plain(Key::Char('r')), KeyboardCommand::Run),
            ],
        ),
        entry(
            "keys.cmd.copy",
            vec![bind(plain(Key::F(2)), KeyboardCommand::Copy)],
        ),
        entry(
            "keys.cmd.search",
            vec![
                bind(plain(Key::Char('/')), KeyboardCommand::Search),
                bind(primary(Key::Char('k')), KeyboardCommand::Search),
            ],
        ),
        entry(
            "keys.cmd.start_move",
            vec![bind(plain(Key::Char('m')), KeyboardCommand::StartMove)],
        ),
        entry(
            "keys.cmd.cycle_board_filter",
            vec![bind(
                plain(Key::Char('b')),
                KeyboardCommand::CycleBoardFilter,
            )],
        ),
        entry(
            "keys.cmd.cycle_tag_filter",
            vec![bind(plain(Key::Char('t')), KeyboardCommand::CycleTagFilter)],
        ),
        entry(
            "keys.cmd.clear_board_filter",
            vec![bind(
                plain(Key::Char('0')),
                KeyboardCommand::ClearBoardFilter,
            )],
        ),
        entry(
            "keys.cmd.focus_cycle",
            vec![
                bind(plain(Key::Tab), KeyboardCommand::FocusNext),
                bind(plain(Key::BackTab), KeyboardCommand::FocusPrevious),
            ],
        ),
        entry(
            "keys.cmd.settings",
            vec![bind(plain(Key::Char('s')), KeyboardCommand::OpenSettings)],
        ),
        entry(
            "keys.cmd.close",
            vec![
                bind(plain(Key::Esc), KeyboardCommand::Close),
                bind(plain(Key::Char('q')), KeyboardCommand::Close),
            ],
        ),
    ]
}

fn form_scope() -> Vec<BindingEntry> {
    vec![
        entry(
            "keys.cmd.field_move",
            vec![
                bind(plain(Key::Tab), KeyboardCommand::FocusNext),
                bind(plain(Key::Down), KeyboardCommand::FocusNext),
                bind(plain(Key::BackTab), KeyboardCommand::FocusPrevious),
                bind(plain(Key::Up), KeyboardCommand::FocusPrevious),
            ],
        ),
        entry(
            "keys.cmd.caret_move",
            vec![
                bind(plain(Key::Left), KeyboardCommand::Move(NavDirection::Left)),
                bind(
                    plain(Key::Right),
                    KeyboardCommand::Move(NavDirection::Right),
                ),
            ],
        ),
        entry(
            "keys.cmd.run",
            vec![
                bind(plain(Key::Enter), KeyboardCommand::Run),
                bind(plain(Key::F(1)), KeyboardCommand::Run),
            ],
        ),
        entry(
            "keys.cmd.backspace",
            vec![bind(plain(Key::Backspace), KeyboardCommand::Backspace)],
        ),
        entry(
            "keys.cmd.clear_input",
            vec![bind(control(Key::Char('u')), KeyboardCommand::ClearInput)],
        ),
        entry(
            "keys.cmd.close",
            vec![bind(plain(Key::Esc), KeyboardCommand::Close)],
        ),
    ]
}

fn settings_scope() -> Vec<BindingEntry> {
    vec![
        entry(
            "keys.cmd.focus_cycle",
            vec![
                bind(plain(Key::Down), KeyboardCommand::FocusNext),
                bind(plain(Key::Tab), KeyboardCommand::FocusNext),
                bind(plain(Key::Char('j')), KeyboardCommand::FocusNext),
                bind(plain(Key::Up), KeyboardCommand::FocusPrevious),
                bind(plain(Key::BackTab), KeyboardCommand::FocusPrevious),
                bind(plain(Key::Char('k')), KeyboardCommand::FocusPrevious),
            ],
        ),
        entry(
            "keys.cmd.adjust_setting",
            vec![
                bind(plain(Key::Left), KeyboardCommand::Move(NavDirection::Left)),
                bind(
                    plain(Key::Char('h')),
                    KeyboardCommand::Move(NavDirection::Left),
                ),
                bind(
                    plain(Key::Right),
                    KeyboardCommand::Move(NavDirection::Right),
                ),
                bind(
                    plain(Key::Char('l')),
                    KeyboardCommand::Move(NavDirection::Right),
                ),
            ],
        ),
        entry(
            "keys.cmd.close",
            vec![
                bind(plain(Key::Esc), KeyboardCommand::Close),
                bind(plain(Key::Char('q')), KeyboardCommand::Close),
            ],
        ),
    ]
}

fn board_editor_scope() -> Vec<BindingEntry> {
    vec![
        entry(
            "keys.cmd.commit",
            vec![
                bind(plain(Key::Enter), KeyboardCommand::Commit),
                bind(plain(Key::F(1)), KeyboardCommand::Commit),
            ],
        ),
        entry(
            "keys.cmd.backspace",
            vec![bind(plain(Key::Backspace), KeyboardCommand::Backspace)],
        ),
        entry(
            "keys.cmd.clear_input",
            vec![bind(control(Key::Char('u')), KeyboardCommand::ClearInput)],
        ),
        entry(
            "keys.cmd.cancel",
            vec![bind(plain(Key::Esc), KeyboardCommand::Cancel)],
        ),
    ]
}

fn confirm_scope() -> Vec<BindingEntry> {
    vec![
        entry(
            "keys.cmd.confirm",
            vec![
                bind(plain(Key::Char('y')), KeyboardCommand::Confirm),
                bind(plain(Key::Enter), KeyboardCommand::Confirm),
                bind(plain(Key::F(1)), KeyboardCommand::Confirm),
            ],
        ),
        entry(
            "keys.cmd.cancel",
            vec![
                bind(plain(Key::Char('n')), KeyboardCommand::Cancel),
                bind(plain(Key::Char('q')), KeyboardCommand::Cancel),
                bind(plain(Key::Esc), KeyboardCommand::Cancel),
            ],
        ),
    ]
}

fn tool_picker_scope() -> Vec<BindingEntry> {
    vec![
        entry(
            "keys.cmd.select_move",
            vec![
                bind(plain(Key::Up), KeyboardCommand::Move(NavDirection::Up)),
                bind(plain(Key::Down), KeyboardCommand::Move(NavDirection::Down)),
            ],
        ),
        entry(
            "keys.cmd.commit",
            vec![
                bind(plain(Key::Enter), KeyboardCommand::Commit),
                bind(plain(Key::F(1)), KeyboardCommand::Commit),
            ],
        ),
        entry(
            "keys.cmd.backspace",
            vec![bind(plain(Key::Backspace), KeyboardCommand::Backspace)],
        ),
        entry(
            "keys.cmd.clear_input",
            vec![bind(control(Key::Char('u')), KeyboardCommand::ClearInput)],
        ),
        entry(
            "keys.cmd.cancel",
            vec![bind(plain(Key::Esc), KeyboardCommand::Cancel)],
        ),
    ]
}

fn filter_bar_scope() -> Vec<BindingEntry> {
    vec![
        entry(
            "keys.cmd.select_move",
            vec![
                bind(plain(Key::Left), KeyboardCommand::Move(NavDirection::Left)),
                bind(
                    plain(Key::Char('h')),
                    KeyboardCommand::Move(NavDirection::Left),
                ),
                bind(
                    plain(Key::Right),
                    KeyboardCommand::Move(NavDirection::Right),
                ),
                bind(
                    plain(Key::Char('l')),
                    KeyboardCommand::Move(NavDirection::Right),
                ),
            ],
        ),
        entry(
            "keys.cmd.jump_edge",
            vec![
                bind(plain(Key::Home), KeyboardCommand::Home),
                bind(plain(Key::End), KeyboardCommand::End),
            ],
        ),
        entry(
            "keys.cmd.open",
            vec![
                bind(plain(Key::Enter), KeyboardCommand::Open),
                bind(plain(Key::Space), KeyboardCommand::Open),
            ],
        ),
    ]
}

fn right_pane_scope() -> Vec<BindingEntry> {
    vec![
        entry(
            "keys.cmd.scroll",
            vec![
                bind(plain(Key::Down), KeyboardCommand::Move(NavDirection::Down)),
                bind(
                    plain(Key::Char('j')),
                    KeyboardCommand::Move(NavDirection::Down),
                ),
                bind(plain(Key::Up), KeyboardCommand::Move(NavDirection::Up)),
                bind(
                    plain(Key::Char('k')),
                    KeyboardCommand::Move(NavDirection::Up),
                ),
            ],
        ),
        entry(
            "keys.cmd.page",
            vec![
                bind(plain(Key::PageUp), KeyboardCommand::Page(PageDirection::Up)),
                bind(
                    plain(Key::PageDown),
                    KeyboardCommand::Page(PageDirection::Down),
                ),
            ],
        ),
        entry(
            "keys.cmd.jump_edge",
            vec![
                bind(plain(Key::Home), KeyboardCommand::Home),
                bind(plain(Key::End), KeyboardCommand::End),
            ],
        ),
    ]
}

fn moving_scope() -> Vec<BindingEntry> {
    vec![
        directional("keys.cmd.moving_step", KeyboardCommand::Move),
        entry(
            "keys.cmd.commit",
            vec![
                bind(plain(Key::Enter), KeyboardCommand::Commit),
                bind(plain(Key::F(1)), KeyboardCommand::Commit),
            ],
        ),
        entry(
            "keys.cmd.cancel",
            vec![
                bind(plain(Key::Esc), KeyboardCommand::Cancel),
                bind(plain(Key::Char('q')), KeyboardCommand::Cancel),
            ],
        ),
    ]
}

fn resize_scope() -> Vec<BindingEntry> {
    let by = |cols: i8, rows: i8| KeyboardCommand::ResizeBy { cols, rows };
    vec![
        entry(
            "keys.cmd.resize_wider",
            vec![
                bind(plain(Key::Right), by(1, 0)),
                bind(plain(Key::Char('l')), by(1, 0)),
            ],
        ),
        entry(
            "keys.cmd.resize_narrower",
            vec![
                bind(plain(Key::Left), by(-1, 0)),
                bind(plain(Key::Char('h')), by(-1, 0)),
            ],
        ),
        entry(
            "keys.cmd.resize_taller",
            vec![
                bind(plain(Key::Down), by(0, 1)),
                bind(plain(Key::Char('j')), by(0, 1)),
            ],
        ),
        entry(
            "keys.cmd.resize_shorter",
            vec![
                bind(plain(Key::Up), by(0, -1)),
                bind(plain(Key::Char('k')), by(0, -1)),
            ],
        ),
        entry(
            "keys.cmd.reset_span",
            vec![bind(plain(Key::Char('0')), KeyboardCommand::ResetSpan)],
        ),
        entry(
            "keys.cmd.commit",
            vec![
                bind(plain(Key::Enter), KeyboardCommand::Commit),
                bind(plain(Key::F(1)), KeyboardCommand::Commit),
            ],
        ),
        entry(
            "keys.cmd.cancel",
            vec![
                bind(plain(Key::Esc), KeyboardCommand::Cancel),
                bind(plain(Key::Char('q')), KeyboardCommand::Cancel),
            ],
        ),
    ]
}

/// The full per-scope binding catalog, in [`ALL_KEYBOARD_SCOPES`] order.
///
/// Chords are stored in canonical display form: lowercase letters stand
/// for their `x | X` resolver duals; case-significant bindings (`R`,
/// `D`) keep their exact character. The consistency tests replay every
/// chord through `resolve_key` and sweep the resolver's key universe
/// back against this catalog.
pub fn binding_catalog() -> Vec<ScopeBindings> {
    ALL_KEYBOARD_SCOPES
        .into_iter()
        .map(|scope| ScopeBindings {
            scope,
            label_key: scope_label_key(scope),
            entries: match scope {
                KeyboardScope::Board => board_scope(),
                KeyboardScope::Detail => detail_scope(),
                KeyboardScope::Form => form_scope(),
                KeyboardScope::Settings => settings_scope(),
                KeyboardScope::BoardEditor => board_editor_scope(),
                KeyboardScope::ConfirmDelete => confirm_scope(),
                KeyboardScope::ToolPicker => tool_picker_scope(),
                KeyboardScope::FilterBar => filter_bar_scope(),
                KeyboardScope::RightPane => right_pane_scope(),
                KeyboardScope::Moving => moving_scope(),
                KeyboardScope::Resize => resize_scope(),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::{KeyboardContext, resolve_key};

    fn context(scope: KeyboardScope, has_tool_focus: bool) -> KeyboardContext {
        KeyboardContext {
            scope,
            has_tool_focus,
        }
    }

    #[test]
    fn 카탈로그의_모든_바인딩은_resolve_key와_일치한다() {
        for scope_bindings in binding_catalog() {
            for entry in &scope_bindings.entries {
                let ctx = context(scope_bindings.scope, entry.requires_tool_focus);
                for binding in &entry.bindings {
                    for stroke in binding.chord.strokes() {
                        assert_eq!(
                            resolve_key(ctx, stroke),
                            Some(binding.command),
                            "{:?} 스코프의 카탈로그 chord {:?} ({})는 resolver와 일치해야 한다",
                            scope_bindings.scope,
                            binding.chord,
                            entry.label_key,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn 포커스_게이트_엔트리는_포커스_없이_해석되지_않는다() {
        for scope_bindings in binding_catalog() {
            for entry in &scope_bindings.entries {
                if !entry.requires_tool_focus {
                    continue;
                }
                let ctx = context(scope_bindings.scope, false);
                for binding in &entry.bindings {
                    for stroke in binding.chord.strokes() {
                        assert_eq!(
                            resolve_key(ctx, stroke),
                            None,
                            "{}는 포커스 없이 해석되면 안 된다",
                            entry.label_key,
                        );
                    }
                }
            }
        }
    }

    /// Sweep universe: every key the surface adapters can produce, with
    /// the modifier sets the resolver distinguishes. `alt` combos are
    /// excluded — they are uniformly rejected by `has_command_modifier`
    /// (and are not displayable bindings).
    fn stroke_universe() -> Vec<KeyStroke> {
        let mut keys = vec![
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
            Key::Enter,
            Key::Esc,
            Key::Tab,
            Key::BackTab,
            Key::PageUp,
            Key::PageDown,
            Key::Home,
            Key::End,
            Key::Backspace,
            Key::Space,
        ];
        keys.extend((1..=24).map(Key::F));
        keys.extend((' '..='~').map(Key::Char));

        let modifier_sets = [
            KeyModifiers::NONE,
            KeyModifiers {
                shift: true,
                ..KeyModifiers::NONE
            },
            KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
            KeyModifiers {
                meta: true,
                ..KeyModifiers::NONE
            },
            KeyModifiers {
                control: true,
                shift: true,
                ..KeyModifiers::NONE
            },
            KeyModifiers {
                meta: true,
                shift: true,
                ..KeyModifiers::NONE
            },
        ];

        keys.iter()
            .flat_map(|&key| {
                modifier_sets
                    .iter()
                    .map(move |&modifiers| KeyStroke::modified(key, modifiers))
            })
            .collect()
    }

    /// Keys equal up to the lowercase/uppercase resolver dual: the
    /// catalog stores one canonical case, the resolver accepts both.
    fn keys_match(catalog: Key, actual: Key) -> bool {
        if catalog == actual {
            return true;
        }
        match (catalog, actual) {
            (Key::Char(a), Key::Char(b)) => a.eq_ignore_ascii_case(&b),
            _ => false,
        }
    }

    /// Modifier equality for the sweep: `shift` is never part of a
    /// binding's identity (shifted characters arrive pre-shifted from
    /// the label parsers), so it is ignored.
    fn modifiers_match(catalog: KeyModifiers, actual: KeyModifiers) -> bool {
        catalog.control == actual.control
            && catalog.meta == actual.meta
            && catalog.alt == actual.alt
    }

    fn catalog_covers(
        entries: &[BindingEntry],
        stroke: KeyStroke,
        command: KeyboardCommand,
    ) -> bool {
        entries.iter().any(|entry| {
            entry.bindings.iter().any(|binding| {
                binding.command == command
                    && binding.chord.strokes().iter().any(|expanded| {
                        keys_match(expanded.key, stroke.key)
                            && modifiers_match(expanded.modifiers, stroke.modifiers)
                    })
            })
        })
    }

    #[test]
    fn resolve_key가_해석하는_모든_키는_카탈로그에_존재한다() {
        let catalog = binding_catalog();
        let universe = stroke_universe();
        for scope_bindings in &catalog {
            for has_tool_focus in [false, true] {
                let ctx = context(scope_bindings.scope, has_tool_focus);
                for &stroke in &universe {
                    let Some(command) = resolve_key(ctx, stroke) else {
                        continue;
                    };
                    // Text entry is unbounded (every printable char) and
                    // deliberately uncataloged; the primary+W chord is a
                    // resolver-internal alias that re-dispatches Esc.
                    if matches!(command, KeyboardCommand::Text(_)) {
                        continue;
                    }
                    if stroke.modifiers.is_primary_shortcut()
                        && matches!(stroke.key, Key::Char('w' | 'W'))
                    {
                        continue;
                    }
                    assert!(
                        catalog_covers(&scope_bindings.entries, stroke, command),
                        "{:?} 스코프에서 {stroke:?} → {command:?} 바인딩이 카탈로그에 없다",
                        scope_bindings.scope,
                    );
                }
            }
        }
    }

    #[test]
    fn 카탈로그는_모든_스코프를_한_번씩_포함한다() {
        let catalog = binding_catalog();
        assert_eq!(catalog.len(), ALL_KEYBOARD_SCOPES.len());
        for (scope_bindings, expected) in catalog.iter().zip(ALL_KEYBOARD_SCOPES) {
            assert_eq!(scope_bindings.scope, expected);
            assert_eq!(scope_bindings.label_key, scope_label_key(expected));
            assert!(
                !scope_bindings.entries.is_empty(),
                "{expected:?} 스코프의 카탈로그가 비어 있다",
            );
        }
    }

    #[test]
    fn 물음표는_보드_스코프에서_치트시트를_연다() {
        // Shift+/ 로 입력되는 '?' — label 파서가 문자를 그대로 넘기므로
        // shift 플래그 유무와 무관하게 해석되어야 한다.
        let ctx = context(KeyboardScope::Board, false);
        assert_eq!(
            resolve_key(ctx, KeyStroke::plain(Key::Char('?'))),
            Some(KeyboardCommand::ShowCheatsheet),
        );
        assert_eq!(
            resolve_key(
                ctx,
                KeyStroke::modified(
                    Key::Char('?'),
                    KeyModifiers {
                        shift: true,
                        ..KeyModifiers::NONE
                    },
                ),
            ),
            Some(KeyboardCommand::ShowCheatsheet),
        );
    }
}
