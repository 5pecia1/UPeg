/// Unit tests for the typed `TrayCommand` enum that replaces the
/// stringly-typed menu key threading in `platform/tray.dart`. The
/// production switch consumes `TrayCommand.fromKey(item.key)` so the
/// menu key strings live in exactly one place — the enum's
/// [TrayCommand.menuItemKey] field.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:tray_manager/tray_manager.dart' show MenuItem;

import 'package:upeg/src/platform/tray.dart';

void main() {
  group('TrayCommand.fromKey', () {
    test('TrayCommand_fromKey는_미지의_키에_대해_null을_반환한다', () {
      expect(TrayCommand.fromKey('unknown'), isNull);
    });

    test('TrayCommand_fromKey는_toggle_pause_키를_매핑한다', () {
      expect(TrayCommand.fromKey('toggle_pause'), TrayCommand.togglePause);
    });

    test('삭제된_service_제어_키는_더이상_매핑되지_않는다', () {
      // service/source control plane 삭제: tray에 남은 host 제어는
      // pause 하나뿐이다.
      expect(TrayCommand.fromKey('toggle_rest_api'), isNull);
      expect(TrayCommand.fromKey('load_mcp_imports'), isNull);
      expect(TrayCommand.fromKey('unload_mcp_imports'), isNull);
    });
  });

  group('buildTrayMenu', () {
    test('Tray_menu는_모든_command_key를_한_번씩_포함한다', () {
      final menu = buildTrayMenu();
      final items = menu.items ?? const <MenuItem>[];
      final keys = [
        for (final item in items)
          if (item.type != 'separator') item.key,
      ];

      expect(keys, [
        TrayCommand.openDashboard.menuItemKey,
        TrayCommand.togglePopup.menuItemKey,
        TrayCommand.togglePin.menuItemKey,
        TrayCommand.togglePause.menuItemKey,
        TrayCommand.quit.menuItemKey,
      ]);
    });

    test('Tray_menu는_그룹_사이에_separator를_렌더한다', () {
      final items = buildTrayMenu().items ?? const <MenuItem>[];
      final separatorIndexes = <int>[
        for (var i = 0; i < items.length; i++)
          if (items[i].type == 'separator') i,
      ];

      expect(separatorIndexes, [3, 5]);
      for (final index in separatorIndexes) {
        expect(items[index].disabled, isTrue);
      }
    });

    test('Tray_menu는_사용자에게_보이는_label을_고정한다', () {
      final items = buildTrayMenu().items ?? const <MenuItem>[];
      final labelsByKey = {
        for (final item in items)
          if (item.type != 'separator') item.key: item.label,
      };

      expect(labelsByKey, {
        TrayCommand.openDashboard.menuItemKey: 'Open dashboard',
        TrayCommand.togglePopup.menuItemKey: 'Toggle popup / full',
        TrayCommand.togglePin.menuItemKey: 'Toggle pin (keep popup open)',
        TrayCommand.togglePause.menuItemKey: 'Pause / Resume host',
        TrayCommand.quit.menuItemKey: 'Quit',
      });
    });

    test('buildTrayMenu는_pauseControllable_false시_pause를_비활성한다', () {
      // Pause is a process-local `AtomicBool` (WS4 PRD §5.9) — it
      // cannot reach a foreign daemon, so the menu item must grey out
      // whenever this process isn't the in-process (Embedded) host.
      final disabledMenu = buildTrayMenu(pauseControllable: false);
      final disabledItem = (disabledMenu.items ?? const <MenuItem>[])
          .firstWhere(
            (item) => item.key == TrayCommand.togglePause.menuItemKey,
          );
      expect(disabledItem.disabled, isTrue);

      final enabledMenu = buildTrayMenu(pauseControllable: true);
      final enabledItem = (enabledMenu.items ?? const <MenuItem>[]).firstWhere(
        (item) => item.key == TrayCommand.togglePause.menuItemKey,
      );
      expect(enabledItem.disabled, isFalse);

      // Default stays enabled so existing call sites that don't pass
      // the parameter keep prior behavior until they're updated.
      final defaultItem = (buildTrayMenu().items ?? const <MenuItem>[])
          .firstWhere(
            (item) => item.key == TrayCommand.togglePause.menuItemKey,
          );
      expect(defaultItem.disabled, isFalse);
    });
  });
}
