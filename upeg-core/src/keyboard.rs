//! Surface-neutral keyboard vocabulary and command resolution.
//!
//! Platform adapters translate their raw key events into [`KeyStroke`].
//! Surface state machines then apply the resulting [`KeyboardCommand`].
//! This keeps key binding policy out of TUI/GUI rendering code.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
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
    Char(char),
    F(u8),
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyModifiers {
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
    pub shift: bool,
}

impl KeyModifiers {
    pub const NONE: Self = Self {
        control: false,
        alt: false,
        meta: false,
        shift: false,
    };

    pub const fn meta_or_control(self) -> bool {
        self.meta || self.control
    }

    pub const fn has_command_modifier(self) -> bool {
        self.control || self.alt || self.meta
    }

    pub const fn is_primary_shortcut(self) -> bool {
        self.meta_or_control() && !self.alt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyStroke {
    pub key: Key,
    pub modifiers: KeyModifiers,
}

impl KeyStroke {
    pub const fn plain(key: Key) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::NONE,
        }
    }

    pub const fn modified(key: Key, modifiers: KeyModifiers) -> Self {
        Self { key, modifiers }
    }
}

impl From<Key> for KeyStroke {
    fn from(key: Key) -> Self {
        Self::plain(key)
    }
}

/// Parse a surface key label into the shared keyboard vocabulary.
pub fn key_from_label(label: &str, shift: bool) -> Option<Key> {
    let key = match label {
        "ArrowUp" => Key::Up,
        "ArrowDown" => Key::Down,
        "ArrowLeft" => Key::Left,
        "ArrowRight" => Key::Right,
        "Enter" => Key::Enter,
        "Escape" => Key::Esc,
        "Tab" if shift => Key::BackTab,
        "Tab" => Key::Tab,
        "Backspace" => Key::Backspace,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        " " | "Space" => Key::Space,
        value => {
            if let Some(number) = value.strip_prefix('F') {
                Key::F(number.parse().ok()?)
            } else {
                let mut chars = value.chars();
                let ch = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                Key::Char(ch)
            }
        }
    };
    Some(key)
}

/// Parse a surface key label plus modifiers into a shared keystroke.
pub fn key_stroke_from_label(label: &str, modifiers: KeyModifiers) -> Option<KeyStroke> {
    key_from_label(label, modifiers.shift).map(|key| KeyStroke::modified(key, modifiers))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDirection {
    Previous,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageDirection {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardSlot(u8);

impl BoardSlot {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 9;

    pub const fn new(slot: u8) -> Option<Self> {
        if slot >= Self::MIN && slot <= Self::MAX {
            Some(Self(slot))
        } else {
            None
        }
    }

    pub const fn as_u8(self) -> u8 {
        self.0
    }

    pub const fn zero_based_index(self) -> usize {
        (self.0 - 1) as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardScope {
    Board,
    Detail,
    Form,
    Settings,
    BoardEditor,
    /// Generic yes/no confirmation scope (y/Enter/F1 = confirm,
    /// n/q/Esc = cancel). Shared by every destructive-confirm dialog —
    /// board delete and the modeless quit-confirm overlay both route
    /// through it, since their key policy is identical; only the host
    /// view decides what a confirm/cancel does.
    ConfirmDelete,
    ToolPicker,
    FilterBar,
    RightPane,
    Moving,
    /// Keyboard resize of the focused pin, symmetric to [`Moving`]:
    /// entered with `e` from board scope (focused pin required),
    /// arrows/hjkl grow/shrink the span, `Enter`/F1 commit, `Esc`/`q`
    /// cancel, `0` resets to the manifest footprint. The resolver emits
    /// delta-carrying [`KeyboardCommand::ResizeBy`] values so the
    /// direction→delta mapping stays single-sourced here instead of
    /// being re-derived per surface.
    ///
    /// [`Moving`]: KeyboardScope::Moving
    Resize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardContext {
    pub scope: KeyboardScope,
    /// Whether a pin/tool is currently focused. The only remaining
    /// contextual gate after the modeless migration: board-management
    /// keys (`n`/`R`/`D`/`a`) are always live in board scope, while the
    /// per-pin keys (`p`/`c`/`m`/`[`/`]`) still need a focused tool to
    /// know what to act on.
    pub has_tool_focus: bool,
}

impl KeyboardContext {
    pub const fn new(scope: KeyboardScope) -> Self {
        Self {
            scope,
            has_tool_focus: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardCommand {
    Move(NavDirection),
    FocusNext,
    FocusPrevious,
    Page(PageDirection),
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
    SwitchBoard(BoardSlot),
    NewBoard,
    RenameBoard,
    DeleteBoard,
    OpenToolPicker,
    TogglePin,
    EditPinColor,
    Reorder(OrderDirection),
    /// Cmd/Ctrl + `[` — move the focused pin one slot earlier.
    /// Uses `move_pin` semantics (anchor swap) rather than the
    /// in-place layout `Reorder` re-sort. The two commands diverge for
    /// board-canvas focus state (Reorder requires a focused pin and
    /// produces a swap; MovePinPrev/Next is a global chord that targets
    /// the focused pin and animates it across boards).
    MovePinPrev,
    /// Cmd/Ctrl + `]` — symmetric to `MovePinPrev`.
    MovePinNext,
    StartMove,
    /// `e` with a focused pin — enter the resize scope for that pin
    /// (symmetric to [`StartMove`]).
    ///
    /// [`StartMove`]: KeyboardCommand::StartMove
    StartResize,
    /// Grow/shrink the focused pin's span by whole cells. Emitted only
    /// in [`KeyboardScope::Resize`]; the deltas are already mapped from
    /// the pressed direction (`→`/`l` = `cols: +1`, `←`/`h` = `cols: -1`,
    /// `↓`/`j` = `rows: +1`, `↑`/`k` = `rows: -1`) so surfaces apply
    /// them verbatim. Clamping to the legal span range
    /// (`ColSpan`/`RowSpan`) is the consumer's job.
    ResizeBy {
        cols: i8,
        rows: i8,
    },
    /// `0` in [`KeyboardScope::Resize`] — drop the user span override so
    /// the manifest footprint applies again (`0` follows the board
    /// scope's `0` = clear-filter idiom).
    ResetSpan,
    Commit,
    Confirm,
    Cancel,
    /// `?` (Shift+`/`) in board scope — open the keyboard-cheatsheet
    /// overlay. The overlay renders [`binding_catalog`] so the surface
    /// never hand-writes a shortcut table that could drift from this
    /// resolver.
    ///
    /// [`binding_catalog`]: crate::keyboard_catalog::binding_catalog
    ShowCheatsheet,
    Text(char),
    Backspace,
    /// Ctrl+U — clear the focused text buffer in one step (Form field
    /// draft / BoardEditor buffer / ToolPicker query / PinColorEditor
    /// draft), instead of repeated single-char Backspace.
    ClearInput,
    /// F2 — copy the current view's canonical output text to the
    /// system clipboard (Result: output/error text; Detail: tool id).
    Copy,
}

pub const fn resolve_key(context: KeyboardContext, stroke: KeyStroke) -> Option<KeyboardCommand> {
    // Cmd/Ctrl+W is the conventional desktop "close current surface"
    // chord. Rather than teaching every scope its own W binding, route it
    // to the same dismissal the scope already binds to `Esc` (Close for
    // Board/Detail/Form/Settings, Cancel for the editor/picker/confirm
    // scopes). Scopes without an `Esc` binding simply yield nothing.
    // Re-dispatching a bare `Esc` stroke can't re-enter this branch, since
    // `is_primary_shortcut()` is false for an unmodified key.
    if stroke.modifiers.is_primary_shortcut() && matches!(stroke.key, Key::Char('w' | 'W')) {
        return resolve_key(context, KeyStroke::plain(Key::Esc));
    }
    match context.scope {
        KeyboardScope::Moving => resolve_moving(stroke),
        KeyboardScope::Resize => resolve_resize(stroke),
        KeyboardScope::Board => resolve_board(context, stroke),
        KeyboardScope::Detail => resolve_detail(stroke),
        KeyboardScope::Form => resolve_form(stroke),
        KeyboardScope::Settings => resolve_settings(stroke),
        KeyboardScope::BoardEditor => resolve_board_editor(stroke),
        KeyboardScope::ConfirmDelete => resolve_confirm(stroke),
        KeyboardScope::ToolPicker => resolve_tool_picker(stroke),
        KeyboardScope::FilterBar => resolve_filter_bar(stroke),
        KeyboardScope::RightPane => resolve_right_pane(stroke),
    }
}

const fn resolve_board(context: KeyboardContext, stroke: KeyStroke) -> Option<KeyboardCommand> {
    if stroke.modifiers.is_primary_shortcut() && matches!(stroke.key, Key::Char('k' | 'K')) {
        return Some(KeyboardCommand::Search);
    }
    // Cmd/Ctrl + `[` / `]` — move the focused pin one slot in the
    // matching direction. The check sits ahead of the
    // `has_command_modifier() → None` early-out below.
    if stroke.modifiers.is_primary_shortcut() {
        match stroke.key {
            Key::Char('[') => return Some(KeyboardCommand::MovePinPrev),
            Key::Char(']') => return Some(KeyboardCommand::MovePinNext),
            // Cmd/Ctrl+Q — the conventional desktop quit chord. Sits ahead
            // of the `has_command_modifier() → None` early-out below, which
            // would otherwise swallow it; bare `q` keeps its own binding.
            Key::Char('q' | 'Q') => return Some(KeyboardCommand::Quit),
            _ => {}
        }
    }
    if stroke.modifiers.has_command_modifier() {
        return None;
    }
    match stroke.key {
        Key::Up | Key::Char('k' | 'K') => Some(KeyboardCommand::Move(NavDirection::Up)),
        Key::Down | Key::Char('j' | 'J') => Some(KeyboardCommand::Move(NavDirection::Down)),
        Key::Left | Key::Char('h' | 'H') => Some(KeyboardCommand::Move(NavDirection::Left)),
        Key::Right | Key::Char('l' | 'L') => Some(KeyboardCommand::Move(NavDirection::Right)),
        // Enter always runs/confirms (issue: Enter meant "open detail"
        // before, colliding with the universal "Enter = execute"
        // contract). Inspecting a tool's manifest now lives on its own
        // key, `o`, so the two intents stay distinguishable.
        Key::Enter | Key::Space | Key::F(1) => Some(KeyboardCommand::Run),
        Key::Char('o' | 'O') => Some(KeyboardCommand::Open),
        Key::Char('/') => Some(KeyboardCommand::Search),
        // `?` (Shift+`/` — the label parser hands the shifted character
        // through as-is) opens the keyboard cheatsheet. Shift is not a
        // command modifier, so the chord survives the modifier gate above.
        Key::Char('?') => Some(KeyboardCommand::ShowCheatsheet),
        Key::Esc => Some(KeyboardCommand::Close),
        // Bare `q` is also Quit (in addition to the Cmd/Ctrl+Q chord
        // handled above) now that the edit toggle is gone — the surface
        // routes it through a confirm-quit dialog so a stray keypress
        // can't drop the session.
        Key::Char('q' | 'Q') => Some(KeyboardCommand::Quit),
        Key::Char('s' | 'S') => Some(KeyboardCommand::OpenSettings),
        // `e` = resize the focused pin (the old edit-toggle key, rebound
        // after the modeless migration freed it). Gated on focus exactly
        // like `m`/StartMove — without a focused pin there is nothing to
        // resize.
        Key::Char('e' | 'E') if context.has_tool_focus => Some(KeyboardCommand::StartResize),
        Key::F(2) => Some(KeyboardCommand::Copy),
        Key::Char('b' | 'B') => Some(KeyboardCommand::CycleBoardFilter),
        Key::Char('t' | 'T') => Some(KeyboardCommand::CycleTagFilter),
        Key::Char('0') => Some(KeyboardCommand::ClearBoardFilter),
        Key::Char(c @ '1'..='9') => {
            let Some(slot) = BoardSlot::new((c as u8) - b'0') else {
                return None;
            };
            Some(KeyboardCommand::SwitchBoard(slot))
        }
        // Board-management keys are always live in board scope (modeless).
        Key::Char('n') => Some(KeyboardCommand::NewBoard),
        Key::Char('R') => Some(KeyboardCommand::RenameBoard),
        Key::Char('D') => Some(KeyboardCommand::DeleteBoard),
        Key::Char('a' | 'A') => Some(KeyboardCommand::OpenToolPicker),
        // Per-pin keys still need a focused tool to act on.
        Key::Char('p' | 'P') if context.has_tool_focus => Some(KeyboardCommand::TogglePin),
        Key::Char('c') if context.has_tool_focus => Some(KeyboardCommand::EditPinColor),
        Key::Char('m' | 'M') if context.has_tool_focus => Some(KeyboardCommand::StartMove),
        Key::Char('[') if context.has_tool_focus => {
            Some(KeyboardCommand::Reorder(OrderDirection::Previous))
        }
        Key::Char(']') if context.has_tool_focus => {
            Some(KeyboardCommand::Reorder(OrderDirection::Next))
        }
        Key::Tab => Some(KeyboardCommand::FocusNext),
        Key::BackTab => Some(KeyboardCommand::FocusPrevious),
        _ => None,
    }
}

const fn resolve_detail(stroke: KeyStroke) -> Option<KeyboardCommand> {
    if stroke.modifiers.is_primary_shortcut() && matches!(stroke.key, Key::Char('k' | 'K')) {
        return Some(KeyboardCommand::Search);
    }
    if stroke.modifiers.has_command_modifier() {
        return None;
    }
    match stroke.key {
        Key::F(1) | Key::Enter | Key::Char('r' | 'R') => Some(KeyboardCommand::Run),
        Key::F(2) => Some(KeyboardCommand::Copy),
        Key::Esc | Key::Char('q' | 'Q') => Some(KeyboardCommand::Close),
        Key::Char('s' | 'S') => Some(KeyboardCommand::OpenSettings),
        Key::Char('m' | 'M') => Some(KeyboardCommand::StartMove),
        Key::Char('/') => Some(KeyboardCommand::Search),
        Key::Char('b' | 'B') => Some(KeyboardCommand::CycleBoardFilter),
        Key::Char('t' | 'T') => Some(KeyboardCommand::CycleTagFilter),
        Key::Char('0') => Some(KeyboardCommand::ClearBoardFilter),
        Key::Tab => Some(KeyboardCommand::FocusNext),
        Key::BackTab => Some(KeyboardCommand::FocusPrevious),
        _ => None,
    }
}

const fn resolve_form(stroke: KeyStroke) -> Option<KeyboardCommand> {
    // Ctrl+U clears the focused field draft in one step. Checked ahead
    // of the command-modifier early-return below, since Ctrl is itself
    // a command modifier.
    if stroke.modifiers.control && matches!(stroke.key, Key::Char('u' | 'U')) {
        return Some(KeyboardCommand::ClearInput);
    }
    if stroke.modifiers.has_command_modifier() {
        return None;
    }
    match stroke.key {
        Key::Tab | Key::Down => Some(KeyboardCommand::FocusNext),
        Key::BackTab | Key::Up => Some(KeyboardCommand::FocusPrevious),
        Key::Left => Some(KeyboardCommand::Move(NavDirection::Left)),
        Key::Right => Some(KeyboardCommand::Move(NavDirection::Right)),
        Key::Backspace => Some(KeyboardCommand::Backspace),
        Key::Enter | Key::F(1) => Some(KeyboardCommand::Run),
        Key::Esc => Some(KeyboardCommand::Close),
        Key::Space => Some(KeyboardCommand::Text(' ')),
        Key::Char(c) => Some(KeyboardCommand::Text(c)),
        _ => None,
    }
}

const fn resolve_settings(stroke: KeyStroke) -> Option<KeyboardCommand> {
    if stroke.modifiers.has_command_modifier() {
        return None;
    }
    match stroke.key {
        Key::Down | Key::Tab | Key::Char('j' | 'J') => Some(KeyboardCommand::FocusNext),
        Key::Up | Key::BackTab | Key::Char('k' | 'K') => Some(KeyboardCommand::FocusPrevious),
        Key::Right | Key::Char('l' | 'L') => Some(KeyboardCommand::Move(NavDirection::Right)),
        Key::Left | Key::Char('h' | 'H') => Some(KeyboardCommand::Move(NavDirection::Left)),
        Key::Esc | Key::Char('q' | 'Q') => Some(KeyboardCommand::Close),
        _ => None,
    }
}

const fn resolve_board_editor(stroke: KeyStroke) -> Option<KeyboardCommand> {
    // Ctrl+U clears the buffer (BoardEditor) / draft (PinColorEditor,
    // which shares this scope) in one step. Checked ahead of the
    // command-modifier early-return below.
    if stroke.modifiers.control && matches!(stroke.key, Key::Char('u' | 'U')) {
        return Some(KeyboardCommand::ClearInput);
    }
    if stroke.modifiers.has_command_modifier() {
        return None;
    }
    match stroke.key {
        // F1 is the universal "confirm" key alongside Enter.
        Key::Enter | Key::F(1) => Some(KeyboardCommand::Commit),
        Key::Esc => Some(KeyboardCommand::Cancel),
        Key::Backspace => Some(KeyboardCommand::Backspace),
        Key::Space => Some(KeyboardCommand::Text(' ')),
        Key::Char(c) => Some(KeyboardCommand::Text(c)),
        _ => None,
    }
}

/// Shared yes/no confirmation resolver (see [`KeyboardScope::ConfirmDelete`]).
const fn resolve_confirm(stroke: KeyStroke) -> Option<KeyboardCommand> {
    if stroke.modifiers.has_command_modifier() {
        return None;
    }
    match stroke.key {
        Key::Char('y' | 'Y') | Key::Enter | Key::F(1) => Some(KeyboardCommand::Confirm),
        Key::Char('n' | 'N' | 'q' | 'Q') | Key::Esc => Some(KeyboardCommand::Cancel),
        _ => None,
    }
}

const fn resolve_tool_picker(stroke: KeyStroke) -> Option<KeyboardCommand> {
    // Ctrl+U clears the search/pin query in one step. Checked ahead of
    // the command-modifier early-return below.
    if stroke.modifiers.control && matches!(stroke.key, Key::Char('u' | 'U')) {
        return Some(KeyboardCommand::ClearInput);
    }
    if stroke.modifiers.has_command_modifier() {
        return None;
    }
    match stroke.key {
        Key::Backspace => Some(KeyboardCommand::Backspace),
        Key::Up => Some(KeyboardCommand::Move(NavDirection::Up)),
        Key::Down => Some(KeyboardCommand::Move(NavDirection::Down)),
        Key::Enter | Key::F(1) => Some(KeyboardCommand::Commit),
        Key::Esc => Some(KeyboardCommand::Cancel),
        Key::Space => Some(KeyboardCommand::Text(' ')),
        Key::Char(c) => Some(KeyboardCommand::Text(c)),
        _ => None,
    }
}

const fn resolve_filter_bar(stroke: KeyStroke) -> Option<KeyboardCommand> {
    if stroke.modifiers.has_command_modifier() {
        return None;
    }
    match stroke.key {
        Key::Left | Key::Char('h' | 'H') => Some(KeyboardCommand::Move(NavDirection::Left)),
        Key::Right | Key::Char('l' | 'L') => Some(KeyboardCommand::Move(NavDirection::Right)),
        Key::Home => Some(KeyboardCommand::Home),
        Key::End => Some(KeyboardCommand::End),
        Key::Enter | Key::Space => Some(KeyboardCommand::Open),
        _ => None,
    }
}

const fn resolve_right_pane(stroke: KeyStroke) -> Option<KeyboardCommand> {
    if stroke.modifiers.has_command_modifier() {
        return None;
    }
    match stroke.key {
        Key::Down | Key::Char('j' | 'J') => Some(KeyboardCommand::Move(NavDirection::Down)),
        Key::Up | Key::Char('k' | 'K') => Some(KeyboardCommand::Move(NavDirection::Up)),
        Key::PageDown => Some(KeyboardCommand::Page(PageDirection::Down)),
        Key::PageUp => Some(KeyboardCommand::Page(PageDirection::Up)),
        Key::Home => Some(KeyboardCommand::Home),
        Key::End => Some(KeyboardCommand::End),
        _ => None,
    }
}

const fn resolve_moving(stroke: KeyStroke) -> Option<KeyboardCommand> {
    if stroke.modifiers.has_command_modifier() {
        return None;
    }
    match stroke.key {
        Key::Esc | Key::Char('q' | 'Q') => Some(KeyboardCommand::Cancel),
        Key::Enter | Key::F(1) => Some(KeyboardCommand::Commit),
        Key::Left | Key::Char('h' | 'H') => Some(KeyboardCommand::Move(NavDirection::Left)),
        Key::Right | Key::Char('l' | 'L') => Some(KeyboardCommand::Move(NavDirection::Right)),
        Key::Up | Key::Char('k' | 'K') => Some(KeyboardCommand::Move(NavDirection::Up)),
        Key::Down | Key::Char('j' | 'J') => Some(KeyboardCommand::Move(NavDirection::Down)),
        _ => None,
    }
}

/// Single-cell grow/shrink step per keypress in the resize scope.
const RESIZE_STEP: i8 = 1;

/// Resize-scope resolver (see [`KeyboardScope::Resize`]). Mirrors
/// [`resolve_moving`]'s commit/cancel keys; the directional keys map to
/// span deltas instead of focus moves.
const fn resolve_resize(stroke: KeyStroke) -> Option<KeyboardCommand> {
    if stroke.modifiers.has_command_modifier() {
        return None;
    }
    match stroke.key {
        Key::Esc | Key::Char('q' | 'Q') => Some(KeyboardCommand::Cancel),
        Key::Enter | Key::F(1) => Some(KeyboardCommand::Commit),
        Key::Right | Key::Char('l' | 'L') => Some(KeyboardCommand::ResizeBy {
            cols: RESIZE_STEP,
            rows: 0,
        }),
        Key::Left | Key::Char('h' | 'H') => Some(KeyboardCommand::ResizeBy {
            cols: -RESIZE_STEP,
            rows: 0,
        }),
        Key::Down | Key::Char('j' | 'J') => Some(KeyboardCommand::ResizeBy {
            cols: 0,
            rows: RESIZE_STEP,
        }),
        Key::Up | Key::Char('k' | 'K') => Some(KeyboardCommand::ResizeBy {
            cols: 0,
            rows: -RESIZE_STEP,
        }),
        Key::Char('0') => Some(KeyboardCommand::ResetSpan),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    include!("keyboard_tests.inc.rs");
}
