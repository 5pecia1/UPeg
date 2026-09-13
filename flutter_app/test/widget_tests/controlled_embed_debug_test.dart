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
    test('디버그_레벨_순서가_올바르게_정의된다', () {
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
    test('디버그_단계가_모든_파이프라인_단계를_포함한다', () {
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
    test('타겟_종류가_flutterWebView와_localBrowserCdp를_포함한다', () {
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
    test('디버그_이벤트가_모든_필드를_正しく_보존한다', () {
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

    test('디버그_이벤트_순서_보존', () {
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

    test('null_details도_정상_처리된다', () {
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
    test('Flutter_WebView_기본_기능이_올바르게_설정된다', () {
      final caps = ControlledEmbedDebugCapabilities.flutterWebView();
      expect(caps.visibleWebView, isTrue);
      expect(caps.consoleCapture, isFalse);
      expect(caps.selectorProbe, isTrue);
      expect(caps.screenshot, isFalse);
      expect(caps.cdpAttach, isFalse);
      expect(caps.networkCapture, isFalse);
    });

    test('localBrowserCdp_미구현_백엔드는_모든_기능이_비활성화된다', () {
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
    });

    test('빈_기본값_생성_가능', () {
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
    test('셀렉터_프로브가_모든_필드를_보존한다', () {
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

    test('매칭되지_않은_셀렉터는_matched_false와_빈_미리보기를_반환한다', () {
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
    test('새_세션은_빈_이벤트_리스트로_초기화된다', () {
      final session = ControlledEmbedDebugSession(
        targetKind: ControlledEmbedDebugTargetKind.flutterWebView,
        capabilities: ControlledEmbedDebugCapabilities.flutterWebView(),
      );
      expect(session.events, isEmpty);
      expect(session.latestProbes, isEmpty);
      expect(session.latestOutputs, isEmpty);
    });

    test('세션에_이벤트를_추가할_수_있다', () {
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

    test('세션에_여러_이벤트를_추가하면_순서가_보존된다', () {
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

    test('latestProbes와_latestOutputs를_업데이트할_수_있다', () {
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

    test('targetKind와_capabilities가_올바르게_저장된다', () {
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

    test('flutterWebView_타겟은_cdpAttach가_false이다', () {
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
    test('빈_바인딩_리스트는_빈_배열_문자열을_반환한다', () {
      final script = buildControlledEmbedSelectorProbeScript([]);
      expect(script, contains('[]'));
    });

    test('단일_바인딩에_대한_스크립트를_생성한다', () {
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

    test('여러_바인딩에_대한_스크립트를_생성한다', () {
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

    test('매칭된_output_role_바인딩은_role을_output으로_보고한다', () {
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

    test('매칭된_trigger_role_바인딩은_field_이름과_무관하게_role을_trigger로_보고한다', () {
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
    });

    test('selector가_escaped_되어_안전하게_生成된다', () {
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
    test('null_입력은_빈_리스트를_반환한다', () {
      final probes = parseControlledEmbedSelectorProbes(null);
      expect(probes, isEmpty);
    });

    test('빈_객체_입력은_빈_리스트를_반환한다', () {
      final probes = parseControlledEmbedSelectorProbes({});
      expect(probes, isEmpty);
    });

    test('빈_배열_입력은_빈_리스트를_반환한다', () {
      final probes = parseControlledEmbedSelectorProbes([]);
      expect(probes, isEmpty);
    });

    test('유효한_probe_배열을_파싱한다', () {
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

    test('매칭되지_않은_셀렉터는_matched_false를_반환한다', () {
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

    test('잘못된_형식_입력은_빈_리스트를_반환한다', () {
      // Not a list
      final probes1 = parseControlledEmbedSelectorProbes('not a list');
      expect(probes1, isEmpty);

      // List of strings instead of objects
      final probes2 = parseControlledEmbedSelectorProbes(['a', 'b']);
      expect(probes2, isEmpty);
    });
  });

  group('previewDebugValue', () {
    test('기본_160자_이내_문자열은_변환되지_않는다', () {
      final short = 'a' * 100;
      expect(previewDebugValue(short), equals(short));
    });

    test('160자_초과_문자열은_자른다', () {
      final long = 'a' * 200;
      final result = previewDebugValue(long);
      expect(result.length, equals(160));
      expect(result.endsWith('…'), isTrue);
    });

    test('줄바꿈_문자를_단일_공백으로_대체한다', () {
      final withNewlines = 'line1\nline2\r\nline3';
      final result = previewDebugValue(withNewlines);
      expect(result.contains('\n'), isFalse);
      expect(result.contains('\r'), isFalse);
    });

    test('여러_줄바꿈을_단일_공백으로_압축한다', () {
      final withMultipleNewlines = 'a\n\n\n\n\nb';
      final result = previewDebugValue(withMultipleNewlines);
      expect(result.contains('\n'), isFalse);
      expect(result, equals('a b'));
    });

    test('max_파라미터로_자르기_길이_변경', () {
      final value = 'a' * 100;
      expect(previewDebugValue(value, max: 50).length, equals(50));
      expect(previewDebugValue(value, max: 50).endsWith('…'), isTrue);
    });

    test('빈_문자열은_빈_문자열로_반환', () {
      expect(previewDebugValue(''), isEmpty);
    });
  });

  group('Integration: Debug Session with Probes', () {
    test('실제_사용_패턴을_시뮬레이션한다', () {
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
