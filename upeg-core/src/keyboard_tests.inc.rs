    use super::*;

    fn stroke(key: Key) -> KeyStroke {
        KeyStroke::plain(key)
    }

    #[test]
    fn 보드_scope는_공통_탐색과_명령을_해석한다() {
        let ctx = KeyboardContext::new(KeyboardScope::Board);

        assert_eq!(
            resolve_key(ctx, stroke(Key::Left)),
            Some(KeyboardCommand::Move(NavDirection::Left))
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Char('h'))),
            Some(KeyboardCommand::Move(NavDirection::Left))
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Enter)),
            Some(KeyboardCommand::Run)
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Space)),
            Some(KeyboardCommand::Run)
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Char('/'))),
            Some(KeyboardCommand::Search)
        );
    }

    #[test]
    fn 보드_scope의_엔터는_상세가_아니라_실행이다() {
        // Enter는 표면 전체에서 "실행/확정"이라는 하나의 계약을 지킨다.
        // 이전에는 List의 Enter만 예외적으로 상세를 열었는데, 그 예외가
        // Result/Form/BoardEditor의 Enter=확정 관례와 충돌했다.
        let ctx = KeyboardContext::new(KeyboardScope::Board);
        assert_eq!(
            resolve_key(ctx, stroke(Key::Enter)),
            Some(KeyboardCommand::Run),
            "Enter는 Open이 아니라 Run으로 해석되어야 한다"
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::F(1))),
            Some(KeyboardCommand::Run),
            "F1도 동일하게 Run이어야 한다"
        );
    }

    #[test]
    fn 보드_scope의_오는_상세_보기를_연다() {
        let ctx = KeyboardContext::new(KeyboardScope::Board);
        assert_eq!(
            resolve_key(ctx, stroke(Key::Char('o'))),
            Some(KeyboardCommand::Open)
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Char('O'))),
            Some(KeyboardCommand::Open)
        );
    }

    #[test]
    fn 보드와_상세_scope의_f2는_복사_명령이다() {
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);
        let detail_ctx = KeyboardContext::new(KeyboardScope::Detail);
        assert_eq!(
            resolve_key(board_ctx, stroke(Key::F(2))),
            Some(KeyboardCommand::Copy)
        );
        assert_eq!(
            resolve_key(detail_ctx, stroke(Key::F(2))),
            Some(KeyboardCommand::Copy)
        );
    }

    #[test]
    fn f1은_boardeditor_pincolor_toolpicker_moving에서_커밋이고_confirmdelete에서_확인이다() {
        // PinColorEditor는 keyboard_scope_for_view에서 BoardEditor와 같은
        // 스코프를 공유하므로 별도 케이스가 필요 없다.
        let cases = [
            (KeyboardScope::BoardEditor, KeyboardCommand::Commit),
            (KeyboardScope::ToolPicker, KeyboardCommand::Commit),
            (KeyboardScope::Moving, KeyboardCommand::Commit),
        ];
        for (scope, expected) in cases {
            assert_eq!(
                resolve_key(KeyboardContext::new(scope), stroke(Key::F(1))),
                Some(expected),
                "{scope:?}에서 F1은 {expected:?}여야 한다"
            );
        }
        assert_eq!(
            resolve_key(
                KeyboardContext::new(KeyboardScope::ConfirmDelete),
                stroke(Key::F(1))
            ),
            Some(KeyboardCommand::Confirm)
        );
    }

    #[test]
    fn ctrl_u는_form_boardeditor_toolpicker에서_입력_지우기_명령이다() {
        let ctrl_u = KeyStroke::modified(
            Key::Char('u'),
            KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        );
        for scope in [
            KeyboardScope::Form,
            KeyboardScope::BoardEditor,
            KeyboardScope::ToolPicker,
        ] {
            assert_eq!(
                resolve_key(KeyboardContext::new(scope), ctrl_u),
                Some(KeyboardCommand::ClearInput),
                "{scope:?}에서 Ctrl+U는 ClearInput이어야 한다"
            );
        }
    }

    #[test]
    fn 보드_변경_명령은_모드리스로_항상_활성이다() {
        // 편집 모드가 사라진 뒤(modeless) `n`/`R`/`D`/`a`는 board 스코프에서
        // 언제나 해석된다. 포커스는 필요 없다 — 보드 레벨 작업이기 때문.
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);

        assert_eq!(
            resolve_key(board_ctx, stroke(Key::Char('n'))),
            Some(KeyboardCommand::NewBoard)
        );
        assert_eq!(
            resolve_key(board_ctx, stroke(Key::Char('R'))),
            Some(KeyboardCommand::RenameBoard)
        );
        assert_eq!(
            resolve_key(board_ctx, stroke(Key::Char('D'))),
            Some(KeyboardCommand::DeleteBoard)
        );
        assert_eq!(
            resolve_key(board_ctx, stroke(Key::Char('a'))),
            Some(KeyboardCommand::OpenToolPicker)
        );
    }

    #[test]
    fn 이는_포커스된_핀에서_리사이즈_시작이다() {
        // 옛 편집 토글이 비운 `e`를 리사이즈 시작으로 재바인딩했다.
        // `m`/StartMove와 대칭으로, 포커스된 핀이 있어야만 해석된다.
        let focused_pin_ctx = KeyboardContext {
            has_tool_focus: true,
            ..KeyboardContext::new(KeyboardScope::Board)
        };
        assert_eq!(
            resolve_key(focused_pin_ctx, stroke(Key::Char('e'))),
            Some(KeyboardCommand::StartResize)
        );
        assert_eq!(
            resolve_key(focused_pin_ctx, stroke(Key::Char('E'))),
            Some(KeyboardCommand::StartResize)
        );
    }

    #[test]
    fn 포커스_없는_보드와_상세_scope의_이는_해석되지_않는다() {
        // 포커스된 핀이 없으면 무엇을 리사이즈할지 알 수 없으므로 `e`는
        // 무시된다. Detail scope에서는 여전히 자유키다.
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);
        let detail_ctx = KeyboardContext::new(KeyboardScope::Detail);
        assert_eq!(resolve_key(board_ctx, stroke(Key::Char('e'))), None);
        assert_eq!(resolve_key(board_ctx, stroke(Key::Char('E'))), None);
        assert_eq!(resolve_key(detail_ctx, stroke(Key::Char('e'))), None);
    }

    #[test]
    fn 리사이즈_scope는_방향키를_span_증감으로_해석한다() {
        let ctx = KeyboardContext::new(KeyboardScope::Resize);
        let grow_cases = [
            (Key::Right, 1, 0),
            (Key::Char('l'), 1, 0),
            (Key::Left, -1, 0),
            (Key::Char('h'), -1, 0),
            (Key::Down, 0, 1),
            (Key::Char('j'), 0, 1),
            (Key::Up, 0, -1),
            (Key::Char('k'), 0, -1),
        ];
        for (key, cols, rows) in grow_cases {
            assert_eq!(
                resolve_key(ctx, stroke(key)),
                Some(KeyboardCommand::ResizeBy { cols, rows }),
                "{key:?}는 ResizeBy(cols {cols}, rows {rows})여야 한다"
            );
        }
    }

    #[test]
    fn 리사이즈_scope는_커밋_취소_리셋을_moving과_같은_키로_해석한다() {
        let ctx = KeyboardContext::new(KeyboardScope::Resize);
        assert_eq!(
            resolve_key(ctx, stroke(Key::Enter)),
            Some(KeyboardCommand::Commit)
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::F(1))),
            Some(KeyboardCommand::Commit)
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Esc)),
            Some(KeyboardCommand::Cancel)
        );
        assert_eq!(
            resolve_key(ctx, stroke(Key::Char('q'))),
            Some(KeyboardCommand::Cancel)
        );
        // `0`은 board scope의 filter-clear 관용을 따라 manifest 기본
        // 크기로 되돌린다.
        assert_eq!(
            resolve_key(ctx, stroke(Key::Char('0'))),
            Some(KeyboardCommand::ResetSpan)
        );
    }

    #[test]
    fn 포커스된_핀은_c로_색상_편집을_연다() {
        let focused_pin_ctx = KeyboardContext {
            has_tool_focus: true,
            ..KeyboardContext::new(KeyboardScope::Board)
        };

        assert_eq!(
            resolve_key(focused_pin_ctx, stroke(Key::Char('c'))),
            Some(KeyboardCommand::EditPinColor)
        );
    }

    #[test]
    fn 포커스_없는_보드의_c는_핀_색상_편집을_열지_않는다() {
        // 포커스된 핀이 없으면 어떤 핀을 대상으로 할지 알 수 없으므로 `c`는
        // 무시된다 — 이 게이트는 modeless 이후에도 유일하게 남은 조건이다.
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);
        assert_eq!(resolve_key(board_ctx, stroke(Key::Char('c'))), None);
    }

    #[test]
    fn 보드_scope의_슬래시는_검색을_유지한다() {
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);

        assert_eq!(
            resolve_key(board_ctx, stroke(Key::Char('/'))),
            Some(KeyboardCommand::Search)
        );
    }

    #[test]
    fn 보드_scope의_primary_k는_검색을_유지한다() {
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);
        let ctrl = KeyModifiers {
            control: true,
            ..KeyModifiers::NONE
        };
        let meta = KeyModifiers {
            meta: true,
            ..KeyModifiers::NONE
        };

        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('k'), ctrl)),
            Some(KeyboardCommand::Search)
        );
        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('K'), meta)),
            Some(KeyboardCommand::Search)
        );
    }

    #[test]
    fn primary_w는_scope의_esc_동작을_따라간다() {
        // Ctrl+W(윈도/리눅스)와 Cmd+W(맥)는 데스크톱 관례상 "현재 표면 닫기"다.
        // 각 scope가 Esc에 바인딩한 dismissal을 그대로 위임한다 —
        // Board/Form은 Close, editor/confirm/picker/Moving은 Cancel.
        let ctrl = KeyModifiers {
            control: true,
            ..KeyModifiers::NONE
        };
        let meta = KeyModifiers {
            meta: true,
            ..KeyModifiers::NONE
        };

        let close_scopes = [
            KeyboardScope::Board,
            KeyboardScope::Detail,
            KeyboardScope::Form,
            KeyboardScope::Settings,
        ];
        for scope in close_scopes {
            let ctx = KeyboardContext::new(scope);
            assert_eq!(
                resolve_key(ctx, KeyStroke::modified(Key::Char('w'), ctrl)),
                Some(KeyboardCommand::Close),
                "{scope:?} scope의 Ctrl+W는 Close여야 한다"
            );
            assert_eq!(
                resolve_key(ctx, KeyStroke::modified(Key::Char('W'), meta)),
                Some(KeyboardCommand::Close),
                "{scope:?} scope의 Cmd+W는 Close여야 한다"
            );
        }

        let cancel_scopes = [
            KeyboardScope::Moving,
            KeyboardScope::Resize,
            KeyboardScope::BoardEditor,
            KeyboardScope::ConfirmDelete,
            KeyboardScope::ToolPicker,
        ];
        for scope in cancel_scopes {
            let ctx = KeyboardContext::new(scope);
            assert_eq!(
                resolve_key(ctx, KeyStroke::modified(Key::Char('w'), ctrl)),
                Some(KeyboardCommand::Cancel),
                "{scope:?} scope의 Ctrl+W는 Cancel이어야 한다"
            );
        }
    }

    #[test]
    fn 보드_scope의_primary_q는_종료를_유지한다() {
        // Ctrl+Q(윈도/리눅스)와 Cmd+Q(맥) 모두 데스크톱 관례상 종료다.
        // 맨 `q`뿐 아니라 primary modifier 조합에서도 Quit이어야 한다.
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);
        let ctrl = KeyModifiers {
            control: true,
            ..KeyModifiers::NONE
        };
        let meta = KeyModifiers {
            meta: true,
            ..KeyModifiers::NONE
        };

        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('q'), ctrl)),
            Some(KeyboardCommand::Quit)
        );
        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('Q'), meta)),
            Some(KeyboardCommand::Quit)
        );
    }

    #[test]
    fn 공유_키보드_contract는_주요_scope의_명령을_고정한다() {
        let board_with_focus = KeyboardContext {
            has_tool_focus: true,
            ..KeyboardContext::new(KeyboardScope::Board)
        };
        let cases = [
            (
                KeyboardContext::new(KeyboardScope::Board),
                KeyStroke::modified(
                    Key::Char('k'),
                    KeyModifiers {
                        control: true,
                        ..KeyModifiers::NONE
                    },
                ),
                Some(KeyboardCommand::Search),
            ),
            (
                KeyboardContext::new(KeyboardScope::Board),
                stroke(Key::Char('q')),
                Some(KeyboardCommand::Quit),
            ),
            (
                KeyboardContext::new(KeyboardScope::Board),
                stroke(Key::Char('b')),
                Some(KeyboardCommand::CycleBoardFilter),
            ),
            (
                KeyboardContext::new(KeyboardScope::Board),
                stroke(Key::Char('t')),
                Some(KeyboardCommand::CycleTagFilter),
            ),
            (
                KeyboardContext::new(KeyboardScope::Board),
                stroke(Key::Char('9')),
                BoardSlot::new(9).map(KeyboardCommand::SwitchBoard),
            ),
            (
                board_with_focus,
                stroke(Key::Char('a')),
                Some(KeyboardCommand::OpenToolPicker),
            ),
            (
                board_with_focus,
                stroke(Key::Char('p')),
                Some(KeyboardCommand::TogglePin),
            ),
            (
                board_with_focus,
                stroke(Key::Char('c')),
                Some(KeyboardCommand::EditPinColor),
            ),
            (
                board_with_focus,
                stroke(Key::Char('m')),
                Some(KeyboardCommand::StartMove),
            ),
            (
                board_with_focus,
                stroke(Key::Char('e')),
                Some(KeyboardCommand::StartResize),
            ),
            (
                board_with_focus,
                stroke(Key::Char(']')),
                Some(KeyboardCommand::Reorder(OrderDirection::Next)),
            ),
            (
                KeyboardContext::new(KeyboardScope::Detail),
                stroke(Key::Char('r')),
                Some(KeyboardCommand::Run),
            ),
            (
                KeyboardContext::new(KeyboardScope::Detail),
                stroke(Key::Char('q')),
                Some(KeyboardCommand::Close),
            ),
            (
                KeyboardContext::new(KeyboardScope::Form),
                stroke(Key::Char('b')),
                Some(KeyboardCommand::Text('b')),
            ),
            (
                KeyboardContext::new(KeyboardScope::Form),
                stroke(Key::BackTab),
                Some(KeyboardCommand::FocusPrevious),
            ),
            (
                KeyboardContext::new(KeyboardScope::Settings),
                stroke(Key::Char('l')),
                Some(KeyboardCommand::Move(NavDirection::Right)),
            ),
            (
                KeyboardContext::new(KeyboardScope::BoardEditor),
                stroke(Key::Enter),
                Some(KeyboardCommand::Commit),
            ),
            (
                KeyboardContext::new(KeyboardScope::ConfirmDelete),
                stroke(Key::Char('y')),
                Some(KeyboardCommand::Confirm),
            ),
            (
                KeyboardContext::new(KeyboardScope::ConfirmDelete),
                stroke(Key::Char('Q')),
                Some(KeyboardCommand::Cancel),
            ),
            (
                KeyboardContext::new(KeyboardScope::ToolPicker),
                stroke(Key::Backspace),
                Some(KeyboardCommand::Backspace),
            ),
            (
                KeyboardContext::new(KeyboardScope::FilterBar),
                stroke(Key::End),
                Some(KeyboardCommand::End),
            ),
            (
                KeyboardContext::new(KeyboardScope::RightPane),
                stroke(Key::PageDown),
                Some(KeyboardCommand::Page(PageDirection::Down)),
            ),
            (
                KeyboardContext::new(KeyboardScope::Moving),
                stroke(Key::Char('h')),
                Some(KeyboardCommand::Move(NavDirection::Left)),
            ),
            (
                KeyboardContext::new(KeyboardScope::Moving),
                stroke(Key::Esc),
                Some(KeyboardCommand::Cancel),
            ),
            (
                KeyboardContext::new(KeyboardScope::Resize),
                stroke(Key::Char('l')),
                Some(KeyboardCommand::ResizeBy { cols: 1, rows: 0 }),
            ),
            (
                KeyboardContext::new(KeyboardScope::Resize),
                stroke(Key::Char('0')),
                Some(KeyboardCommand::ResetSpan),
            ),
        ];

        for (context, stroke, expected) in cases {
            assert_eq!(
                resolve_key(context, stroke),
                expected,
                "{context:?} + {stroke:?} must keep the shared key binding contract"
            );
        }
    }

    #[test]
    fn command_modifier는_명시_shortcut이_아니면_일반_binding을_막는다() {
        let board_ctx = KeyboardContext::new(KeyboardScope::Board);
        let focus_ctx = KeyboardContext {
            has_tool_focus: true,
            ..KeyboardContext::new(KeyboardScope::Board)
        };
        let ctrl = KeyModifiers {
            control: true,
            ..KeyModifiers::NONE
        };
        let alt = KeyModifiers {
            alt: true,
            ..KeyModifiers::NONE
        };
        let meta = KeyModifiers {
            meta: true,
            ..KeyModifiers::NONE
        };
        let ctrl_alt = KeyModifiers {
            control: true,
            alt: true,
            ..KeyModifiers::NONE
        };
        let shift = KeyModifiers {
            shift: true,
            ..KeyModifiers::NONE
        };

        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('k'), ctrl)),
            Some(KeyboardCommand::Search),
            "Ctrl/Cmd+K is the only global command-modified board shortcut"
        );
        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('k'), ctrl_alt)),
            None,
            "Alt must not combine with the Ctrl/Cmd+K primary shortcut"
        );
        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('e'), ctrl)),
            None,
            "Ctrl+E is unbound — bare `e` starts resize, but no chord does"
        );
        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('b'), alt)),
            None,
            "Alt+B must not cycle board filters"
        );
        assert_eq!(
            resolve_key(board_ctx, KeyStroke::modified(Key::Char('1'), meta)),
            None,
            "Meta+1 must not switch boards"
        );
        assert_eq!(
            resolve_key(
                KeyboardContext::new(KeyboardScope::Form),
                KeyStroke::modified(Key::Char('b'), ctrl),
            ),
            None,
            "Ctrl+B in form scope is a command chord, not text input"
        );
        assert_eq!(
            resolve_key(
                KeyboardContext::new(KeyboardScope::Moving),
                KeyStroke::modified(Key::Left, alt),
            ),
            None,
            "Alt+arrow must not move a board item"
        );
        assert_eq!(
            resolve_key(focus_ctx, KeyStroke::modified(Key::Char('R'), shift)),
            Some(KeyboardCommand::RenameBoard),
            "Shift only produces case/symbol keys and remains eligible"
        );
    }

    #[test]
    fn 보드_slot은_일부터_구까지만_허용한다() {
        assert_eq!(BoardSlot::new(0), None);
        assert_eq!(BoardSlot::new(1).map(BoardSlot::zero_based_index), Some(0));
        assert_eq!(BoardSlot::new(9).map(BoardSlot::zero_based_index), Some(8));
        assert_eq!(BoardSlot::new(10), None);
    }

    #[test]
    fn key_label_parser는_surface_adapter들의_공통_키_문자열을_해석한다() {
        assert_eq!(key_from_label("ArrowLeft", false), Some(Key::Left));
        assert_eq!(key_from_label(" ", false), Some(Key::Space));
        assert_eq!(key_from_label("Space", false), Some(Key::Space));
        assert_eq!(key_from_label("F1", false), Some(Key::F(1)));
        assert_eq!(key_from_label("F24", false), Some(Key::F(24)));
        assert_eq!(key_from_label("Tab", true), Some(Key::BackTab));
        assert_eq!(key_from_label("Shift", false), None);
    }
