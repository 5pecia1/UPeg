// Check a built native library's FRB exports and compiled-in tool metadata.
//
// Run with `dart run tool/frb_link_smoke.dart <absolute framework binary path>`.
// This does not start the app, host, or user-provided runtime sources.
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:upeg/src/rust/api/events.dart' as events_api;
import 'package:upeg/src/rust/api/tools.dart' as tools_api;
import 'package:upeg/src/rust/frb_generated.dart';

Future<void> main(List<String> args) async {
  if (args.length != 1 || !File(args.single).isAbsolute) {
    stderr.writeln(
      'usage: dart run tool/frb_link_smoke.dart <absolute binary path>',
    );
    exitCode = 64;
    return;
  }

  await RustLib.init(externalLibrary: ExternalLibrary.open(args.single));
  final toolkitIds =
      tools_api.listToolkits().map((toolkit) => toolkit.id).toList()..sort();
  final toolIds = tools_api.listTools().map((tool) => tool.id).toList()..sort();
  for (final expected in const ['embed.transform_tools', 'time.epoch']) {
    if (!toolIds.contains(expected)) {
      throw StateError('compiled-in tool missing after FRB load: $expected');
    }
  }

  // Exercise an async FRB worker call without invoking a native Toolkit.
  // This process has no focus-loss subscribers, so the broadcast only
  // traverses an empty in-memory registry.
  await events_api.focusLossBroadcast().timeout(const Duration(seconds: 5));
  stdout.writeln(jsonEncode({'toolkit_ids': toolkitIds, 'tool_ids': toolIds}));
}
