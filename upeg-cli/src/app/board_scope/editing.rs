use std::fmt::Write as _;

use upeg_core::{BoardKey, ColSpan, PinColorHex, PinId, PinSpan, Placement, RowSpan};
use upeg_runtime::{pegboard as geometry, toolbox_tool};
use upeg_sources::pegboard::{self, BoardVisibility, PegboardSelection, PegboardState};

use crate::error::CliError;
use crate::surfaces::cli::PinEditArgs;

use super::ensure_board_exists;

pub(in crate::app) fn create_board(title: &str, json: bool) -> Result<String, CliError> {
    pegboard::edit_state(|state| {
        let key = pegboard::add_board(state, title)
            .ok_or_else(|| CliError::tool_failed("board title must not be blank"))?;
        board_output(&key, title.trim(), "created", json)
    })
    .map_err(store_error)?
}

pub(super) fn rename_board(board: &BoardKey, title: &str, json: bool) -> Result<String, CliError> {
    require_personal_board(board)?;
    edit_board(board, |state| {
        if !pegboard::rename_board(state, board.as_str(), title) {
            return Err(CliError::tool_failed("board title must not be blank"));
        }
        board_output(board.as_str(), title.trim(), "renamed", json)
    })
}

pub(super) fn delete_board(board: &BoardKey, json: bool) -> Result<String, CliError> {
    require_personal_board(board)?;
    edit_board(board, |state| {
        let title = state
            .boards
            .iter()
            .find(|entry| entry.key == board.as_str())
            .map(|entry| entry.title.clone())
            .ok_or_else(|| CliError::tool_failed(format!("unknown board `{board}`")))?;
        pegboard::remove_board(state, board.as_str());
        if state.selection.board_key.as_deref() == Some(board.as_str()) {
            state.selection = PegboardSelection::default();
        }
        board_output(board.as_str(), &title, "deleted", json)
    })
}

fn require_personal_board(board: &BoardKey) -> Result<(), CliError> {
    if BoardVisibility::from_process().is_project_board(board.as_str()) {
        return Err(CliError::tool_failed(format!(
            "board `{board}` is declared in the project manifest"
        )));
    }
    Ok(())
}

/// Run a board edit in the same transaction that reads the current layout.
/// Collision routines only lay out loaded tools. Retain unavailable rows
/// even if one of those routines drops them while moving a visible pin.
pub(super) fn edit_board<T>(
    board: &BoardKey,
    edit: impl FnOnce(&mut PegboardState) -> Result<T, CliError>,
) -> Result<T, CliError> {
    pegboard::edit_state(|state| {
        ensure_board_exists(state, board)?;
        let unavailable: Vec<_> = state
            .layouts
            .get(board.as_str())
            .into_iter()
            .flatten()
            .filter(|pin| toolbox_tool(&pin.tool_id).is_none())
            .cloned()
            .collect();
        let outcome = edit(state)?;
        if let Some(layout) = state.layouts.get_mut(board.as_str()) {
            for pin in unavailable {
                if !layout.iter().any(|entry| entry.pin_id == pin.pin_id) {
                    layout.push(pin);
                }
            }
        }
        Ok(outcome)
    })
    .map_err(store_error)?
}

pub(super) fn list_pins(board: &BoardKey, json: bool) -> Result<String, CliError> {
    let state = pegboard::load_persisted_state().map_err(store_error)?;
    ensure_board_exists(&state, board)?;
    let pins = state.layouts.get(board.as_str()).into_iter().flatten();
    if json {
        return json_line(&pins.map(pin_json).collect::<Vec<_>>());
    }
    let mut output = String::new();
    for pin in pins {
        let _ = writeln!(output, "{}", pin_row(pin));
    }
    Ok(output)
}

pub(super) fn show_pin(board: &BoardKey, pin_id: &str, json: bool) -> Result<String, CliError> {
    let id = PinId::parse(pin_id).map_err(input_error)?;
    let state = pegboard::load_persisted_state().map_err(store_error)?;
    ensure_board_exists(&state, board)?;
    let pin = state
        .layouts
        .get(board.as_str())
        .into_iter()
        .flatten()
        .find(|pin| pin.pin_id == id)
        .ok_or_else(|| unknown_pin(board, &id))?;
    pin_output(pin, json)
}

pub(super) fn edit_pin(board: &BoardKey, args: PinEditArgs) -> Result<String, CliError> {
    let id = PinId::parse(&args.pin_id).map_err(input_error)?;
    let span = args.size.as_deref().map(parse_span).transpose()?;
    let color = args
        .color
        .as_deref()
        .map(PinColorHex::parse)
        .transpose()
        .map_err(input_error)?;
    let edit_span = args.reset_size || span.is_some();
    let edit_color = args.reset_color || color.is_some();
    if !edit_span && !edit_color {
        return Err(CliError::tool_failed(
            "pass --size, --reset-size, --color, or --reset-color to edit a pin",
        ));
    }
    edit_board(board, |state| {
        let layout = state
            .layouts
            .get_mut(board.as_str())
            .ok_or_else(|| unknown_pin(board, &id))?;
        let pin = layout
            .iter_mut()
            .find(|pin| pin.pin_id == id)
            .ok_or_else(|| unknown_pin(board, &id))?;
        if edit_color {
            pin.color = color;
        }
        if edit_span && pin.span != span {
            pin.span = span;
            if toolbox_tool(&pin.tool_id).is_some() {
                let (x, y) = (pin.x, pin.y);
                let _ = geometry::place_tool_with_push(layout, id.as_str(), x, y);
            }
        }
        let pin = layout
            .iter()
            .find(|pin| pin.pin_id == id)
            .ok_or_else(|| unknown_pin(board, &id))?;
        pin_output(pin, args.json)
    })
}

fn parse_span(raw: &str) -> Result<PinSpan, CliError> {
    let invalid = || CliError::tool_failed(format!("invalid size `{raw}`; expected COLSxROWS"));
    let (cols, rows) = raw.split_once(['x', 'X']).ok_or_else(invalid)?;
    let cols = ColSpan::new(cols.parse().map_err(|_| invalid())?).map_err(input_error)?;
    let rows = RowSpan::new(rows.parse().map_err(|_| invalid())?).map_err(input_error)?;
    Ok(PinSpan::new(cols, rows))
}

fn unknown_pin(board: &BoardKey, id: &PinId) -> CliError {
    CliError::tool_failed(format!(
        "unknown pin `{id}` on board `{board}`; use pin-list"
    ))
}

fn effective_size(pin: &Placement) -> Option<(u16, u16)> {
    pin.span
        .map(PinSpan::grid_span)
        .or_else(|| toolbox_tool(&pin.tool_id).map(|tool| tool.pegboard_units.grid_span()))
}

fn pin_json(pin: &Placement) -> serde_json::Value {
    serde_json::json!({
        "pin_id": pin.pin_id,
        "tool_id": pin.tool_id,
        "cell": { "row": pin.y, "col": pin.x },
        "size": effective_size(pin).map(|(cols, rows)| serde_json::json!({ "cols": cols, "rows": rows })),
        "size_override": pin.span.map(|span| serde_json::json!({
            "cols": span.cols.get(), "rows": span.rows.get()
        })),
        "color": pin.color,
        "args_preset": pin.args_preset,
    })
}

fn pin_row(pin: &Placement) -> String {
    let size = effective_size(pin).map_or_else(
        || "unknown".to_string(),
        |(cols, rows)| format!("{cols}x{rows}"),
    );
    let color = pin.color.as_ref().map_or("default", PinColorHex::as_str);
    format!(
        "{}\t{}\t{},{}\t{size}\t{color}",
        pin.pin_id, pin.tool_id, pin.y, pin.x
    )
}

fn pin_output(pin: &Placement, json: bool) -> Result<String, CliError> {
    if json {
        return json_line(&pin_json(pin));
    }
    Ok(format!("{}\n", pin_row(pin)))
}

fn board_output(key: &str, title: &str, action: &str, json: bool) -> Result<String, CliError> {
    if json {
        return json_line(&serde_json::json!({ "key": key, "title": title, "action": action }));
    }
    Ok(format!("{key}\n"))
}

fn json_line(value: &impl serde::Serialize) -> Result<String, CliError> {
    let body = serde_json::to_string_pretty(value).map_err(input_error)?;
    Ok(format!("{body}\n"))
}

fn input_error(error: impl std::fmt::Display) -> CliError {
    CliError::tool_failed(error.to_string())
}

fn store_error(error: pegboard::PegboardStateError) -> CliError {
    CliError::tool_failed(format!("pegboard: {error}"))
}
