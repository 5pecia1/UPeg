import 'package:upeg/src/rust/api/board_details.dart';
import 'package:upeg/src/rust/api/boot.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';

const boardDetailsTestKey = 'review';
const boardDetailsTestDirectory = '/projects/review';
const boardDetailsTestManifest = '/projects/review/upeg.toml';
const boardDetailsTestConfig = '''{
  "mcpServers": {
    "upeg-review": {
      "command": "upeg",
      "args": ["--working-directory", "/projects/review", "mcp", "--board", "review"]
    }
  }
}''';

class FakeBoardDetails {
  FakeBoardDetails({
    this.project = false,
    this.native = true,
    this.saveFails = false,
    this.cliAvailable = true,
    this.projectChanged = false,
    this.unresolvedPins = false,
  });

  final bool project;
  final bool native;
  final bool saveFails;
  final bool cliAvailable;
  final bool projectChanged;
  final bool unresolvedPins;
  String description = '검토용 보드';
  String instructions = '# 점검\n먼저 상태를 확인한다.';

  Future<BoardDetailsDto> load(String key) async {
    if (projectChanged) {
      throw const FrbError.projectManifestChanged(
        path: boardDetailsTestManifest,
      );
    }
    return BoardDetailsDto(
      boardKey: key,
      title: 'Review',
      description: description,
      instructions: instructions,
      projectManifestPath: project ? boardDetailsTestManifest : null,
      executionDirectory: native ? boardDetailsTestDirectory : null,
      nativeConnectionSupported: native,
    );
  }

  Future<void> save(String key, String description, String instructions) async {
    if (key != boardDetailsTestKey || project || saveFails) {
      throw StateError('저장할 수 없습니다');
    }
    this.description = description;
    this.instructions = instructions;
  }

  Future<BoardConnectionPreviewDto> preview(String key) async {
    if (!native) throw StateError('native 연결 없음');
    return BoardConnectionPreviewDto(
      boardKey: key,
      executionDirectory: boardDetailsTestDirectory,
      projectManifestPath: project ? boardDetailsTestManifest : null,
      tools: unresolvedPins
          ? const []
          : const [
              BoardConnectionToolDto(
                toolId: 'git.status',
                description: '현재 작업 상태를 확인한다.',
                defaultsJson: '{"short":true}',
                inputSchemaJson: '{"type":"object"}',
                executionDirectory: boardDetailsTestDirectory,
                readiness: BoardToolReadinessDto.ready,
                readinessReasons: [],
              ),
              BoardConnectionToolDto(
                toolId: 'remote.review',
                description: '원격 검토 요청',
                defaultsJson: '{}',
                inputSchemaJson: '{"type":"object"}',
                readiness: BoardToolReadinessDto.unchecked,
                readinessReasons: ['원격 서비스 상태를 확인하지 않았습니다.'],
              ),
            ],
      configJson: boardDetailsTestConfig,
      readiness: unresolvedPins
          ? BoardToolReadinessDto.unchecked
          : cliAvailable
          ? BoardToolReadinessDto.ready
          : BoardToolReadinessDto.unavailable,
      readinessReasons: unresolvedPins
          ? const ['로드되지 않은 핀: remote.review']
          : const [],
    );
  }
}

class BoardDetailsClipboard extends ClipboardWriter {
  String? text;
  bool fails = false;

  @override
  Future<void> write(String text) async {
    if (fails) throw StateError('클립보드 사용 불가');
    this.text = text;
  }
}
