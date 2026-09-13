#[derive(Debug, Clone, PartialEq)]
pub struct PinVisualState {
    pub class_name: String,
    pub cursor: &'static str,
    pub opacity: f32,
    pub focus_outline: &'static str,
}

pub fn pin_visual_state(
    tool_id: &'static str,
    edit: bool,
    drag_source: Option<&'static str>,
    keyboard_focused: bool,
) -> PinVisualState {
    let is_dragging_self = drag_source == Some(tool_id);
    let is_drop_candidate = edit && drag_source.is_some() && !is_dragging_self;

    let mut class_name = String::from(if edit { "cell edit" } else { "cell" });
    if is_drop_candidate {
        class_name.push_str(" drop-target");
    }
    if keyboard_focused {
        class_name.push_str(" kbd-focus");
    }

    let cursor = if !edit {
        "pointer"
    } else if is_dragging_self {
        "grabbing"
    } else {
        "grab"
    };
    let opacity = if is_dragging_self { 0.5 } else { 1.0 };
    let focus_outline = if keyboard_focused {
        "outline:2px solid var(--accent); outline-offset:2px;"
    } else {
        ""
    };

    PinVisualState {
        class_name,
        cursor,
        opacity,
        focus_outline,
    }
}
