use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers as CrosstermKeyModifiers, MouseButton,
    MouseEventKind,
};
use ratatui::layout::Rect;
use upeg_core::{KeyModifiers, KeyStroke, ToolMeta};

pub use upeg_core::Key;

use super::model::{PresentationHost, RunToken};
use crate::domain::execution::dispatch::Outcome;

/// Phase of a single pointer (button) event.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PointerPhase {
    Down,
    Drag,
    Up,
}

/// Axis a wheel/scroll event scrolls along.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ScrollAxis {
    Vertical,
    Horizontal,
}

/// A single discrete scroll tick. `forward` means "down" on the vertical
/// axis and "right" on the horizontal axis — i.e. advances the content
/// in the natural reading direction for the surface.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ScrollDelta {
    pub axis: ScrollAxis,
    pub forward: bool,
}

impl ScrollDelta {
    pub const VERTICAL_FORWARD: Self = Self {
        axis: ScrollAxis::Vertical,
        forward: true,
    };
    pub const VERTICAL_BACK: Self = Self {
        axis: ScrollAxis::Vertical,
        forward: false,
    };
    pub const HORIZONTAL_FORWARD: Self = Self {
        axis: ScrollAxis::Horizontal,
        forward: true,
    };
    pub const HORIZONTAL_BACK: Self = Self {
        axis: ScrollAxis::Horizontal,
        forward: false,
    };
}

/// Logical mouse input. Kept separate from crossterm so mouse behavior can be
/// tested without a terminal session.
///
/// Splitting the variants into `Pointer(_)` vs `Scroll(_)` means routing
/// code can match the kind of event once and then parameterize the rest
/// of the handler on the inner value — the previous flat enum forced
/// four near-identical scroll arms.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MouseKind {
    Pointer(PointerPhase),
    Scroll(ScrollDelta),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Mouse {
    pub column: u16,
    pub row: u16,
    pub kind: MouseKind,
}

/// Messages accepted by the pure TUI update function.
///
/// Stored placement coordinates are accessed via [`super::grid::set_placement_hints`]
/// (a thread-local) rather than threaded through every message — the
/// TUI is single-threaded and the render loop refreshes the cache once
/// per frame, so passing it through every key/mouse path would just
/// inflate the surface area without changing behavior.
#[derive(Debug)]
pub enum Msg<'a> {
    KeyPress {
        stroke: KeyStroke,
        tools: &'a [&'static ToolMeta],
        area: Option<Rect>,
    },
    Mouse {
        mouse: Mouse,
        tools: &'a [&'static ToolMeta],
        area: Rect,
    },
    ToolDone {
        run: RunToken,
        host: PresentationHost,
        tool_id: &'static str,
        outcome: Outcome,
    },
    /// One incremental output chunk from the dispatch currently in
    /// flight. Best-effort by construction (the final `ToolDone`
    /// envelope is the contract), so a dropped or late event only ever
    /// costs a line of the live tail.
    ///
    /// `run` names which dispatch produced it. A chunk that names a run
    /// the model no longer considers active is a straggler from a
    /// finished run and must not be folded into whatever pane happens to
    /// be on screen.
    ToolProgress {
        run: RunToken,
        event: upeg_runtime::ProgressEvent,
    },
}

pub(crate) fn key_stroke_from_crossterm(event: KeyEvent) -> Option<KeyStroke> {
    if event.kind != KeyEventKind::Press {
        return None;
    }
    let key = key_code_to_key(event.code)?;
    Some(KeyStroke::modified(
        key,
        key_modifiers_from_crossterm(event.modifiers),
    ))
}

const fn key_code_to_key(code: KeyCode) -> Option<Key> {
    match code {
        KeyCode::Up => Some(Key::Up),
        KeyCode::Down => Some(Key::Down),
        KeyCode::Left => Some(Key::Left),
        KeyCode::Right => Some(Key::Right),
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Esc => Some(Key::Esc),
        KeyCode::Tab => Some(Key::Tab),
        KeyCode::BackTab => Some(Key::BackTab),
        KeyCode::PageUp => Some(Key::PageUp),
        KeyCode::PageDown => Some(Key::PageDown),
        KeyCode::Home => Some(Key::Home),
        KeyCode::End => Some(Key::End),
        KeyCode::Backspace => Some(Key::Backspace),
        KeyCode::F(n) => Some(Key::F(n)),
        KeyCode::Char(' ') => Some(Key::Space),
        KeyCode::Char(c) => Some(Key::Char(c)),
        _ => None,
    }
}

const fn key_modifiers_from_crossterm(modifiers: CrosstermKeyModifiers) -> KeyModifiers {
    KeyModifiers {
        control: modifiers.contains(CrosstermKeyModifiers::CONTROL),
        alt: modifiers.contains(CrosstermKeyModifiers::ALT),
        meta: modifiers.contains(CrosstermKeyModifiers::META),
        shift: modifiers.contains(CrosstermKeyModifiers::SHIFT),
    }
}

pub(crate) const fn mouse_kind_from_crossterm(kind: MouseEventKind) -> Option<MouseKind> {
    match kind {
        MouseEventKind::Down(MouseButton::Left) => Some(MouseKind::Pointer(PointerPhase::Down)),
        MouseEventKind::Drag(MouseButton::Left) => Some(MouseKind::Pointer(PointerPhase::Drag)),
        MouseEventKind::Up(MouseButton::Left) => Some(MouseKind::Pointer(PointerPhase::Up)),
        MouseEventKind::ScrollUp => Some(MouseKind::Scroll(ScrollDelta::VERTICAL_BACK)),
        MouseEventKind::ScrollDown => Some(MouseKind::Scroll(ScrollDelta::VERTICAL_FORWARD)),
        MouseEventKind::ScrollLeft => Some(MouseKind::Scroll(ScrollDelta::HORIZONTAL_BACK)),
        MouseEventKind::ScrollRight => Some(MouseKind::Scroll(ScrollDelta::HORIZONTAL_FORWARD)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossterm_key_adapter_maps_only_press_events_to_typed_key() {
        let release = KeyEvent::new_with_kind(
            KeyCode::Char('k'),
            CrosstermKeyModifiers::CONTROL,
            KeyEventKind::Release,
        );

        assert_eq!(key_stroke_from_crossterm(release), None);

        let press = KeyEvent::new_with_kind(
            KeyCode::Char('k'),
            CrosstermKeyModifiers::CONTROL,
            KeyEventKind::Press,
        );
        let stroke = key_stroke_from_crossterm(press).expect("press event should map");

        assert_eq!(stroke.key, Key::Char('k'));
        assert!(stroke.modifiers.control);
        assert!(!stroke.modifiers.meta);
    }

    #[test]
    fn crossterm_key_adapter_preserves_backtab_and_modifiers() {
        let event = KeyEvent::new_with_kind(
            KeyCode::BackTab,
            CrosstermKeyModifiers::SHIFT,
            KeyEventKind::Press,
        );
        let stroke = key_stroke_from_crossterm(event).expect("backtab should map");

        assert_eq!(stroke.key, Key::BackTab);
        assert!(stroke.modifiers.shift);
    }
}
