use std::collections::BTreeMap;

use upeg_core::prefs::Tweaks;
use upeg_core::{
    DraftInputValue, InputFieldSpec, InputKind, InputName, InputSpec, OutputEntry, Placement,
    RecentSignal, ToolMeta,
};
use upeg_sources::pegboard::{self, BoardData, PegboardState};

use super::scroll::{Horizontal, ScrollOffset, Vertical};

mod active_run;
mod draft_seed;
mod form_value;
mod live_tail;
mod pin_color;

pub use active_run::{ActiveRun, RunToken, RunTokenMint};
use draft_seed::initial_tui_draft;
use form_value::{cycle_choice, cycle_multi_options_text, input_value_from_tui_draft};
pub use live_tail::LiveTail;
#[cfg(test)]
pub(crate) use live_tail::TUI_LIVE_TAIL_MAX_LINES;
pub(crate) use pin_color::{PIN_COLOR_PALETTE, pin_color_palette_for_digit};
pub use pin_color::{PinColorEditor, PinColorEditorDraft};

#[derive(Default, Clone, Debug, PartialEq, Eq)]
/// Current TUI screen state.
pub enum View {
    /// Tool list view.
    #[default]
    List,
    /// Details for the selected tool.
    Detail,
    /// Editing arguments for a tool, derived from its typed input spec.
    Form {
        tool_id: &'static str,
        form: TuiFormState,
    },
    Result {
        tool_id: &'static str,
        outputs: Vec<OutputEntry>,
        text: String,
        is_error: bool,
    },
    /// Edit shared user preferences (Locale / Theme / Accent). Activated
    /// by the `s` key from List or Detail. Persists to the shared
    /// Tweaks file so desktop UI / future TUI sessions pick up the
    /// change on next cold start.
    Settings {
        focused_field: SettingsField,
    },
    /// Single-line text editor for board CRUD. `mode` carries the
    /// intent (new board vs rename an existing board) so the same view
    /// covers both Phase 5/6 keystrokes (`n` / `R`) with one render
    /// path. Buffer accumulates the user's typing; ESC discards, Enter
    /// commits through `sources::add_board` / `sources::rename_board`.
    BoardEditor {
        mode: BoardEditMode,
        buffer: String,
    },
    /// Two-key (`y` / `n`) confirmation overlay used by `D` (delete
    /// board). Keeping the in-flight target on State (not in `View`)
    /// would mean every render that ignores delete still has to
    /// branch on it; a dedicated view keeps the responsibility
    /// scoped.
    ConfirmDeleteBoard {
        key: String,
        title: String,
    },
    PinColorEditor(PinColorEditor),
    /// Tool picker overlay shared by search (`/`, Ctrl/Cmd+K) and
    /// edit-mode add-tool (`a`). Mode keeps the selection side effect
    /// explicit: search opens the selected visible tool; pin toggles
    /// the selected toolbox tool on the active board.
    ToolPicker {
        mode: ToolPickerMode,
        return_to: ToolPickerReturn,
        query: String,
        cursor: usize,
    },
    /// Modeless accidental-quit guard. `q` opens this yes/no overlay
    /// instead of ending the session outright; Confirm quits, Cancel
    /// (n / q / Esc) returns to the board.
    ConfirmQuit,
    /// Human approval barrier in front of a gated Tool's dispatch
    /// (`upeg_runtime::tool_approval_policy`). The already-built args
    /// ride along so Confirm can stamp the reserved approval key onto
    /// exactly what the user was about to run — re-deriving them from
    /// the form after the fact would let the two drift apart.
    ConfirmApproval {
        tool_id: &'static str,
        args: serde_json::Value,
    },
    /// A dispatch is in flight. `tail` is the last few lines the tool
    /// has printed so far (`Msg::ToolProgress`), and `cancelling`
    /// records that Esc already asked the run to stop — a cancellation
    /// is a request, so the pane says "cancelling" rather than
    /// pretending the run is already over.
    Running {
        tool_id: &'static str,
        tail: LiveTail,
        cancelling: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolPickerMode {
    Search,
    Pin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolPickerReturnView {
    List,
    Detail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolPickerReturn {
    pub view: ToolPickerReturnView,
    pub focus: FocusArea,
}

impl ToolPickerReturn {
    pub const fn list(focus: FocusArea) -> Self {
        Self {
            view: ToolPickerReturnView::List,
            focus,
        }
    }

    pub const fn detail(focus: FocusArea) -> Self {
        Self {
            view: ToolPickerReturnView::Detail,
            focus,
        }
    }
}

/// Which body sub-surface a non-dialog View wants to dominate when the
/// terminal is too narrow to show both. Encoded as an enum so callers
/// (render, mouse routing, focus reconciliation) can never confuse the
/// two and so a `bool` argument can't carry the wrong semantics into a
/// layout call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BodyDominant {
    /// Pegboard wins the body. List, Detail.
    Board,
    /// Right-pane content wins the body. Form, Result.
    RightPane,
}

/// What the current `View` wants the body to render. Dialog views paint
/// a full-body modal and dispatch on their own variant; body views
/// share rendering with the layout-driven narrow swap.
///
/// Exhaustive over `View` — a new variant must declare itself here, so
/// neither render nor mouse routing nor focus cycling can silently
/// default to the wrong branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BodyPresentation {
    /// Full-body modal: Settings, BoardEditor, ConfirmDeleteBoard,
    /// ToolPicker. Dialog renderers own their input via keyboard;
    /// mouse clicks on the body region are inert.
    Dialog,
    /// Body sub-surfaces (pegboard grid + right pane). Narrow swap
    /// folds the non-dominant one.
    Surfaces(BodyDominant),
}

impl View {
    pub(crate) const fn body_presentation(&self) -> BodyPresentation {
        match self {
            Self::List | Self::Detail => BodyPresentation::Surfaces(BodyDominant::Board),
            Self::Form { .. } | Self::Result { .. } | Self::Running { .. } => {
                BodyPresentation::Surfaces(BodyDominant::RightPane)
            }
            Self::Settings { .. }
            | Self::BoardEditor { .. }
            | Self::ConfirmDeleteBoard { .. }
            | Self::PinColorEditor(_)
            | Self::ToolPicker { .. }
            | Self::ConfirmQuit
            | Self::ConfirmApproval { .. } => BodyPresentation::Dialog,
        }
    }
}

/// Two intents the [`View::BoardEditor`] covers. Carrying the key of
/// the target board through `Rename` keeps the render layer pure: it
/// can show "Rename {title}" without re-querying State to figure out
/// what's being renamed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoardEditMode {
    AddBoard,
    Rename { key: String, original_title: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// In-progress coordinate move operation. `base` is the committed board layout
/// at move start; `preview` is recomputed on every arrow key with the shared
/// push-placement primitive and becomes the committed layout only on Enter.
pub struct MoveMode {
    pub board: String,
    pub tool_id: &'static str,
    pub target_x: u16,
    pub target_y: u16,
    pub base: Vec<Placement>,
    pub preview: Vec<Placement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// In-progress pin-resize operation — the size twin of [`MoveMode`].
/// `cols`/`rows` are the currently previewed effective span; `base` is
/// the committed board layout at resize start and `preview` is
/// recomputed on every span change with the shared push-placement
/// primitive. Only Enter turns the preview into the committed layout.
pub struct ResizeMode {
    pub board: String,
    pub tool_id: &'static str,
    /// Currently previewed span width in board columns.
    pub cols: u16,
    /// Currently previewed span height in board rows.
    pub rows: u16,
    pub base: Vec<Placement>,
    pub preview: Vec<Placement>,
}

/// Cycle-able prefs the TUI Settings View exposes.
///
/// The order here drives the up/down cursor order in the rendered view.
/// Adding a field requires extending [`SettingsField::next`] /
/// [`SettingsField::prev`] so navigation stays exhaustive.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsField {
    #[default]
    Locale,
    Theme,
    Accent,
}

impl SettingsField {
    pub const fn next(self) -> Self {
        match self {
            Self::Locale => Self::Theme,
            Self::Theme => Self::Accent,
            Self::Accent => Self::Locale,
        }
    }
    pub const fn prev(self) -> Self {
        match self {
            Self::Locale => Self::Accent,
            Self::Theme => Self::Locale,
            Self::Accent => Self::Theme,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// TUI-local editable form state backed by canonical typed input metadata.
pub struct TuiFormState {
    /// Canonical typed input metadata for the selected Tool.
    pub input_spec: InputSpec,
    /// Per-field mutable state keyed by [`InputName`]. Field labels,
    /// descriptions, required flags, and kinds stay in [`InputFieldSpec`].
    pub fields: Vec<TuiFormFieldState>,
    /// Focused field index inside [`Self::fields`].
    pub focused: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One TUI form field draft, referencing [`InputFieldSpec`] by name.
pub struct TuiFormFieldState {
    pub name: InputName,
    pub draft: DraftInputValue,
}

impl TuiFormState {
    pub(crate) fn new(input_spec: InputSpec) -> Self {
        let fields = input_spec
            .fields
            .iter()
            .map(|field| TuiFormFieldState {
                name: field.name.clone(),
                draft: initial_tui_draft(field),
            })
            .collect();
        Self {
            input_spec,
            fields,
            focused: 0,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.fields.len()
    }

    pub(crate) fn focused_field_mut(&mut self) -> Option<&mut TuiFormFieldState> {
        self.fields.get_mut(self.focused)
    }

    pub(crate) fn spec_for_field(&self, field: &TuiFormFieldState) -> Option<&InputFieldSpec> {
        self.input_spec
            .fields
            .iter()
            .find(|spec| spec.name == field.name)
    }

    pub(crate) fn focused_spec(&self) -> Option<&InputFieldSpec> {
        self.fields
            .get(self.focused)
            .and_then(|field| self.spec_for_field(field))
    }

    pub(crate) fn push_char_to_focused_text(&mut self, c: char) -> bool {
        let Some(field) = self.focused_field_mut() else {
            return false;
        };
        if let DraftInputValue::Text(value) = &mut field.draft {
            value.push(c);
            true
        } else {
            false
        }
    }

    pub(crate) fn backspace_focused_text(&mut self) -> bool {
        let Some(field) = self.focused_field_mut() else {
            return false;
        };
        if let DraftInputValue::Text(value) = &mut field.draft {
            value.pop();
            true
        } else {
            false
        }
    }

    /// Ctrl+U — clear the focused field's text draft in one step,
    /// instead of repeated single-char Backspace.
    pub(crate) fn clear_focused_text(&mut self) -> bool {
        let Some(field) = self.focused_field_mut() else {
            return false;
        };
        if let DraftInputValue::Text(value) = &mut field.draft {
            value.clear();
            true
        } else {
            false
        }
    }

    pub(crate) fn cycle_focused_typed_value(&mut self, reverse: bool) -> bool {
        let Some(kind) = self.focused_spec().map(|spec| spec.kind.clone()) else {
            return false;
        };
        let Some(field) = self.focused_field_mut() else {
            return false;
        };
        match (kind, &mut field.draft) {
            (InputKind::Boolean, DraftInputValue::Boolean(value)) => {
                *value = !*value;
                true
            }
            (InputKind::Options(choices), DraftInputValue::Options(value)) => {
                *value = cycle_choice(value.as_deref(), &choices, reverse);
                true
            }
            (InputKind::MultiOptions(choices), DraftInputValue::Text(value)) => {
                cycle_multi_options_text(value, &choices, reverse);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn args(&self) -> Result<serde_json::Value, String> {
        let mut args = serde_json::Map::with_capacity(self.input_spec.fields.len());
        for spec in &self.input_spec.fields {
            let Some(field) = self.fields.iter().find(|field| field.name == spec.name) else {
                if spec.required {
                    return Err(format!("input `{}` is required", spec.name));
                }
                continue;
            };
            let Some(value) = input_value_from_tui_draft(spec, &field.draft)? else {
                if spec.required {
                    return Err(format!("input `{}` is required", spec.name));
                }
                continue;
            };
            args.insert(spec.name.as_str().to_string(), value.into_json_value());
        }
        Ok(serde_json::Value::Object(args))
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
/// Mutable state for the pure TUI state machine.
pub struct State {
    /// Index of the currently focused tool in the rendered list.
    pub cursor: usize,
    /// Vertical scroll offset for the pegboard grid content area.
    pub grid_scroll: ScrollOffset<Vertical>,
    /// Horizontal scroll offset for the pegboard grid content area.
    /// The canvas is always BOARD_COLS wide (see
    /// [`super::grid::grid_canvas_width`]); narrow terminals slice
    /// the canvas via this offset, mirroring the GUI's overflow-x:auto.
    pub grid_h_scroll: ScrollOffset<Horizontal>,
    /// Vertical scroll offset for the right-side detail/result pane.
    pub right_scroll: ScrollOffset<Vertical>,
    /// Horizontal scroll offset for the board filter bar.
    pub board_scroll: ScrollOffset<Horizontal>,
    /// Horizontal scroll offset for the tag filter bar.
    pub tag_scroll: ScrollOffset<Horizontal>,
    /// Keyboard focus target on the main TUI surface.
    pub focus: FocusArea,
    /// Focused option index in the board filter bar.
    pub board_cursor: usize,
    /// Focused option index in the tag filter bar.
    pub tag_cursor: usize,
    /// Active board/tag filters.
    pub filters: TuiFilters,
    /// Screen currently shown by the TUI.
    pub view: View,
    /// Shared user preferences (locale, theme, accent, …) loaded from
    /// the same file the desktop UI writes — see
    /// `upeg_runtime::persistence::bootstrap::bootstrap_tweaks`.
    pub tweaks: Tweaks,
    /// Cached boards, sourced from the shared pegboard state file
    /// at bootstrap and mutated locally during edit-mode operations
    /// (`Effect::SavePegboard` persists the change). Caching keeps the
    /// render loop off the filesystem on every keystroke.
    pub boards: Vec<BoardData>,
    /// Per-board coordinate placements — twin of `boards`, sourced from
    /// the same shared state file.
    pub layouts: BTreeMap<String, Vec<Placement>>,
    /// Active move preview. While set, arrow keys move the target coordinate,
    /// Enter commits the preview, and ESC/q cancels without touching the
    /// committed layout.
    pub move_mode: Option<MoveMode>,
    /// Active resize preview — mutually exclusive with `move_mode`.
    /// While set, arrows/hjkl grow or shrink the span, `0` drops the
    /// override back to the manifest footprint, Enter commits, and
    /// ESC/q cancels without touching the committed layout.
    pub resize_mode: Option<ResizeMode>,
    /// Recent-tool ranking loaded by the event loop. Keeping this on State
    /// lets the pure update/render path combine it with current pinned
    /// placements without reaching back into the filesystem.
    pub tool_picker_recent: Vec<RecentSignal>,
    /// Ephemeral one-line status toast (e.g. F2 clipboard copy result).
    /// Set by the event loop after an IO-backed `Effect` completes
    /// (clipboard has no synchronous result `update` could compute
    /// purely) and cleared on the next keystroke, so a headless host
    /// gets a visible message instead of silently doing nothing.
    pub status_message: Option<String>,
    /// The dispatch the event loop currently has in flight, if any.
    ///
    /// Deliberately *not* derived from `View`: leaving the running pane
    /// (the quit-confirm escalation walks out to `View::List`) drops the
    /// tail but never the run, and the model has to keep agreeing with
    /// the event loop about that. See [`ActiveRun`].
    pub active_run: Option<ActiveRun>,
    /// Mints the identity of the next run this session starts. Public
    /// only because `State` is built with functional-update syntax all
    /// over the crate; go through [`State::start_active_run`] rather
    /// than minting by hand, so the mint and `active_run` cannot drift.
    pub run_tokens: RunTokenMint,
}

impl State {
    /// Claim the identity of a run that is about to start. The token is
    /// what `Msg::ToolProgress` is stamped with, so only output from
    /// *this* run can reach the pane it opens.
    pub(crate) fn start_active_run(&mut self, tool_id: &'static str) -> RunToken {
        let run = self.run_tokens.mint();
        self.active_run = Some(ActiveRun { run, tool_id });
        run
    }

    /// Whether `run` is the dispatch the model still considers live.
    pub(crate) fn is_active_run(&self, run: RunToken) -> bool {
        self.active_run.is_some_and(|active| active.run == run)
    }

    pub(crate) fn with_filters(filters: TuiFilters) -> Self {
        let mut state = Self {
            filters,
            ..Self::default()
        };
        state.sync_filter_cursors();
        state
    }

    /// Read the underlying [`PegboardState`] view of `self` without
    /// cloning the per-call temporary every caller would otherwise have
    /// to build. `selection` is derived from the active filters so a
    /// persist-on-edit round-trips the same board/tag the user sees.
    pub(crate) fn pegboard_snapshot(&self) -> PegboardState {
        PegboardState {
            boards: self.boards.clone(),
            layouts: self.layouts.clone(),
            selection: pegboard::PegboardSelection {
                board_key: self.filters.board.as_deref().map(str::to_string),
                tag: self
                    .filters
                    .tag
                    .as_deref()
                    .unwrap_or(pegboard::ALL_TAG)
                    .to_string(),
            },
        }
    }

    pub(crate) fn replace_pegboard(&mut self, next: PegboardState) {
        let board_still_exists = match self.filters.board.as_deref() {
            Some(key) => next.boards.iter().any(|board| board.key == key),
            None => true,
        };
        if !board_still_exists {
            self.filters.board = BoardFilter::All;
        }
        self.boards = next.boards;
        self.layouts = next.layouts;
        self.sync_filter_cursors();
        let visible_len = self.visible_placements().len();
        if self.cursor >= visible_len {
            self.cursor = visible_len.saturating_sub(1);
        }
    }

    pub(crate) fn replace_pegboard_from_external_change(&mut self, next: PegboardState) {
        let selection = next.selection.clone();
        self.replace_pegboard(next);
        self.filters = initial_tui_filters(selection.board_key.as_deref(), Some(&selection.tag));
        self.sync_filter_cursors();
        let visible_len = self.visible_placements().len();
        if self.cursor >= visible_len {
            self.cursor = visible_len.saturating_sub(1);
        }
    }

    pub(crate) fn can_reload_external_pegboard(&self) -> bool {
        self.move_mode.is_none()
            && self.resize_mode.is_none()
            && !matches!(
                self.view,
                View::BoardEditor { .. }
                    | View::ConfirmDeleteBoard { .. }
                    | View::PinColorEditor(_)
                    | View::ToolPicker {
                        mode: ToolPickerMode::Pin,
                        ..
                    }
            )
    }

    pub(crate) fn can_reload_external_tweaks(&self) -> bool {
        !matches!(self.view, View::Settings { .. })
    }

    pub(crate) fn replace_tweaks(&mut self, next: Tweaks) {
        self.tweaks = next;
    }

    fn display_snapshot(&self) -> PegboardState {
        let mut snapshot = self.pegboard_snapshot();
        if let Some(move_mode) = &self.move_mode {
            snapshot
                .layouts
                .insert(move_mode.board.clone(), move_mode.preview.clone());
        }
        if let Some(resize_mode) = &self.resize_mode {
            snapshot
                .layouts
                .insert(resize_mode.board.clone(), resize_mode.preview.clone());
        }
        snapshot
    }

    /// Board filter chips for rendering. Unlike tag filters, `all` is NOT
    /// shown as a visible chip - it's the implicit default state. Only
    /// actual board keys from `self.boards` are exposed to the UI.
    /// This matches the Flutter GUI behavior where there's no "all" board chip.
    #[must_use]
    pub fn board_filter_options(&self) -> Vec<String> {
        let mut options = Vec::with_capacity(self.boards.len());
        options.extend(self.boards.iter().map(|b| b.key.clone()));
        options
    }

    /// Tag filter chips for the currently-active board filter (or the
    /// union of all boards when no board is selected).
    #[must_use]
    pub fn tag_filter_options(&self) -> Vec<String> {
        pegboard::tag_options_for_board_in(&self.pegboard_snapshot(), self.filters.board.as_deref())
    }

    /// Tools visible under the current board + tag filters. Surfaces
    /// the cached state, not the on-disk one.
    #[cfg(test)]
    pub(crate) fn visible_tools(&self) -> Vec<&'static ToolMeta> {
        pegboard::tools_for_board_and_tag_in(
            &self.display_snapshot(),
            self.filters.board.as_deref(),
            self.filters.tag.as_deref(),
        )
    }

    /// Visible tools paired with their cached canonical placement. The
    /// TUI render loop reads this once per frame to render from the
    /// in-memory state, not from disk.
    pub(crate) fn visible_placements(&self) -> Vec<(Placement, &'static ToolMeta)> {
        pegboard::placements_for_board_and_tag_in(
            &self.display_snapshot(),
            self.filters.board.as_deref(),
            self.filters.tag.as_deref(),
        )
    }

    /// Translate a digit shortcut (`1`–`9`) into the corresponding
    /// board key. `0` clears the board filter back to `all`. Digits
    /// are remapped so `1` selects the first board (index 0), since
    /// "all" is no longer a visible option. Returns `None` when the
    /// digit is out of range for the current board count.
    pub(crate) fn board_for_digit(&self, digit: char) -> Option<BoardFilter> {
        if digit == '0' {
            return Some(BoardFilter::All);
        }
        let idx = (digit.to_digit(10)? as usize).saturating_sub(1);
        self.board_filter_options()
            .get(idx)
            .map(|option| BoardFilter::selected(option.as_str()))
    }

    /// The board the user has currently filtered to, looked up in
    /// `self.boards`. Returns `None` when the filter is `all` or when
    /// the cached state no longer contains the active key. Phase 4+
    /// gates destructive edit-mode keys on this returning `Some`.
    pub(crate) fn current_board(&self) -> Option<&BoardData> {
        let key = self.filters.board.as_deref()?;
        self.boards.iter().find(|b| b.key == key)
    }

    pub(crate) fn sync_filter_cursors(&mut self) {
        normalize_tag_filter_for_board_in(&mut self.filters, &self.boards, &self.layouts);
        let board_opts = self.board_filter_options();
        self.board_cursor = filter_option_index(self.filters.board.as_deref(), &board_opts);
        let tag_opts = self.tag_filter_options();
        self.tag_cursor = filter_option_index(self.filters.tag.as_deref(), &tag_opts);
    }

    pub(crate) fn clamp_filter_cursors(&mut self, board_count: usize, tag_count: usize) {
        self.board_cursor = clamp_index(self.board_cursor, board_count);
        self.tag_cursor = clamp_index(self.tag_cursor, tag_count);
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
/// Keyboard focus target on the main TUI surface.
pub enum FocusArea {
    /// Board filter bar.
    Boards,
    /// Tag filter bar.
    Tags,
    /// Pegboard grid.
    #[default]
    Grid,
    /// Right detail/result pane.
    RightPane,
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
/// Active board/tag filters for the TUI surface.
pub struct TuiFilters {
    /// Active board filter.
    pub board: BoardFilter,
    /// Active tag filter.
    pub tag: TagFilter,
}

impl TuiFilters {
    pub(crate) fn from_options(board: Option<&str>, tag: Option<&str>) -> Self {
        Self {
            board: BoardFilter::from_option(board),
            tag: TagFilter::from_option(tag),
        }
    }

    pub(crate) fn select_board(&mut self, key: impl Into<String>) {
        self.board = BoardFilter::selected(key);
    }

    pub(crate) fn select_tag(&mut self, key: impl Into<String>) {
        self.tag = TagFilter::selected(key);
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
/// A board filter is either all boards or one concrete board key.
pub enum BoardFilter {
    #[default]
    All,
    Board(BoardKey),
}

impl BoardFilter {
    pub(crate) fn selected(key: impl Into<String>) -> Self {
        BoardKey::new(key).map_or(Self::All, Self::Board)
    }

    pub(crate) fn from_option(value: Option<&str>) -> Self {
        value.map_or(Self::All, Self::selected)
    }

    pub(crate) fn as_deref(&self) -> Option<&str> {
        match self {
            Self::All => None,
            Self::Board(key) => Some(key.as_str()),
        }
    }

    #[cfg(test)]
    pub(crate) const fn is_some(&self) -> bool {
        matches!(self, Self::Board(_))
    }

    pub(crate) const fn is_none(&self) -> bool {
        matches!(self, Self::All)
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
/// A tag filter is either all tags or one concrete tag key.
pub enum TagFilter {
    #[default]
    All,
    Tag(TagKey),
}

impl TagFilter {
    pub(crate) fn selected(key: impl Into<String>) -> Self {
        TagKey::new(key).map_or(Self::All, Self::Tag)
    }

    pub(crate) fn from_option(value: Option<&str>) -> Self {
        value.map_or(Self::All, Self::selected)
    }

    pub(crate) fn as_deref(&self) -> Option<&str> {
        match self {
            Self::All => None,
            Self::Tag(key) => Some(key.as_str()),
        }
    }

    #[cfg(test)]
    pub(crate) const fn is_some(&self) -> bool {
        matches!(self, Self::Tag(_))
    }

    #[cfg(test)]
    pub(crate) const fn is_none(&self) -> bool {
        matches!(self, Self::All)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Concrete board key selected by a filter.
pub struct BoardKey(String);

impl BoardKey {
    fn new(key: impl Into<String>) -> Option<Self> {
        let key = key.into();
        normalized_filter_value(&key).map(Self)
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Concrete tag key selected by a filter.
pub struct TagKey(String);

impl TagKey {
    fn new(key: impl Into<String>) -> Option<Self> {
        let key = key.into();
        normalized_filter_value(&key).map(Self)
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("all"))
        .map(ToOwned::to_owned)
}

pub fn initial_tui_filters(board: Option<&str>, tag: Option<&str>) -> TuiFilters {
    TuiFilters::from_options(board, tag)
}

fn cycle_filter_value(current: Option<&str>, options: &[String]) -> Option<String> {
    if options.is_empty() {
        return None;
    }
    let current = normalized_filter(current);
    let current_idx = current
        .as_deref()
        .and_then(|value| options.iter().position(|option| option == value))
        .unwrap_or(0);
    let next = options[(current_idx + 1) % options.len()].as_str();
    normalized_filter(Some(next))
}

pub fn cycle_board_filter(current: &BoardFilter, options: &[String]) -> BoardFilter {
    if options.is_empty() {
        return BoardFilter::All;
    }
    // When current is All, go directly to first board (index 0)
    if current.is_none() {
        return BoardFilter::selected(options.first().map(String::as_str).unwrap_or(""));
    }
    // For other cases, use the generic cycle logic
    BoardFilter::from_option(cycle_filter_value(current.as_deref(), options).as_deref())
}

pub fn cycle_tag_filter(current: &TagFilter, options: &[String]) -> TagFilter {
    TagFilter::from_option(cycle_filter_value(current.as_deref(), options).as_deref())
}

pub(crate) fn filter_option_index(current: Option<&str>, options: &[String]) -> usize {
    let current = normalized_filter(current);
    current
        .as_deref()
        .and_then(|value| options.iter().position(|option| option == value))
        .unwrap_or(0)
}

fn clamp_index(index: usize, len: usize) -> usize {
    index.min(len.saturating_sub(1))
}

/// State-aware variant of `normalize_tag_filter_for_board`. The
/// `layouts` map is borrowed instead of `&State` so callers (notably
/// `sync_filter_cursors`) can mutate `filters` and read `layouts` in
/// the same `&mut self` block without splitting the borrow.
pub(crate) fn normalize_tag_filter_for_board_in(
    filters: &mut TuiFilters,
    boards: &[BoardData],
    layouts: &BTreeMap<String, Vec<Placement>>,
) {
    let Some(tag) = filters.tag.as_deref() else {
        return;
    };
    let snapshot = PegboardState {
        boards: boards.to_vec(),
        layouts: layouts.clone(),
        selection: pegboard::PegboardSelection::default(),
    };
    let options = pegboard::tag_options_for_board_in(&snapshot, filters.board.as_deref());
    if !options.iter().any(|option| option == tag) {
        filters.tag = TagFilter::All;
    }
}

fn normalized_filter_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case(pegboard::ALL_TAG) {
        None
    } else {
        Some(trimmed.to_string())
    }
}
