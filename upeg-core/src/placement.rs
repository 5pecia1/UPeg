//! User-controlled placement of one tool instance on a board.

use crate::{ArgsPreset, PinColorHex, PinId, PinSpan};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// User-controlled placement of a tool on a board. Position is owned by
/// the user; size comes from the tool's manifest (`pegboard_units`)
/// unless the user set a per-pin [`PinSpan`] override in `span`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "PlacementWire"))]
pub struct Placement {
    pub pin_id: PinId,
    pub tool_id: String,
    pub x: u16,
    pub y: u16,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub color: Option<PinColorHex>,
    /// User-set size override. `None` keeps the manifest footprint.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub span: Option<PinSpan>,
    /// User-saved argument preset applied when invoking from this pin.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub args_preset: Option<ArgsPreset>,
}

impl Placement {
    pub fn new(tool_id: impl Into<String>, x: u16, y: u16) -> Self {
        let tool_id = tool_id.into();
        Self {
            pin_id: PinId::from_legacy_tool_id(&tool_id),
            tool_id,
            x,
            y,
            color: None,
            span: None,
            args_preset: None,
        }
    }

    pub fn with_pin_id(mut self, pin_id: PinId) -> Self {
        self.pin_id = pin_id;
        self
    }

    pub fn with_color(mut self, color: Option<PinColorHex>) -> Self {
        self.color = color;
        self
    }

    pub fn with_span(mut self, span: Option<PinSpan>) -> Self {
        self.span = span;
        self
    }

    pub fn with_args_preset(mut self, args_preset: Option<ArgsPreset>) -> Self {
        self.args_preset = args_preset;
        self
    }
}

#[cfg(feature = "serde")]
#[derive(Deserialize)]
struct PlacementWire {
    pin_id: Option<PinId>,
    tool_id: String,
    x: u16,
    y: u16,
    #[serde(default)]
    color: Option<PinColorHex>,
    #[serde(default)]
    span: Option<PinSpan>,
    #[serde(default)]
    args_preset: Option<ArgsPreset>,
}

#[cfg(feature = "serde")]
impl From<PlacementWire> for Placement {
    fn from(wire: PlacementWire) -> Self {
        let legacy_id = PinId::from_legacy_tool_id(&wire.tool_id);
        Self {
            pin_id: wire.pin_id.unwrap_or(legacy_id),
            tool_id: wire.tool_id,
            x: wire.x,
            y: wire.y,
            color: wire.color,
            span: wire.span,
            args_preset: wire.args_preset,
        }
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn legacy_json_gets_stable_pin_id_and_new_ids_round_trip() {
        let legacy: Placement =
            serde_json::from_str(r#"{"tool_id":"num.hex_to_decimal","x":1,"y":2}"#).unwrap();
        assert_eq!(legacy.pin_id.as_str(), "num.hex_to_decimal");

        let second = Placement::new("num.hex_to_decimal", 3, 4)
            .with_pin_id(PinId::parse("01995a93-5165-7ee0-8bd6-d0cfba6f2727").unwrap());
        let json = serde_json::to_string(&second).unwrap();
        let restored: Placement = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, second);
    }

    #[test]
    fn explicit_blank_or_padded_pin_id_is_rejected() {
        for pin_id in ["", " padded "] {
            let json =
                format!(r#"{{"pin_id":"{pin_id}","tool_id":"num.hex_to_decimal","x":0,"y":0}}"#);
            assert!(serde_json::from_str::<Placement>(&json).is_err());
        }
    }
}
