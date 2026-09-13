use upeg_core::{PinColorError, PinColorHex};

const PALETTE_FIRST_DIGIT: char = '1';
const PIN_COLOR_PALETTE_LEN: usize = 9;

pub(crate) const PIN_COLOR_PALETTE: [&str; PIN_COLOR_PALETTE_LEN] = [
    "#EF4444", "#F97316", "#F59E0B", "#22C55E", "#14B8A6", "#3B82F6", "#8B5CF6", "#EC4899",
    "#64748B",
];

pub(crate) fn pin_color_palette_for_digit(digit: char) -> Option<PinColorHex> {
    let index = digit
        .to_digit(10)?
        .checked_sub(PALETTE_FIRST_DIGIT.to_digit(10)?)? as usize;
    PIN_COLOR_PALETTE
        .get(index)
        .and_then(|color| PinColorHex::parse(color).ok())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinColorEditor {
    pub board: String,
    pub tool_id: String,
    pub original: Option<PinColorHex>,
    pub draft: PinColorEditorDraft,
    pub error: Option<String>,
}

impl PinColorEditor {
    pub(crate) fn new(board: String, tool_id: String, original: Option<PinColorHex>) -> Self {
        let draft = original
            .clone()
            .map_or(PinColorEditorDraft::Empty, PinColorEditorDraft::Existing);
        Self {
            board,
            tool_id,
            original,
            draft,
            error: None,
        }
    }

    pub(crate) fn push_char(&mut self, c: char) {
        self.error = None;
        match &mut self.draft {
            PinColorEditorDraft::Custom(value) => value.push(c),
            _ if c == '#' => self.draft = PinColorEditorDraft::Custom(c.to_string()),
            _ => {}
        }
    }

    pub(crate) fn backspace(&mut self) {
        self.error = None;
        if let PinColorEditorDraft::Custom(value) = &mut self.draft {
            value.pop();
            if value.is_empty() {
                self.draft = PinColorEditorDraft::Empty;
            }
        }
    }

    /// Ctrl+U — clear the in-progress draft back to empty in one step,
    /// distinct from `r` (reset), which commits `None` and closes the
    /// editor. This only clears the typed/selected draft so the user
    /// keeps working in the same dialog.
    pub(crate) fn clear(&mut self) {
        self.error = None;
        self.draft = PinColorEditorDraft::Empty;
    }

    pub(crate) fn select_palette_digit(&mut self, digit: char) -> bool {
        let Some(color) = pin_color_palette_for_digit(digit) else {
            return false;
        };
        self.error = None;
        self.draft = PinColorEditorDraft::Palette(color);
        true
    }

    pub(crate) fn color_to_apply(&self) -> Result<Option<PinColorHex>, PinColorError> {
        match &self.draft {
            PinColorEditorDraft::Empty => Ok(None),
            PinColorEditorDraft::Existing(color) | PinColorEditorDraft::Palette(color) => {
                Ok(Some(color.clone()))
            }
            PinColorEditorDraft::Custom(value) => PinColorHex::parse(value).map(Some),
        }
    }

    pub(crate) fn draft_label(&self) -> String {
        match &self.draft {
            PinColorEditorDraft::Empty => "(none)".to_string(),
            PinColorEditorDraft::Existing(color) | PinColorEditorDraft::Palette(color) => {
                color.to_string()
            }
            PinColorEditorDraft::Custom(value) => value.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PinColorEditorDraft {
    Empty,
    Existing(PinColorHex),
    Palette(PinColorHex),
    Custom(String),
}
