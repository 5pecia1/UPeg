/// Controlled-embed browser settings seams.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/embed.dart' as frb;

typedef ControlledEmbedSettingsLoader =
    frb.ControlledEmbedSettingsDto Function(ToolId toolId);

typedef ControlledEmbedSettingsSaver =
    void Function(ToolId toolId, frb.ControlledEmbedSettingsDto settings);

final controlledEmbedSettingsLoaderProvider =
    Provider<ControlledEmbedSettingsLoader>(
      (ref) =>
          (toolId) => frb.controlledEmbedSettingsFor(toolId: toolId.value),
    );

final controlledEmbedSettingsSaverProvider =
    Provider<ControlledEmbedSettingsSaver>(
      (ref) =>
          (toolId, settings) => frb.setControlledEmbedSettings(
            toolId: toolId.value,
            settings: settings,
          ),
    );
