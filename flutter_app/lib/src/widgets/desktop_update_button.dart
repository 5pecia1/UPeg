import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/platform/app_lifecycle.dart';
import 'package:upeg/src/platform/desktop_update.dart';
import 'package:upeg/src/state/desktop_update_provider.dart';
import 'package:upeg/src/widgets/notification_results_dialog.dart'
    show notificationResultsNavigatorKey;

class DesktopUpdateButton extends ConsumerWidget {
  const DesktopUpdateButton({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final update = ref.watch(desktopUpdateProvider);
    if (update.version == null) return const SizedBox.shrink();
    return IconButton(
      tooltip: t(ref, 'desktop.update.available'),
      onPressed: () => showDesktopUpdateDialog(context),
      icon: const Icon(Icons.system_update_alt, size: 16),
      padding: EdgeInsets.zero,
      constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
      visualDensity: VisualDensity.compact,
    );
  }
}

bool _showingUpdate = false;

Future<void> showDesktopUpdateDialog([BuildContext? context]) async {
  context ??= notificationResultsNavigatorKey.currentContext;
  if (_showingUpdate || context == null) return;
  _showingUpdate = true;
  try {
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (_) => const _DesktopUpdateDialog(),
    );
  } finally {
    _showingUpdate = false;
  }
}

class _DesktopUpdateDialog extends ConsumerWidget {
  const _DesktopUpdateDialog();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final update = ref.watch(desktopUpdateProvider);
    final windows = defaultTargetPlatform == TargetPlatform.windows;
    return PopScope(
      canPop: !update.busy,
      child: AlertDialog(
        title: Text(
          t(ref, 'desktop.update.title', {'version': update.version ?? ''}),
        ),
        content: Text(
          t(
                ref,
                update.busy
                    ? 'desktop.update.busy'
                    : update.failed
                    ? 'desktop.update.failed'
                    : windows
                    ? 'desktop.update.windows_message'
                    : 'desktop.update.message',
              ) +
              (update.failed && update.error != null
                  ? '\n\n${update.error}'
                  : ''),
        ),
        actions: [
          TextButton(
            onPressed: update.busy
                ? null
                : () {
                    ref.read(desktopUpdateProvider.notifier).dismiss();
                    Navigator.of(context).pop();
                  },
            child: Text(t(ref, 'desktop.update.later')),
          ),
          FilledButton(
            onPressed: update.busy
                ? null
                : () async {
                    final quit = ref.read(quitAppProvider);
                    final notifier = ref.read(desktopUpdateProvider.notifier);
                    final result = await notifier.perform();
                    if (result == DesktopUpdateAction.installed) {
                      await quit();
                    } else if (result == DesktopUpdateAction.openedDownloads) {
                      if (context.mounted) await notifier.dismiss();
                      if (context.mounted) Navigator.of(context).pop();
                    }
                  },
            child: Text(
              t(
                ref,
                windows
                    ? 'desktop.update.open_download'
                    : 'desktop.update.approve',
              ),
            ),
          ),
        ],
      ),
    );
  }
}
