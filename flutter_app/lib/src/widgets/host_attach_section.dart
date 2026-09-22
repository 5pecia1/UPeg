/// Settings → Host attach section (Task B3).
///
/// Lets the user pair the PWA with a local `upeg` daemon: a base URL, a
/// bearer token, and a check-connection action that probes `/healthz` and reports
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
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/state/external_readiness_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/external_readiness_panel.dart';

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
const String kHostAttachUnauthorizedKey =
    'host_attach.notice.unauthorized_hint';
const String kHostAttachUnavailableKey = 'readiness.host_unavailable';

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
  List<AttachToolSummary> _hostTools = const <AttachToolSummary>[];
  HostAttachConfig? _catalogConfig;
  int _configurationRevision = 0;

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
    final next = HostAttachConfig(
      baseUrl: _baseUrlController.text.trim(),
      token: _tokenController.text.trim(),
    );
    if (next == ref.read(hostAttachConfigProvider)) return;
    ref.read(hostAttachConfigProvider.notifier).save(next);
    setState(() {
      _configurationRevision += 1;
      _checking = false;
      _statusKey = null;
      _hostTools = const <AttachToolSummary>[];
      _catalogConfig = null;
    });
  }

  Future<void> _check() async {
    _persist();
    final revision = _configurationRevision;
    setState(() {
      _checking = true;
      _statusKey = kHostAttachCheckingKey;
    });
    final client = ref.read(attachClientProvider);
    final result = await client.checkHealth();
    if (!mounted || revision != _configurationRevision) return;
    final listed = result is HealthzOk ? await client.listTools() : null;
    if (!mounted || revision != _configurationRevision) return;
    setState(() {
      _checking = false;
      _statusKey = switch ((result, listed)) {
        (HealthzOk(), AttachListOk()) => kHostAttachConnectedKey,
        (HealthzOk(), AttachListUnauthorized()) => kHostAttachUnauthorizedKey,
        (HealthzOk(), _) => kHostAttachUnavailableKey,
        (HealthzUnreachable(), _) => kHostAttachUnreachableKey,
      };
      _hostTools = listed is AttachListOk
          ? listed.tools
                .where(
                  (tool) =>
                      tool.supportsReadinessInspection &&
                      !tool.source.startsWith('mcp-import:'),
                )
                .toList(growable: false)
          : const <AttachToolSummary>[];
      _catalogConfig = listed is AttachListOk
          ? ref.read(hostAttachConfigProvider)
          : null;
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
        if (_hostTools.isNotEmpty) ...[
          const SizedBox(height: 8),
          Text(
            t(ref, 'readiness.remote_catalog_limit'),
            style: TextStyle(color: tokens.fg4, fontSize: 10.5),
          ),
          for (final tool in _hostTools)
            _HostToolReadiness(tool: tool, config: _catalogConfig!),
        ],
      ],
    );
  }
}

class _HostToolReadiness extends ConsumerWidget {
  const _HostToolReadiness({required this.tool, required this.config});

  final AttachToolSummary tool;
  final HostAttachConfig config;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final toolId = ToolId.parse(tool.id);
    final target = (toolId: toolId, config: config);
    final inspection = ref.watch(hostExternalReadinessProvider(target));
    if (inspection case AsyncData(value: ExternalReadinessNotApplicable())) {
      return const SizedBox.shrink();
    }
    final tokens = context.upeg;
    final platform = switch (inspection) {
      AsyncData(value: ExternalReadinessInspected(:final readiness)) =>
        readiness.platform,
      _ => null,
    };
    return Container(
      margin: const EdgeInsets.only(top: 8),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        border: Border.all(color: tokens.lineSoft),
        borderRadius: BorderRadius.circular(UpegSizing.radius1),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            platform == null
                ? '${tool.label} · ${tool.id}'
                : '${tool.label} · ${tool.id} · $platform',
            overflow: TextOverflow.ellipsis,
            style: TextStyle(color: tokens.fg2, fontSize: 11),
          ),
          const SizedBox(height: 6),
          ExternalReadinessInspectionView(
            inspection: inspection,
            onRecheck: () =>
                ref.invalidate(hostExternalReadinessProvider(target)),
            showReady: true,
          ),
        ],
      ),
    );
  }
}
