/// Unit tests for `ControlledEmbedDebug` models and helpers.
/// These tests verify the debug event model, selector probe helpers, and
/// debug session management without any Flutter WebView or CDP dependencies.
library;

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/widgets/controlled_embed/debug.dart';

void main() {
  group('ControlledEmbedDebugLevel', () {
    test('debug_levels_are_defined_in_the_correct_order', () {
      expect(ControlledEmbedDebugLevel.values.length, equals(4));
      expect(
        ControlledEmbedDebugLevel.info.index,
        lessThan(ControlledEmbedDebugLevel.success.index),
      );
      expect(
        ControlledEmbedDebugLevel.success.index,
        lessThan(ControlledEmbedDebugLevel.warning.index),
      );
      expect(
        ControlledEmbedDebugLevel.warning.index,
        lessThan(ControlledEmbedDebugLevel.error.index),
      );
    });
  });

  group('ControlledEmbedDebugPhase', () {
    test('debug_phases_cover_every_pipeline_stage', () {
      expect(
        ControlledEmbedDebugPhase.values,
        contains(ControlledEmbedDebugPhase.setup),
      );
      expect(
        ControlledEmbedDebugPhase.values,
        contains(ControlledEmbedDebugPhase.webview),
      );
      expect(
        ControlledEmbedDebugPhase.values,
        contains(ControlledEmbedDebugPhase.probe),
      );
      expect(
        ControlledEmbedDebugPhase.values,
        contains(ControlledEmbedDebugPhase.write),
      );
      expect(
        ControlledEmbedDebugPhase.values,
        contains(ControlledEmbedDebugPhase.trigger),
      );
      expect(
        ControlledEmbedDebugPhase.values,
        contains(ControlledEmbedDebugPhase.read),
      );
      expect(
        ControlledEmbedDebugPhase.values,
        contains(ControlledEmbedDebugPhase.console),
      );
      expect(ControlledEmbedDebugPhase.values.length, equals(7));
    });
  });

  group('ControlledEmbedDebugTargetKind', () {
    test('target_kinds_include_flutterWebView_and_localBrowserCdp', () {
      expect(
        ControlledEmbedDebugTargetKind.values,
        contains(ControlledEmbedDebugTargetKind.flutterWebView),
      );
      expect(
        ControlledEmbedDebugTargetKind.values,
        contains(ControlledEmbedDebugTargetKind.localBrowserCdp),
      );
      expect(ControlledEmbedDebugTargetKind.values.length, equals(2));
    });
  });

  group('ControlledEmbedDebugEvent', () {
    test('a_debug_event_preserves_all_fields', () {
      final event = ControlledEmbedDebugEvent(
        kind: 'test-kind',
        phase: ControlledEmbedDebugPhase.probe,
        level: ControlledEmbedDebugLevel.info,
        message: 'Test message',
        details: {'key': 'value'},
        elapsedMs: 150,
        sequence: 1,
      );
      expect(event.kind, equals('test-kind'));
      expect(event.phase, equals(ControlledEmbedDebugPhase.probe));
      expect(event.level, equals(ControlledEmbedDebugLevel.info));
      expect(event.message, equals('Test message'));
      expect(event.details, equals({'key': 'value'}));
      expect(event.elapsedMs, equals(150));
      expect(event.sequence, equals(1));
    });

    test('debug_event_order_is_preserved', () {
      final events = [
        ControlledEmbedDebugEvent(
          kind: 'setup',
          phase: ControlledEmbedDebugPhase.setup,
          level: ControlledEmbedDebugLevel.info,
          message: 'Start',
          details: null,
          elapsedMs: 0,
          sequence: 0,
        ),
        ControlledEmbedDebugEvent(
          kind: 'probe',
          phase: ControlledEmbedDebugPhase.probe,
          level: ControlledEmbedDebugLevel.success,
          message: 'Found elements',
          details: null,
          elapsedMs: 50,
          sequence: 1,
        ),
        ControlledEmbedDebugEvent(
          kind: 'read',
          phase: ControlledEmbedDebugPhase.read,
          level: ControlledEmbedDebugLevel.info,
          message: 'Completed',
          details: null,
          elapsedMs: 200,
          sequence: 2,
        ),
      ];
      expect(events[0].sequence, lessThan(events[1].sequence));
      expect(events[1].sequence, lessThan(events[2].sequence));
      expect(events[0].elapsedMs, lessThan(events[1].elapsedMs));
      expect(events[1].elapsedMs, lessThan(events[2].elapsedMs));
    });

    test('null_details_are_supported', () {
      final event = ControlledEmbedDebugEvent(
        kind: 'test',
        phase: ControlledEmbedDebugPhase.console,
        level: ControlledEmbedDebugLevel.warning,
        message: 'Warning',
        details: null,
        elapsedMs: 10,
        sequence: 0,
      );
      expect(event.details, isNull);
    });
  });

  group('ControlledEmbedDebugCapabilities', () {
    test('Flutter_WebView_default_capabilities_are_configured_correctly', () {
      final caps = ControlledEmbedDebugCapabilities.flutterWebView();
      expect(caps.visibleWebView, isTrue);
      expect(caps.consoleCapture, isFalse);
      expect(caps.selectorProbe, isTrue);
      expect(caps.screenshot, isFalse);
      expect(caps.cdpAttach, isFalse);
      expect(caps.networkCapture, isFalse);
    });

    test(
      'the_unimplemented_localBrowserCdp_backend_disables_all_capabilities',
      () {
        // localBrowserCdp is a planned, not-yet-implemented backend. The
        // capability report must not advertise features that have no code
        // path behind them — every flag stays false until implemented.
        final caps = ControlledEmbedDebugCapabilities.localBrowserCdp();
        expect(caps.visibleWebView, isFalse);
        expect(caps.consoleCapture, isFalse);
        expect(caps.selectorProbe, isFalse);
        expect(caps.screenshot, isFalse);
        expect(caps.cdpAttach, isFalse);
        expect(caps.networkCapture, isFalse);
      },
    );

    test('the_default_constructor_disables_all_capabilities', () {
      final caps = ControlledEmbedDebugCapabilities();
      expect(caps.visibleWebView, isFalse);
      expect(caps.consoleCapture, isFalse);
      expect(caps.selectorProbe, isFalse);
      expect(caps.screenshot, isFalse);
      expect(caps.cdpAttach, isFalse);
      expect(caps.networkCapture, isFalse);
    });
  });

  group('ControlledEmbedSelectorProbe', () {
    test('a_selector_probe_preserves_all_fields', () {
      final probe = ControlledEmbedSelectorProbe(
        role: BindingRoleDto.output,
        field: 'result',
        selector: '#result',
        matched: true,
        valuePreview: 'test-value',
        textPreview: 'test text',
      );
      expect(probe.role, equals(BindingRoleDto.output));
      expect(probe.field, equals('result'));
      expect(probe.selector, equals('#result'));
      expect(probe.matched, isTrue);
      expect(probe.valuePreview, equals('test-value'));
      expect(probe.textPreview, equals('test text'));
    });

    test('an_unmatched_selector_returns_matched_false_and_empty_previews', () {
      final probe = ControlledEmbedSelectorProbe(
        role: BindingRoleDto.output,
        field: 'missing',
        selector: '#missing-element',
        matched: false,
        valuePreview: '',
        textPreview: '',
      );
      expect(probe.matched, isFalse);
      expect(probe.valuePreview, isEmpty);
      expect(probe.textPreview, isEmpty);
    });
  });

  group('ControlledEmbedDebugSession', () {
    test('a_new_session_starts_with_an_empty_event_list', () {
      final session = ControlledEmbedDebugSession(
        targetKind: ControlledEmbedDebugTargetKind.flutterWebView,
        capabilities: ControlledEmbedDebugCapabilities.flutterWebView(),
      );
      expect(session.events, isEmpty);
      expect(session.latestProbes, isEmpty);
      expect(session.latestOutputs, isEmpty);
    });

    test('events_can_be_added_to_a_session', () {
      final session = ControlledEmbedDebugSession(
        targetKind: ControlledEmbedDebugTargetKind.flutterWebView,
        capabilities: ControlledEmbedDebugCapabilities.flutterWebView(),
      );
      session.addEvent(
        ControlledEmbedDebugEvent(
          kind: 'test',
          phase: ControlledEmbedDebugPhase.setup,
          level: ControlledEmbedDebugLevel.info,
          message: 'Started',
          details: null,
          elapsedMs: 0,
          sequence: 0,
        ),
      );
      expect(session.events.length, equals(1));
      expect(session.events.first.message, equals('Started'));
    });

    test('adding_multiple_events_to_a_session_preserves_their_order', () {
      final session = ControlledEmbedDebugSession(
        targetKind: ControlledEmbedDebugTargetKind.flutterWebView,
        capabilities: ControlledEmbedDebugCapabilities.flutterWebView(),
      );
      for (var i = 0; i < 5; i++) {
        session.addEvent(
          ControlledEmbedDebugEvent(
            kind: 'event-$i',
            phase: ControlledEmbedDebugPhase.console,
            level: ControlledEmbedDebugLevel.info,
            message: 'Event $i',
            details: null,
            elapsedMs: i * 10,
            sequence: i,
          ),
        );
      }
      expect(session.events.length, equals(5));
      for (var i = 0; i < 5; i++) {
        expect(session.events[i].sequence, equals(i));
      }
    });

    test('latestProbes_and_latestOutputs_can_be_updated', () {
      final session = ControlledEmbedDebugSession(
        targetKind: ControlledEmbedDebugTargetKind.flutterWebView,
        capabilities: ControlledEmbedDebugCapabilities.flutterWebView(),
      );
      final probes = [
        ControlledEmbedSelectorProbe(
          role: BindingRoleDto.output,
          field: 'result',
          selector: '#result',
          matched: true,
          valuePreview: 'ok',
          textPreview: 'Result',
        ),
      ];
      session.latestProbes = probes;
      final outputs = {'result': 'ok'};
      session.latestOutputs = outputs;
      expect(session.latestProbes.length, equals(1));
      expect(session.latestOutputs, equals(outputs));
    });

    test('targetKind_and_capabilities_are_stored_correctly', () {
      final session = ControlledEmbedDebugSession(
        targetKind: ControlledEmbedDebugTargetKind.localBrowserCdp,
        capabilities: ControlledEmbedDebugCapabilities.localBrowserCdp(),
      );
      expect(
        session.targetKind,
        equals(ControlledEmbedDebugTargetKind.localBrowserCdp),
      );
      // localBrowserCdp is unimplemented — capabilities are honestly false.
      expect(session.capabilities.cdpAttach, isFalse);
      expect(session.capabilities.networkCapture, isFalse);
    });

    test('the_flutterWebView_target_has_cdpAttach_false', () {
      final session = ControlledEmbedDebugSession(
        targetKind: ControlledEmbedDebugTargetKind.flutterWebView,
        capabilities: ControlledEmbedDebugCapabilities.flutterWebView(),
      );
      expect(
        session.targetKind,
        equals(ControlledEmbedDebugTargetKind.flutterWebView),
      );
      expect(session.capabilities.cdpAttach, isFalse);
      expect(session.capabilities.networkCapture, isFalse);
    });
  });

  group('buildControlledEmbedSelectorProbeScript', () {
    test('an_empty_binding_list_returns_an_empty_array_string', () {
      final script = buildControlledEmbedSelectorProbeScript([]);
      expect(script, contains('[]'));
    });

    test('generates_a_script_for_a_single_binding', () {
      final script = buildControlledEmbedSelectorProbeScript([
        const SelectorBindingDto(
          role: BindingRoleDto.output,
          field: 'result',
          selector: '#result',
          triggerAction: ControlledEmbedTriggerActionDto.click,
        ),
      ]);
      expect(script, contains('querySelector'));
      expect(script, contains('#result'));
      expect(script, contains('JSON.stringify'));
    });

    test('generates_a_script_for_multiple_bindings', () {
      final script = buildControlledEmbedSelectorProbeScript([
        const SelectorBindingDto(
          role: BindingRoleDto.input,
          field: 'q',
          selector: '#q',
          triggerAction: ControlledEmbedTriggerActionDto.click,
        ),
        const SelectorBindingDto(
          role: BindingRoleDto.output,
          field: 'result',
          selector: '.result',
          triggerAction: ControlledEmbedTriggerActionDto.click,
        ),
        const SelectorBindingDto(
          role: BindingRoleDto.output,
          field: 'status',
          selector: '#status',
          triggerAction: ControlledEmbedTriggerActionDto.click,
        ),
      ]);
      expect(script, contains('querySelector'));
      expect(script, contains('#q'));
      expect(script, contains('.result'));
      expect(script, contains('#status'));
      expect(script, contains('JSON.stringify'));
    });

    test('a_matched_output_binding_reports_its_role_as_output', () {
      // Regression: the matched branch previously derived role from
      // field-name suffix heuristics (__trigger/__output), so every
      // matched element reported role:"input". It must instead carry the
      // binding's declared role. The field here ('result') has neither
      // heuristic suffix, so the old code would have reported "input".
      final script = buildControlledEmbedSelectorProbeScript([
        const SelectorBindingDto(
          role: BindingRoleDto.output,
          field: 'result',
          selector: '#result',
          triggerAction: ControlledEmbedTriggerActionDto.click,
        ),
      ]);
      // The matched push (matched:true) now carries the declared role.
      expect(
        script,
        contains(
          'role:"output",field:"result",selector:"#result",matched:true',
        ),
      );
      // The old field-name heuristic is gone entirely.
      expect(script, isNot(contains('__trigger')));
      expect(script, isNot(contains('__output')));
    });

    test(
      'a_matched_trigger_binding_reports_the_trigger_role_regardless_of_field_name',
      () {
        // 'submit' does not start with __trigger, so the retired heuristic
        // would have mislabeled this matched element as "input".
        final script = buildControlledEmbedSelectorProbeScript([
          const SelectorBindingDto(
            role: BindingRoleDto.trigger,
            field: 'submit',
            selector: '#go',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ]);
        expect(
          script,
          contains('role:"trigger",field:"submit",selector:"#go",matched:true'),
        );
      },
    );

    test('selectors_are_escaped_for_safe_script_generation', () {
      // Test with a selector that needs JSON escaping
      final script = buildControlledEmbedSelectorProbeScript([
        const SelectorBindingDto(
          role: BindingRoleDto.output,
          field: 'content',
          selector: "input[name='test']",
          triggerAction: ControlledEmbedTriggerActionDto.click,
        ),
      ]);
      // Should properly escape the selector - check for escaped quotes in the output
      // The selector should appear with escaped quotes: "input[name='test']"
      expect(script, contains(r'"input[name='));
      expect(script, contains(r"'test'"));
    });
  });

  group('parseControlledEmbedSelectorProbes', () {
    test('null_input_returns_an_empty_list', () {
      final probes = parseControlledEmbedSelectorProbes(null);
      expect(probes, isEmpty);
    });

    test('an_empty_object_input_returns_an_empty_list', () {
      final probes = parseControlledEmbedSelectorProbes({});
      expect(probes, isEmpty);
    });

    test('an_empty_array_input_returns_an_empty_list', () {
      final probes = parseControlledEmbedSelectorProbes([]);
      expect(probes, isEmpty);
    });

    test('parses_a_valid_probe_array', () {
      final raw = jsonEncode([
        {
          'role': 'output',
          'field': 'result',
          'selector': '#result',
          'matched': true,
          'valuePreview': 'test-value',
          'textPreview': 'Test Text',
        },
        {
          'role': 'output',
          'field': 'status',
          'selector': '#status',
          'matched': false,
          'valuePreview': '',
          'textPreview': '',
        },
      ]);
      final parsed = jsonDecode(raw);
      final probes = parseControlledEmbedSelectorProbes(parsed);
      expect(probes.length, equals(2));
      expect(probes[0].field, equals('result'));
      expect(probes[0].matched, isTrue);
      expect(probes[0].valuePreview, equals('test-value'));
      expect(probes[1].field, equals('status'));
      expect(probes[1].matched, isFalse);
    });

    test('an_unmatched_selector_returns_matched_false', () {
      final raw = jsonEncode([
        {
          'role': 'output',
          'field': 'missing',
          'selector': '#missing',
          'matched': false,
          'valuePreview': '',
          'textPreview': '',
        },
      ]);
      final parsed = jsonDecode(raw);
      final probes = parseControlledEmbedSelectorProbes(parsed);
      expect(probes.length, equals(1));
      expect(probes[0].matched, isFalse);
      expect(probes[0].valuePreview, isEmpty);
      expect(probes[0].textPreview, isEmpty);
    });

    test('malformed_input_returns_an_empty_list', () {
      // Not a list
      final probes1 = parseControlledEmbedSelectorProbes('not a list');
      expect(probes1, isEmpty);

      // List of strings instead of objects
      final probes2 = parseControlledEmbedSelectorProbes(['a', 'b']);
      expect(probes2, isEmpty);
    });
  });

  group('previewDebugValue', () {
    test('strings_within_the_default_160_character_limit_are_unchanged', () {
      final short = 'a' * 100;
      expect(previewDebugValue(short), equals(short));
    });

    test('strings_longer_than_160_characters_are_truncated', () {
      final long = 'a' * 200;
      final result = previewDebugValue(long);
      expect(result.length, equals(160));
      expect(result.endsWith('…'), isTrue);
    });

    test('replaces_newline_characters_with_a_single_space', () {
      final withNewlines = 'line1\nline2\r\nline3';
      final result = previewDebugValue(withNewlines);
      expect(result.contains('\n'), isFalse);
      expect(result.contains('\r'), isFalse);
    });

    test('collapses_multiple_newlines_into_a_single_space', () {
      final withMultipleNewlines = 'a\n\n\n\n\nb';
      final result = previewDebugValue(withMultipleNewlines);
      expect(result.contains('\n'), isFalse);
      expect(result, equals('a b'));
    });

    test('the_max_parameter_changes_the_truncation_length', () {
      final value = 'a' * 100;
      expect(previewDebugValue(value, max: 50).length, equals(50));
      expect(previewDebugValue(value, max: 50).endsWith('…'), isTrue);
    });

    test('an_empty_string_returns_an_empty_string', () {
      expect(previewDebugValue(''), isEmpty);
    });
  });

  group('Integration: Debug Session with Probes', () {
    test('simulates_a_real_world_usage_pattern', () {
      // Create session
      final session = ControlledEmbedDebugSession(
        targetKind: ControlledEmbedDebugTargetKind.flutterWebView,
        capabilities: ControlledEmbedDebugCapabilities.flutterWebView(),
      );

      // Add setup event
      session.addEvent(
        ControlledEmbedDebugEvent(
          kind: 'init',
          phase: ControlledEmbedDebugPhase.setup,
          level: ControlledEmbedDebugLevel.info,
          message: 'Debug session started',
          details: {'target': 'flutterWebView'},
          elapsedMs: 0,
          sequence: 0,
        ),
      );

      // Simulate probe execution
      final probeScript = buildControlledEmbedSelectorProbeScript([
        const SelectorBindingDto(
          role: BindingRoleDto.output,
          field: 'result',
          selector: '#result',
          triggerAction: ControlledEmbedTriggerActionDto.click,
        ),
      ]);
      expect(probeScript, isNotEmpty);

      // Simulate probe result (JSON string from WebView)
      final probeResultJson = jsonEncode([
        {
          'role': 'output',
          'field': 'result',
          'selector': '#result',
          'matched': true,
          'valuePreview': 'success',
          'textPreview': 'Result Text',
        },
      ]);
      final probeResult = jsonDecode(probeResultJson);
      final probes = parseControlledEmbedSelectorProbes(probeResult);
      session.latestProbes = probes;

      // Add probe event
      session.addEvent(
        ControlledEmbedDebugEvent(
          kind: 'probe',
          phase: ControlledEmbedDebugPhase.probe,
          level: ControlledEmbedDebugLevel.success,
          message: 'Probed 1 selector',
          details: {'matched': 1, 'total': 1},
          elapsedMs: 50,
          sequence: 1,
        ),
      );

      // Verify session state
      expect(session.events.length, equals(2));
      expect(session.latestProbes.length, equals(1));
      expect(session.latestProbes.first.matched, isTrue);
      expect(session.latestProbes.first.field, equals('result'));

      // Add outputs
      session.latestOutputs = {'result': 'success'};

      // Verify outputs stored
      expect(session.latestOutputs['result'], equals('success'));

      // Verify capabilities preserved
      expect(session.capabilities.visibleWebView, isTrue);
      expect(session.capabilities.cdpAttach, isFalse);
    });
  });
}
