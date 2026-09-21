use super::*;

#[test]
fn f1_button_click_runs_selected_no_arg_tool() {
    let mut s = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 100, 30);
    let layout = tui_layout(area);

    let action = handle_mouse(
        &mut s,
        Mouse {
            column: layout.right.x + 3,
            row: layout.right.y + layout.right.height - 2,
            kind: MouseKind::Pointer(PointerPhase::Down),
        },
        t,
        area,
    );
    match action {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, "test.simple");
            assert_eq!(args, json!({}));
        }
        other => panic!("expected Dispatch on F1 mouse click but got {other:?}"),
    }
}

#[test]
fn esc_in_detail_returns_to_list_without_quitting() {
    let mut s = State {
        cursor: 1,
        view: View::Detail,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Esc, t);
    assert_eq!(action, Action::None);
    assert_eq!(s.view, View::List);
}

#[test]
fn enter_on_no_arg_tool_runs_immediately() {
    // simple has empty args, so Enter in Detail runs it with {}.
    let mut s = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    let action = handle_key(&mut s, Key::Enter, t);
    match action {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, "test.simple");
            assert_eq!(args, json!({}));
        }
        other => panic!("expected Dispatch but got {other:?}"),
    }
}

#[test]
fn run_key_opens_form_for_tool_requiring_input() {
    let mut s = State {
        cursor: 1,
        view: View::Detail,
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Char('r'), t);
    match &s.view {
        View::Form { tool_id, form } => {
            assert_eq!(*tool_id, "test.with_input");
            assert_eq!(form.len(), 1);
            assert_eq!(form.fields[0].name.as_str(), "input");
            let input = form.spec_for_field(&form.fields[0]).unwrap();
            assert!(input.required);
            assert_eq!(form.focused, 0);
        }
        other => panic!("expected the Form view but got {other:?}"),
    }
}

#[test]
fn typing_in_form_appends_to_focused_field() {
    let mut s = State {
        cursor: 1,
        view: View::Form {
            tool_id: "test.with_input",
            form: string_form("input", "", true),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    for c in "0xff".chars() {
        handle_key(&mut s, Key::Char(c), t);
    }
    if let View::Form { form, .. } = &s.view {
        assert_eq!(form.fields[0].draft, DraftInputValue::Text("0xff".into()));
    } else {
        panic!("expected the Form view");
    }
}

#[test]
fn backspace_in_form_removes_last_char() {
    let mut s = State {
        cursor: 0,
        view: View::Form {
            tool_id: "x",
            form: string_form("input", "abc", false),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Backspace, t);
    if let View::Form { form, .. } = &s.view {
        assert_eq!(form.fields[0].draft, DraftInputValue::Text("ab".into()));
    } else {
        panic!("expected the Form view");
    }
}

#[test]
fn enter_in_form_triggers_run() {
    let mut s = State {
        cursor: 1,
        view: View::Form {
            tool_id: "test.with_input",
            form: string_form("input", "0xff", true),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    match handle_key(&mut s, Key::Enter, t) {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, "test.with_input");
            assert_eq!(args, json!({"input": "0xff"}));
        }
        other => panic!("expected Dispatch but got {other:?}"),
    }
}

#[test]
fn esc_in_form_returns_to_list() {
    let mut s = State {
        cursor: 1,
        view: View::Form {
            tool_id: "x",
            form: TuiFormState::new(InputSpec::empty()),
        },
        ..State::default()
    };
    let t = fixture_tools();
    let t = t.as_slice();
    handle_key(&mut s, Key::Esc, t);
    assert_eq!(s.view, View::List);
}

// ─── Phase E-1: View::Settings entry / navigation ────────────────

#[test]
fn settings_key_in_list_opens_settings_focused_on_locale() {
    use crate::surfaces::tui::model::SettingsField;
    let mut s = fresh();
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('s'), t.as_slice());
    assert_eq!(
        s.view,
        View::Settings {
            focused_field: SettingsField::Locale,
        }
    );
}

#[test]
fn tui_prefs_save_uses_settings_root_not_toolkits_dir() {
    const EFFECTS_SRC: &str = include_str!("../effects.rs");
    assert!(
        EFFECTS_SRC.contains("upeg_core::paths::tweaks_path()"),
        "TUI prefs must be stored under the settings root (`~/.upeg` on Unix), not toolkits_dir"
    );
    assert!(
        !EFFECTS_SRC.contains("paths::toolkits_dir()"),
        "TUI prefs must not follow UPEG_TOOLKITS_DIR; that env var is for tool source loading"
    );
}

#[test]
fn settings_key_in_detail_opens_settings() {
    use crate::surfaces::tui::model::SettingsField;
    let mut s = State {
        view: View::Detail,
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Char('s'), t.as_slice());
    assert_eq!(
        s.view,
        View::Settings {
            focused_field: SettingsField::Locale,
        }
    );
}

#[test]
fn esc_in_settings_returns_to_list() {
    use crate::surfaces::tui::model::SettingsField;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Theme,
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Esc, t.as_slice());
    assert_eq!(s.view, View::List);
}

#[test]
fn down_key_in_settings_cycles_focused_field() {
    use crate::surfaces::tui::model::SettingsField;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Down, t.as_slice());
    assert_eq!(
        s.view,
        View::Settings {
            focused_field: SettingsField::Theme,
        }
    );
    handle_key(&mut s, Key::Down, t.as_slice());
    assert_eq!(
        s.view,
        View::Settings {
            focused_field: SettingsField::Accent,
        }
    );
    // Cycles back to Locale.
    handle_key(&mut s, Key::Down, t.as_slice());
    assert_eq!(
        s.view,
        View::Settings {
            focused_field: SettingsField::Locale,
        }
    );
}

#[test]
fn up_key_in_settings_cycles_backward() {
    use crate::surfaces::tui::model::SettingsField;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Up, t.as_slice());
    assert_eq!(
        s.view,
        View::Settings {
            focused_field: SettingsField::Accent,
        }
    );
}

#[test]
fn q_in_settings_returns_to_list_without_quitting() {
    // `q` in the List view quits the TUI, but `q` in the Settings view
    // must go back one step. That way a user experimenting with the
    // accent toggle does not accidentally quit the binary.
    use crate::surfaces::tui::model::SettingsField;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Char('q'), t.as_slice());
    assert!(matches!(effect, Action::None));
    assert_eq!(s.view, View::List);
}

// ─── Phase E-2: ←/→ value cycling on the focused field ──────────

#[test]
fn right_arrow_cycles_focused_locale_value() {
    use crate::surfaces::tui::model::SettingsField;
    use upeg_core::prefs::Locale;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        ..fresh()
    };
    assert_eq!(s.tweaks.locale, Locale::En);
    let t = fixture_tools();
    let effect = handle_key(&mut s, Key::Right, t.as_slice());
    assert_eq!(s.tweaks.locale, Locale::Ko);
    // Phase E-3: every value-cycle key returns SaveTweaks so the event
    // loop persists the change to disk.
    assert!(
        matches!(effect, Action::SaveTweaks),
        "value cycling must emit Effect::SaveTweaks"
    );
    // Cycles back to English.
    handle_key(&mut s, Key::Right, t.as_slice());
    assert_eq!(s.tweaks.locale, Locale::En);
}

#[test]
fn left_arrow_cycles_locale_backward() {
    use crate::surfaces::tui::model::SettingsField;
    use upeg_core::prefs::Locale;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Left, t.as_slice());
    assert_eq!(s.tweaks.locale, Locale::Ko);
}

#[test]
fn right_arrow_cycles_theme_when_theme_focused() {
    use crate::surfaces::tui::model::SettingsField;
    use upeg_core::prefs::Theme;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Theme,
        },
        ..fresh()
    };
    assert_eq!(s.tweaks.theme, Theme::Light);
    let t = fixture_tools();
    handle_key(&mut s, Key::Right, t.as_slice());
    assert_eq!(s.tweaks.theme, Theme::Dark);
    handle_key(&mut s, Key::Right, t.as_slice());
    assert_eq!(
        s.tweaks.theme,
        Theme::Light,
        "a two-value field must wrap around"
    );
}

#[test]
fn right_arrow_cycles_four_accent_values() {
    use crate::surfaces::tui::model::SettingsField;
    use upeg_core::prefs::Accent;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Accent,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let expected = [Accent::Amber, Accent::Cyan, Accent::Pink, Accent::Green];
    for want in expected {
        handle_key(&mut s, Key::Right, t.as_slice());
        assert_eq!(s.tweaks.accent, want);
    }
}

#[test]
fn left_arrow_cycles_accent_backward() {
    use crate::surfaces::tui::model::SettingsField;
    use upeg_core::prefs::Accent;
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Accent,
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Left, t.as_slice());
    assert_eq!(s.tweaks.accent, Accent::Pink);
}

#[test]
fn settings_view_renders_on_full_body_panel_not_just_right_pane() {
    // Visibility fix pinning: before the fix, Settings rendered via
    // `right_pane_content` and ended up in the narrow right pane beside
    // the still-active pegboard grid body, so a user who pressed `s`
    // could not find the screen. Since phase 6 the BoardEditor /
    // ConfirmDeleteBoard / ToolPicker variants share the same
    // full-body branch, so the source pin verifies the Settings variant
    // is matched inside the unified dispatcher.
    const SRC: &str = include_str!("../view.rs");
    assert!(
        SRC.contains("View::Settings { focused_field }"),
        "render_with_context must route View::Settings to the full-body render \
         path; otherwise the screen stays invisible inside the side panel"
    );
    assert!(
        SRC.contains("fn render_settings_panel"),
        "the Settings-specific renderer render_settings_panel must exist"
    );
    // Full-body occupancy: at dispatch entry render_with_context builds
    // the left+right combined rect via `let body = layout.body();`, and
    // every dialog-variant arm calls its dedicated renderer with that
    // `body`. Pinned as the inline-argument form.
    assert!(
        SRC.contains("let body = layout.body();"),
        "render_with_context must bind layout.body() (the full body) at body entry"
    );
    assert!(
        SRC.contains("render_settings_panel(frame, body,"),
        "the Settings renderer must be called with body (the full body)"
    );
}

#[test]
fn value_cycling_targets_only_the_focused_field() {
    // While Locale is focused, pressing ← / → must not change Theme or
    // Accent; otherwise a confused user ends up changing every value at
    // once.
    use crate::surfaces::tui::model::SettingsField;
    use upeg_core::prefs::{Accent, Locale, Theme};
    let mut s = State {
        view: View::Settings {
            focused_field: SettingsField::Locale,
        },
        ..fresh()
    };
    let t = fixture_tools();
    handle_key(&mut s, Key::Right, t.as_slice());
    assert_eq!(s.tweaks.locale, Locale::Ko);
    assert_eq!(s.tweaks.theme, Theme::Light, "Theme must not move");
    assert_eq!(s.tweaks.accent, Accent::Green, "Accent must not move");
}
