/// Settings → Host attach section (Task B3).
///
/// Lets the user pair the PWA with a local `upeg` daemon: a base URL, a
/// bearer token, and a "연결 확인" action that probes `/healthz` and reports
/// whether the daemon answered. A reachable host always serves the REST
/// data plane, so "connected" is the whole answer. Once
/// paired, the board routes in-process-unsupported tools through the daemon
/// (see `board_canvas.dart`).
///
/// Persistence flows through `hostAttachConfigProvider`; the health probe
/// goes through `attachClientProvider`. Both are Riverpod seams tests
/// override without a real browser store or network.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/features/host_attach/host_attach_config.dart';
import 'package:upeg/src/features/host_attach/host_attach_config_provider.dart';
import 'package:upeg/src/features/host_attach/host_attach_dispatch_provider.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

const Key kHostAttachBaseUrlFieldKey = Key('host-attach-base-url-field');
const Key kHostAttachTokenFieldKey = Key('host-attach-token-field');
const Key kHostAttachCheckButtonKey = Key('host-attach-check-button');
const Key kHostAttachStatusKey = Key('host-attach-status');

/// Catalog keys for the section copy — the strings live in the Rust
/// En/Ko catalog (upeg-pegboard-ui/src/i18n.rs) and flow through `t()`.
const String kHostAttachBaseUrlLabelKey = 'host_attach.base_url_label';
const String kHostAttachTokenLabelKey = 'host_attach.token_label';
const String kHostAttachCheckLabelKey = 'host_attach.check_button';
const String kHostAttachCheckingKey = 'host_attach.checking';
const String kHostAttachConnectedKey = 'host_attach.connected';
const String kHostAttachUnreachableKey = 'host_attach.unreachable';

class HostAttachSection extends ConsumerStatefulWidget {
  const HostAttachSection({super.key});

  @override
  ConsumerState<HostAttachSection> createState() => _HostAttachSectionState();
}

class _HostAttachSectionState extends ConsumerState<HostAttachSection> {
  late final TextEditingController _baseUrlController;
  late final TextEditingController _tokenController;

  /// Catalog key of the current status line (`null` = no status yet).
  /// Storing the key — not the rendered string — keeps the label live
  /// across locale flips.
  String? _statusKey;
  bool _checking = false;

  @override
  void initState() {
    super.initState();
    final config = ref.read(hostAttachConfigProvider);
    _baseUrlController = TextEditingController(text: config.baseUrl);
    _tokenController = TextEditingController(text: config.token);
  }

  @override
  void dispose() {
    _baseUrlController.dispose();
    _tokenController.dispose();
    super.dispose();
  }

  void _persist() {
    ref
        .read(hostAttachConfigProvider.notifier)
        .save(
          HostAttachConfig(
            baseUrl: _baseUrlController.text.trim(),
            token: _tokenController.text.trim(),
          ),
        );
  }

  Future<void> _check() async {
    _persist();
    setState(() {
      _checking = true;
      _statusKey = kHostAttachCheckingKey;
    });
    final result = await ref.read(attachClientProvider).checkHealth();
    if (!mounted) return;
    setState(() {
      _checking = false;
      _statusKey = switch (result) {
        HealthzOk() => kHostAttachConnectedKey,
        HealthzUnreachable() => kHostAttachUnreachableKey,
      };
    });
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        TextField(
          key: kHostAttachBaseUrlFieldKey,
          controller: _baseUrlController,
          onChanged: (_) => _persist(),
          decoration: InputDecoration(
            labelText: t(ref, kHostAttachBaseUrlLabelKey),
            isDense: true,
          ),
        ),
        const SizedBox(height: 8),
        TextField(
          key: kHostAttachTokenFieldKey,
          controller: _tokenController,
          onChanged: (_) => _persist(),
          obscureText: true,
          decoration: InputDecoration(
            labelText: t(ref, kHostAttachTokenLabelKey),
            isDense: true,
          ),
        ),
        const SizedBox(height: 8),
        Row(
          children: [
            // Flexible so longer locale variants of the label squeeze
            // instead of overflowing the settings column.
            Flexible(
              child: TextButton(
                key: kHostAttachCheckButtonKey,
                onPressed: _checking ? null : _check,
                child: Text(
                  t(ref, kHostAttachCheckLabelKey),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ),
            const SizedBox(width: 8),
            if (_statusKey != null)
              Expanded(
                child: Text(
                  t(ref, _statusKey!),
                  key: kHostAttachStatusKey,
                  style: TextStyle(
                    fontFamily: upegMonoFontFamily,
                    fontFamilyFallback: upegMonoFontFamilyFallback,
                    fontSize: 10.5,
                    color: tokens.fg3,
                  ),
                ),
              ),
          ],
        ),
      ],
    );
  }
}
