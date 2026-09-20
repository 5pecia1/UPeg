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
    test('TrayCommand_fromKey_returns_null_for_an_unknown_key', () {
      expect(TrayCommand.fromKey('unknown'), isNull);
    });

    test('TrayCommand_fromKey_maps_the_toggle_pause_key', () {
      expect(TrayCommand.fromKey('toggle_pause'), TrayCommand.togglePause);
    });

    test('deleted_service_control_keys_are_no_longer_mapped', () {
      // The service/source control plane was deleted: pause is the
      // only host control left on the tray.
      expect(TrayCommand.fromKey('toggle_rest_api'), isNull);
      expect(TrayCommand.fromKey('load_mcp_imports'), isNull);
      expect(TrayCommand.fromKey('unload_mcp_imports'), isNull);
    });
  });

  group('buildTrayMenu', () {
    test('the_tray_menu_contains_every_command_key_exactly_once', () {
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

    test('the_tray_menu_renders_separators_between_groups', () {
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

    test('the_tray_menu_pins_the_user_visible_labels', () {
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

    test('buildTrayMenu_disables_pause_when_pauseControllable_is_false', () {
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
