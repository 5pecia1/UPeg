/// Displays HTTP tool outcomes and acknowledges only the result displayed.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/notification_results.dart';
import 'package:upeg/src/state/notification_results_provider.dart';

final notificationResultsNavigatorKey = GlobalKey<NavigatorState>();
final notificationResultsLoaderProvider =
    Provider<Future<List<NotificationResultDto>> Function()>(
      (ref) => listNotificationResults,
    );
final notificationResultsAcknowledgerProvider =
    Provider<Future<void> Function(List<String>)>(
      (ref) =>
          (ids) => acknowledgeNotificationResults(ids: ids),
    );

bool _showingResults = false;

Future<void> showNotificationResults() async {
  if (_showingResults) return;
  final context = notificationResultsNavigatorKey.currentContext;
  if (context == null) return;
  _showingResults = true;
  try {
    await showDialog<void>(
      context: context,
      builder: (_) => const _ResultsDialog(),
    );
  } finally {
    _showingResults = false;
  }
}

class _ResultsDialog extends ConsumerStatefulWidget {
  const _ResultsDialog();

  @override
  ConsumerState<_ResultsDialog> createState() => _ResultsDialogState();
}

class _ResultsDialogState extends ConsumerState<_ResultsDialog> {
  late final Future<List<NotificationResultDto>> _results = ref.read(
    notificationResultsLoaderProvider,
  )();
  final Set<String> _acknowledged = {};
  int _index = 0;

  void _acknowledgeAfterFrame(
    NotificationResultDto result,
    int displayedIndex,
  ) {
    if (!_acknowledged.add(result.id)) return;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted || _index != displayedIndex) {
        _acknowledged.remove(result.id);
        return;
      }
      try {
        await ref.read(notificationResultsAcknowledgerProvider)([result.id]);
        if (mounted) ref.invalidate(unreadNotificationResultsProvider);
      } on Object catch (error) {
        _acknowledged.remove(result.id);
        debugPrint('upeg: cannot acknowledge result: $error');
      }
    });
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: Text(t(ref, 'notifications.results')),
    content: SizedBox(
      width: 560,
      child: FutureBuilder<List<NotificationResultDto>>(
        future: _results,
        builder: (context, snapshot) {
          if (snapshot.hasError) {
            return Text(t(ref, 'notifications.load_failed'));
          }
          if (!snapshot.hasData) {
            return const Center(child: CircularProgressIndicator());
          }
          final results = snapshot.data!;
          if (results.isEmpty) return Text(t(ref, 'notifications.empty'));
          final result = results[_index];
          _acknowledgeAfterFrame(result, _index);
          return Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('${_index + 1} / ${results.length}'),
              const SizedBox(height: 8),
              Text(
                result.title,
                style: Theme.of(context).textTheme.titleMedium,
              ),
              const SizedBox(height: 8),
              Flexible(
                child: SingleChildScrollView(
                  child: SelectableText(result.summary),
                ),
              ),
              const SizedBox(height: 12),
              Wrap(
                children: [
                  TextButton(
                    onPressed: _index == 0
                        ? null
                        : () => setState(() => _index--),
                    child: Text(t(ref, 'notifications.previous')),
                  ),
                  TextButton(
                    onPressed: _index + 1 == results.length
                        ? null
                        : () => setState(() => _index++),
                    child: Text(t(ref, 'notifications.next')),
                  ),
                ],
              ),
            ],
          );
        },
      ),
    ),
    actions: [
      TextButton(
        onPressed: () => Navigator.pop(context),
        child: Text(t(ref, 'notifications.close')),
      ),
    ],
  );
}
