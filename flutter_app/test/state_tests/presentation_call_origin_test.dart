import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/state/presentation_call_origin.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

void main() {
  group('PresentationCallOriginController', () {
    test('keeps a row action origin across a write follow-up', () {
      final controller = PresentationCallOriginController();
      final origin = controller.begin(
        toolId: 'catalog.list',
        args: ToolArgs.fromJsonObject(const {'project': '/a'}),
        host: 'board-a',
      );
      expect(
        controller.captureOriginForRowAction(origin, originEffectIsRead: true),
        isTrue,
      );

      final write = controller.beginFollowup(
        toolId: 'catalog.apply',
        args: ToolArgs.fromJsonObject(const {'id': 'one'}),
        host: 'board-a',
      );
      final refresh = controller.writeSucceeded(
        write: write,
        refreshOrigin: true,
      );

      expect(refresh?.origin, origin);
      expect(refresh?.origin.args.toJsonObject(), {'project': '/a'});
      expect(refresh?.origin.host, 'board-a');
      expect(controller.writeStatus, isA<PresentationWriteSucceeded>());
    });

    test('rejects a late result after another project replaces the list', () {
      final controller = PresentationCallOriginController();
      final projectA = controller.begin(
        toolId: 'catalog.list',
        args: ToolArgs.fromJsonObject(const {'project': '/a'}),
        host: 'board',
      );
      final projectB = controller.begin(
        toolId: 'catalog.list',
        args: ToolArgs.fromJsonObject(const {'project': '/b'}),
        host: 'board',
      );

      expect(controller.acceptResult(projectA), isFalse);
      expect(controller.acceptResult(projectB), isTrue);
      expect(controller.active?.args.toJsonObject(), {'project': '/b'});
    });

    test('does not refresh when the captured origin has been replaced', () {
      final controller = PresentationCallOriginController();
      final origin = controller.begin(
        toolId: 'catalog.list',
        args: ToolArgs.fromJsonObject(const {'project': '/a'}),
        host: null,
      );
      controller.captureOriginForRowAction(origin, originEffectIsRead: true);
      final write = controller.beginFollowup(
        toolId: 'catalog.apply',
        args: ToolArgs.fromJsonObject(const {'id': 'one'}),
        host: null,
      );
      final refresh = controller.writeSucceeded(
        write: write,
        refreshOrigin: true,
      )!;
      controller.begin(
        toolId: 'catalog.list',
        args: ToolArgs.fromJsonObject(const {'project': '/b'}),
        host: null,
      );

      expect(controller.beginRefresh(refresh), isNull);
    });

    test('marks a cancelled write as unconfirmed without retrying it', () {
      final controller = PresentationCallOriginController();
      final origin = controller.begin(
        toolId: 'catalog.list',
        args: ToolArgs.fromJsonObject(const {'project': '/a'}),
        host: null,
      );
      controller.captureOriginForRowAction(origin, originEffectIsRead: true);
      final write = controller.beginFollowup(
        toolId: 'catalog.apply',
        args: ToolArgs.fromJsonObject(const {'id': 'one'}),
        host: null,
      );

      controller.writeUnconfirmed(write);

      expect(controller.writeStatus, isA<PresentationWriteUnconfirmed>());
      expect(controller.origin, origin);
    });

    test('distinguishes a confirmed write failure from transport loss', () {
      final controller = PresentationCallOriginController();
      final origin = controller.begin(
        toolId: 'catalog.list',
        args: ToolArgs.empty,
        host: null,
      );
      controller.captureOriginForRowAction(origin, originEffectIsRead: true);
      final write = controller.beginFollowup(
        toolId: 'catalog.apply',
        args: ToolArgs.empty,
        host: null,
      );

      controller.writeFailed(write);

      expect(controller.writeStatus, isA<PresentationWriteFailed>());
      expect(controller.origin, origin);
    });

    test('only refreshes a declared read origin', () {
      final controller = PresentationCallOriginController();
      final origin = controller.begin(
        toolId: 'catalog.unknown',
        args: ToolArgs.empty,
        host: null,
      );
      controller.captureOriginForRowAction(origin, originEffectIsRead: false);
      final write = controller.beginFollowup(
        toolId: 'catalog.apply',
        args: ToolArgs.empty,
        host: null,
      );

      final refresh = controller.writeSucceeded(
        write: write,
        refreshOrigin: true,
      );

      expect(refresh, isNull);
      expect(
        (controller.writeStatus as PresentationWriteSucceeded).refreshPending,
        isFalse,
      );
    });

    test(
      'rejects a refresh response after a later refresh generation starts',
      () {
        final controller = PresentationCallOriginController();
        final origin = controller.begin(
          toolId: 'catalog.list',
          args: ToolArgs.fromJsonObject(const {'project': '/a'}),
          host: 'board-a',
        );
        controller.captureOriginForRowAction(origin, originEffectIsRead: true);
        final write = controller.beginFollowup(
          toolId: 'catalog.apply',
          args: ToolArgs.empty,
          host: 'board-a',
        );
        final request = controller.writeSucceeded(
          write: write,
          refreshOrigin: true,
        )!;
        final firstRefresh = controller.beginRefresh(request)!;
        final secondRefresh = controller.beginRefresh(request)!;

        expect(controller.acceptRefresh(firstRefresh), isFalse);
        expect(controller.acceptRefresh(secondRefresh), isTrue);
        expect(secondRefresh.call.args.toJsonObject(), {'project': '/a'});
        expect(secondRefresh.call.host, 'board-a');
      },
    );

    test(
      'refuses a follow-up when its captured board host is no longer current',
      () {
        final controller = PresentationCallOriginController();
        controller.begin(
          toolId: 'catalog.list',
          args: ToolArgs.empty,
          host: 'board-a',
        );

        expect(controller.activeHostMatches('board-a'), isTrue);
        expect(controller.activeHostMatches('board-b'), isFalse);
      },
    );
  });
}
