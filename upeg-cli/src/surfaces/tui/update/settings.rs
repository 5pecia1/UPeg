use super::super::effects::Effect;
use super::super::model::{SettingsField, State, View};
use super::super::scroll::ScrollOffset;

pub(super) fn open_settings(state: &mut State) -> Effect {
    state.right_scroll = ScrollOffset::ZERO;
    state.view = View::Settings {
        focused_field: SettingsField::default(),
    };
    Effect::None
}

pub(super) fn cycle_focused_value(state: &mut State, forward: bool) {
    let View::Settings { focused_field } = &state.view else {
        return;
    };
    match focused_field {
        SettingsField::Locale => state.tweaks.locale = cycle_locale(state.tweaks.locale, forward),
        SettingsField::Theme => state.tweaks.theme = cycle_theme(state.tweaks.theme, forward),
        SettingsField::Accent => state.tweaks.accent = cycle_accent(state.tweaks.accent, forward),
    }
}

const fn cycle_locale(
    locale: upeg_core::prefs::Locale,
    _forward: bool,
) -> upeg_core::prefs::Locale {
    use upeg_core::prefs::Locale;
    match locale {
        Locale::En => Locale::Ko,
        Locale::Ko => Locale::En,
    }
}

const fn cycle_theme(theme: upeg_core::prefs::Theme, _forward: bool) -> upeg_core::prefs::Theme {
    use upeg_core::prefs::Theme;
    match theme {
        Theme::Dark => Theme::Light,
        Theme::Light => Theme::Dark,
    }
}

const fn cycle_accent(accent: upeg_core::prefs::Accent, forward: bool) -> upeg_core::prefs::Accent {
    use upeg_core::prefs::Accent;
    match (accent, forward) {
        (Accent::Green, true) => Accent::Amber,
        (Accent::Amber, true) => Accent::Cyan,
        (Accent::Cyan, true) => Accent::Pink,
        (Accent::Pink, true) => Accent::Green,
        (Accent::Green, false) => Accent::Pink,
        (Accent::Amber, false) => Accent::Green,
        (Accent::Cyan, false) => Accent::Amber,
        (Accent::Pink, false) => Accent::Cyan,
    }
}
