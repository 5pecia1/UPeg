/// Shared fake i18n catalog + Riverpod overrides for widget tests.
///
/// Widget tests never link the native dylib, so any widget that calls
/// `t()` (lib/src/i18n/t.dart) needs [i18nTranslateOverride] /
/// [i18nTranslateArgsOverride] swapped for fakes. Spreading
/// [i18nTestOverrides] into a `ProviderScope.overrides` list does that
/// with a catalog that mirrors the Rust En/Ko entries the widgets under
/// test consume — assertions can keep matching the real English copy,
/// and locale-flip tests can assert the Korean copy from the same map.
///
/// Keep entries in sync with upeg-pegboard-ui/src/i18n.rs when a widget
/// under test starts using a new key. Unknown keys fall back to the key
/// string itself (the same "key as marker" contract as the Rust core
/// fallback chain), so a missing entry shows up in failures as the raw
/// key rather than a crash.
library;

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;

/// En/Ko fixture rows for the catalog keys exercised by widget tests.
const Map<String, Map<LocaleDto, String>> i18nTestCatalog = {
  "media.convert.guidance": {
    LocaleDto.en:
        "Choose an image and output format, then convert and save. SVG becomes pixels. Animated images and TIFF use the first frame/page; WebP is lossless.",
    LocaleDto.ko:
        "이미지와 출력 형식을 선택하고 변환한 뒤 저장하세요. SVG는 픽셀 이미지로 변환합니다. 애니메이션·TIFF는 첫 프레임/페이지만, WebP는 무손실로 변환합니다.",
  },
  "media.convert.advanced": {
    LocaleDto.en: "Advanced settings",
    LocaleDto.ko: "고급 설정",
  },
  "media.convert.ico": {
    LocaleDto.en:
        "ICO supports images up to 256 × 256 pixels. For SVG, set the output width to 256 or less.",
    LocaleDto.ko: "ICO는 최대 256 × 256 픽셀을 지원합니다. SVG는 출력 폭을 256 이하로 설정하세요.",
  },
  "media.convert.input": {LocaleDto.en: "Image", LocaleDto.ko: "이미지"},
  "media.convert.images": {
    LocaleDto.en: "Images (ZIP output)",
    LocaleDto.ko: "이미지 여러 장 (ZIP 출력)",
  },
  "media.convert.format": {
    LocaleDto.en: "Output format",
    LocaleDto.ko: "출력 형식",
  },
  "media.convert.quality": {
    LocaleDto.en: "JPEG quality",
    LocaleDto.ko: "JPEG 품질",
  },
  "media.convert.background": {
    LocaleDto.en: "Background for transparent pixels (#RRGGBB)",
    LocaleDto.ko: "투명 영역의 배경색 (#RRGGBB)",
  },
  "media.convert.svg_width": {
    LocaleDto.en: "SVG width in pixels (0 = original)",
    LocaleDto.ko: "SVG 출력 폭 (픽셀, 0 = 원본)",
  },
  "media.convert.limit": {
    LocaleDto.en: "Maximum output size in bytes",
    LocaleDto.ko: "최대 출력 크기 (바이트)",
  },
  "media.file.preview": {
    LocaleDto.en: "Converted image preview",
    LocaleDto.ko: "변환된 이미지 미리보기",
  },
  "media.file.preview_unavailable": {
    LocaleDto.en: "Preview unavailable. Save the file to view it.",
    LocaleDto.ko: "미리보기를 지원하지 않습니다. 파일을 저장하여 확인하세요.",
  },
  "media.file.save": {LocaleDto.en: "Save", LocaleDto.ko: "저장"},
  "media.file.save_title": {
    LocaleDto.en: "Save output file",
    LocaleDto.ko: "결과 파일 저장",
  },
  "media.file.saving": {LocaleDto.en: "Saving…", LocaleDto.ko: "저장 중…"},
  "media.file.saved": {LocaleDto.en: "Saved.", LocaleDto.ko: "저장했습니다."},
  "media.file.download_started": {
    LocaleDto.en: "Download started. Check your browser downloads.",
    LocaleDto.ko: "다운로드를 시작했습니다. 브라우저 다운로드 목록을 확인하세요.",
  },
  "media.file.save_failed": {
    LocaleDto.en:
        "Could not save the file. Check the destination and try again.",
    LocaleDto.ko: "파일을 저장하지 못했습니다. 저장 위치를 확인하고 다시 시도하세요.",
  },
  "tool.media.image_convert.label": {
    LocaleDto.en: "Convert image",
    LocaleDto.ko: "이미지 변환",
  },
  "tool.media.images_convert.label": {
    LocaleDto.en: "Batch convert images",
    LocaleDto.ko: "이미지 일괄 변환",
  },
  'board.connection.incomplete_tools': {
    LocaleDto.en:
        "No tools are available in this preview yet. Review the readiness details before changing this Board's pins.",
    LocaleDto.ko: '현재 미리보기에서 확인할 수 있는 도구가 없습니다. 보드의 핀을 바꾸기 전에 준비 상태 안내를 확인하세요.',
  },
  'board.details.saved_browser': {
    LocaleDto.en: 'Guidance saved in this browser.',
    LocaleDto.ko: '이 브라우저에 지침을 저장했습니다.',
  },
  'board.details.project_changed': {
    LocaleDto.en:
        "The project file changed: {path}. Restart UPeg to reload its guidance and tools, then reconnect your agent's MCP server.",
    LocaleDto.ko:
        '프로젝트 파일이 변경되었습니다: {path}. UPeg를 다시 시작하여 지침과 도구를 불러온 뒤 에이전트의 MCP 서버를 다시 연결하세요.',
  },
  'board.connection.cli_unavailable': {
    LocaleDto.en:
        'The upeg CLI is not installed or cannot be found. Install it on PATH before using this configuration.',
    LocaleDto.ko:
        'upeg CLI가 설치되어 있지 않거나 찾을 수 없습니다. 설정을 사용하기 전에 PATH에서 실행할 수 있도록 설치하세요.',
  },
  "board.details.open": {LocaleDto.en: "Board details", LocaleDto.ko: "보드 상세"},
  "board.details.title": {
    LocaleDto.en: "Board guidance & agent connection",
    LocaleDto.ko: "보드 지침 및 에이전트 연결",
  },
  "board.details.prepare": {
    LocaleDto.en:
        "Pin the tools this Board needs, then save each pin's default arguments in its tool view.",
    LocaleDto.ko: "이 보드에 필요한 도구를 핀으로 추가하고 각 도구 화면에서 기본 인수를 저장하세요.",
  },
  "board.details.description": {
    LocaleDto.en: "Description (optional)",
    LocaleDto.ko: "설명 (선택 사항)",
  },
  "board.details.instructions": {
    LocaleDto.en: "Instructions · Markdown (optional)",
    LocaleDto.ko: "지침 · Markdown (선택 사항)",
  },
  "board.details.optional": {
    LocaleDto.en:
        "Describe the purpose, order of work and checks the agent should follow. Empty guidance is allowed.",
    LocaleDto.ko: "목적, 작업 순서, 에이전트가 확인할 내용을 적으세요. 비워 두어도 사용할 수 있습니다.",
  },
  "board.details.project_owned": {
    LocaleDto.en:
        "This project's upeg.toml owns this guidance. Edit description and instructions in its [[boards]] entry, restart UPeg, then reconnect your agent's MCP server.",
    LocaleDto.ko:
        "프로젝트의 upeg.toml에서 지침을 관리합니다. 해당 [[boards]] 항목의 description과 instructions를 수정한 뒤 UPeg를 다시 시작하고 에이전트의 MCP 서버를 다시 연결하세요.",
  },
  "board.details.save": {LocaleDto.en: "Save guidance", LocaleDto.ko: "지침 저장"},
  "board.details.saving": {LocaleDto.en: "Saving…", LocaleDto.ko: "저장 중…"},
  "board.details.saved": {
    LocaleDto.en:
        "Guidance saved. Reconnect existing agent sessions to use it.",
    LocaleDto.ko: "지침을 저장했습니다. 기존 에이전트 세션을 다시 연결하면 적용됩니다.",
  },
  "board.details.save_failed": {
    LocaleDto.en: "Could not save guidance: {message}",
    LocaleDto.ko: "지침을 저장하지 못했습니다: {message}",
  },
  "board.details.load_failed": {
    LocaleDto.en: "Could not load this Board: {message}",
    LocaleDto.ko: "보드를 불러오지 못했습니다: {message}",
  },
  "board.details.retry": {LocaleDto.en: "Retry", LocaleDto.ko: "다시 시도"},
  "board.details.close": {LocaleDto.en: "Close", LocaleDto.ko: "닫기"},
  "board.connection.show": {
    LocaleDto.en: "Connect this Board to an agent",
    LocaleDto.ko: "이 보드를 에이전트에 연결",
  },
  "board.connection.directory": {
    LocaleDto.en: "Execution directory",
    LocaleDto.ko: "실행 폴더",
  },
  "board.connection.directory_help": {
    LocaleDto.en:
        "This is the folder upeg was opened from. Open upeg from another project to use that project's tools and Boards.",
    LocaleDto.ko:
        "upeg를 실행한 폴더입니다. 다른 프로젝트의 도구와 보드를 사용하려면 해당 프로젝트에서 upeg를 여세요.",
  },
  "board.connection.web_unavailable": {
    LocaleDto.en:
        "This Board is saved in this browser. Local MCP connections require the desktop app or CLI; this page cannot connect an agent to your computer.",
    LocaleDto.ko:
        "이 보드는 브라우저에 저장됩니다. 로컬 MCP 연결은 데스크톱 앱이나 CLI에서 설정해야 합니다. 이 페이지에서 컴퓨터의 에이전트에 연결할 수는 없습니다.",
  },
  "board.connection.steps": {
    LocaleDto.en:
        "1. Review the tools and defaults below.\n2. Add this MCP configuration to your agent and restart its MCP connection.\n3. Ask the agent to call upeg.board_context and follow its instructions before running tools.",
    LocaleDto.ko:
        "1. 아래 도구와 기본값을 확인하세요.\n2. 에이전트에 MCP 설정을 추가하고 MCP 연결을 다시 시작하세요.\n3. 도구 실행 전에 upeg.board_context 도구를 호출하여 지침을 읽고 따르도록 요청하세요.",
  },
  "board.connection.project": {
    LocaleDto.en: "Project manifest",
    LocaleDto.ko: "프로젝트 설정 파일",
  },
  "board.connection.tools": {
    LocaleDto.en: "Effective MCP tools ({count})",
    LocaleDto.ko: "적용되는 MCP 도구 ({count}개)",
  },
  "board.connection.no_tools": {
    LocaleDto.en:
        "No tools are exposed to MCP on this Board. Add an MCP-capable tool and refresh this preview.",
    LocaleDto.ko: "MCP에 노출되는 도구가 없습니다. MCP를 지원하는 도구를 추가한 뒤 미리보기를 새로 여세요.",
  },
  "board.connection.ready": {LocaleDto.en: "Ready", LocaleDto.ko: "사용 가능"},
  "board.connection.unavailable": {
    LocaleDto.en: "Unavailable",
    LocaleDto.ko: "사용 불가",
  },
  "board.connection.unchecked": {
    LocaleDto.en: "Readiness not verified",
    LocaleDto.ko: "사용 가능 여부 미확인",
  },
  "board.connection.defaults": {
    LocaleDto.en: "Saved default arguments",
    LocaleDto.ko: "저장된 기본 인수",
  },
  "board.connection.schema": {
    LocaleDto.en: "Accepted arguments",
    LocaleDto.ko: "허용되는 인수",
  },
  "board.connection.config": {
    LocaleDto.en: "MCP configuration",
    LocaleDto.ko: "MCP 설정",
  },
  "board.connection.copy": {
    LocaleDto.en: "Copy MCP configuration",
    LocaleDto.ko: "MCP 설정 복사",
  },
  "board.connection.copied": {
    LocaleDto.en: "Configuration copied. Add it to your agent to connect.",
    LocaleDto.ko: "설정을 복사했습니다. 에이전트에 추가하여 연결하세요.",
  },
  "board.connection.copy_failed": {
    LocaleDto.en: "Could not copy configuration: {message}",
    LocaleDto.ko: "설정을 복사하지 못했습니다: {message}",
  },
  "board.connection.load_failed": {
    LocaleDto.en: "Could not preview the connection: {message}",
    LocaleDto.ko: "연결 미리보기를 불러오지 못했습니다: {message}",
  },
  "board.connection.reconnect": {
    LocaleDto.en:
        "After changing tools, defaults or guidance, reconnect the agent to refresh its Board context. This preview does not verify an active connection.",
    LocaleDto.ko:
        "도구, 기본값, 지침을 바꾼 뒤에는 에이전트를 다시 연결해 보드 컨텍스트를 갱신하세요. 이 미리보기는 실제 연결 상태를 확인하지 않습니다.",
  },
  // Shared.
  'common.unknown_tool': {
    LocaleDto.en: 'unknown tool: {tool_id}',
    LocaleDto.ko: '알 수 없는 도구: {tool_id}',
  },
  'common.search_failed': {
    LocaleDto.en: 'search failed: {msg}',
    LocaleDto.ko: '검색 실패: {msg}',
  },

  // Status bar.
  'desktop.status.board_prefix': {LocaleDto.en: 'board ', LocaleDto.ko: '보드 '},
  'desktop.status.pinned_count': {
    LocaleDto.en: ' · {count} pinned',
    LocaleDto.ko: ' · {count}개 핀됨',
  },
  'desktop.status.paused': {LocaleDto.en: 'paused', LocaleDto.ko: '일시정지'},
  'desktop.status.imports': {
    LocaleDto.en: 'imports {count}',
    LocaleDto.ko: '임포트 {count}',
  },
  'desktop.status.imports_loading': {
    LocaleDto.en: 'imports loading…',
    LocaleDto.ko: '임포트 로딩 중…',
  },

  // Board tab bar.
  'desktop.tab.add_board': {LocaleDto.en: '+ board', LocaleDto.ko: '+ 보드'},
  'desktop.tab.add_tool': {LocaleDto.en: '+ pin', LocaleDto.ko: '+ 핀'},
  'desktop.tab.search': {LocaleDto.en: 'search ', LocaleDto.ko: '검색 '},
  'desktop.tab.settings': {LocaleDto.en: 'settings', LocaleDto.ko: '설정'},
  'desktop.tab.pin_color': {LocaleDto.en: 'color', LocaleDto.ko: '색상'},
  'desktop.tab.pin_color_disabled': {
    LocaleDto.en: 'select a pin to edit color',
    LocaleDto.ko: '색상을 바꿀 핀을 선택하세요',
  },
  'desktop.tab.load_failed': {
    LocaleDto.en: 'failed to load boards: {msg}',
    LocaleDto.ko: '보드 목록을 불러오지 못했습니다: {msg}',
  },
  'desktop.tab.menu_rename': {LocaleDto.en: 'Rename…', LocaleDto.ko: '이름 바꾸기…'},
  'desktop.tab.menu_delete': {LocaleDto.en: 'Delete', LocaleDto.ko: '삭제'},
  'desktop.tab.create_prompt': {
    LocaleDto.en: 'New board',
    LocaleDto.ko: '새 보드',
  },
  'desktop.tab.create_confirm': {LocaleDto.en: 'Create', LocaleDto.ko: '만들기'},
  'desktop.tab.new_placeholder': {
    LocaleDto.en: 'Board name',
    LocaleDto.ko: '보드 이름',
  },
  'desktop.tab.rename_prompt_titled': {
    LocaleDto.en: 'Rename "{title}"',
    LocaleDto.ko: '"{title}" 이름 바꾸기',
  },
  'desktop.tab.rename_confirm_title': {
    LocaleDto.en: 'rename',
    LocaleDto.ko: '이름 변경',
  },
  'desktop.tab.remove_confirm': {
    LocaleDto.en: 'Remove board "{title}"?',
    LocaleDto.ko: '보드 "{title}"를 제거할까요?',
  },
  'desktop.tab.remove_confirm_body': {
    LocaleDto.en: 'This drops the board and its pinned tools.',
    LocaleDto.ko: '보드와 핀된 도구가 함께 삭제됩니다.',
  },
  'desktop.tab.remove_confirm_ok': {
    LocaleDto.en: 'Delete board',
    LocaleDto.ko: '삭제',
  },
  'desktop.tab.remove_cancel': {LocaleDto.en: 'Cancel', LocaleDto.ko: '취소'},

  // Empty board card.
  'empty.pegboard.title': {
    LocaleDto.en: 'pegboard is empty',
    LocaleDto.ko: '페그보드가 비어 있습니다',
  },
  'empty.pegboard.hint': {
    LocaleDto.en: 'This board is empty — pin a tool from the palette.',
    LocaleDto.ko: '이 보드는 비어 있습니다 — 팔레트에서 도구를 핀하세요.',
  },
  'empty.pegboard.no_pins': {
    LocaleDto.en: 'no pins on this board yet',
    LocaleDto.ko: '이 보드에는 아직 핀이 없습니다',
  },
  'empty.find_tool_button': {
    LocaleDto.en: 'pin a tool',
    LocaleDto.ko: '도구 핀하기',
  },
  'empty.suggestion_header': {
    LocaleDto.en: 'Suggested starters',
    LocaleDto.ko: '추천 시작 도구',
  },

  // Palette.
  'palette.placeholder': {
    LocaleDto.en: 'search tools, paste, or type a command…',
    LocaleDto.ko: '도구 검색, 붙여넣기 또는 명령 입력…',
  },
  'palette.empty_toolbox': {
    LocaleDto.en: 'toolbox empty.',
    LocaleDto.ko: '툴박스가 비어 있습니다.',
  },
  'palette.no_match': {
    LocaleDto.en: 'no tools match "{needle}"',
    LocaleDto.ko: '"{needle}"와(과) 일치하는 도구가 없습니다',
  },
  'palette.row.pin': {LocaleDto.en: '+ pin', LocaleDto.ko: '+ 핀'},
  'palette.row.pinned': {LocaleDto.en: 'pinned', LocaleDto.ko: '핀됨'},
  'palette.footer.navigate': {LocaleDto.en: 'navigate', LocaleDto.ko: '이동'},
  'palette.footer.open': {LocaleDto.en: 'open', LocaleDto.ko: '열기'},
  'palette.footer.open_pin': {
    LocaleDto.en: 'open + pin',
    LocaleDto.ko: '열기 + 핀',
  },
  'palette.footer.summary': {
    LocaleDto.en: '{count} tools · {total} boards',
    LocaleDto.ko: '도구 {count}개 · 보드 {total}개',
  },

  // Popup.
  'popup.tag': {LocaleDto.en: 'upeg · popup', LocaleDto.ko: 'upeg · 팝업'},
  'popup.open_desktop': {LocaleDto.en: 'open desktop', LocaleDto.ko: '데스크탑 열기'},
  'popup.search_placeholder': {
    LocaleDto.en: 'search tools…',
    LocaleDto.ko: '도구 검색…',
  },
  'popup.no_match': {
    LocaleDto.en: 'no tools match "{needle}"',
    LocaleDto.ko: '"{needle}"와(과) 일치하는 도구가 없습니다',
  },
  'popup.catalogue_failed': {
    LocaleDto.en: 'catalogue failed: {msg}',
    LocaleDto.ko: '카탈로그 로드 실패: {msg}',
  },
  'popup.pinned_section': {
    LocaleDto.en: 'PINNED · {board}',
    LocaleDto.ko: '고정됨 · {board}',
  },
  'popup.result.ok': {LocaleDto.en: 'OK', LocaleDto.ko: '성공'},
  'popup.result.error': {LocaleDto.en: 'ERROR', LocaleDto.ko: '오류'},
  'popup.result.empty_output': {
    LocaleDto.en: '(no output)',
    LocaleDto.ko: '(출력 없음)',
  },
  'popup.result.copy_tooltip': {
    LocaleDto.en: 'copy result [F2]',
    LocaleDto.ko: '결과 복사 [F2]',
  },

  // Embed / controlled embed.
  'embed.url_unavailable': {
    LocaleDto.en: 'embed url not available for {tool_id}',
    LocaleDto.ko: '임베드 URL이 없습니다: {tool_id}',
  },
  'embed.open_externally': {
    LocaleDto.en: 'open externally',
    LocaleDto.ko: '외부 브라우저에서 열기',
  },
  'embed.load_failed_hint': {
    LocaleDto.en: "This page didn't load. Open it in an external browser.",
    LocaleDto.ko: '이 페이지가 로드되지 않습니다. 외부 브라우저에서 여세요.',
  },
  'controlled_embed.inline_run_unsupported': {
    LocaleDto.en:
        "inline Run isn't supported on this platform — open in an external browser",
    LocaleDto.ko: '이 플랫폼에서는 인라인 실행 미지원 — 외부 브라우저에서 열기',
  },
  'controlled_embed.press_run': {
    LocaleDto.en: 'press Run to fetch outputs',
    LocaleDto.ko: '실행을 눌러 출력을 가져오세요',
  },
  'controlled_embed.no_outputs': {
    LocaleDto.en: '(no outputs)',
    LocaleDto.ko: '(출력 없음)',
  },
  'controlled_embed.debug_button': {LocaleDto.en: 'Debug', LocaleDto.ko: '디버그'},
  'controlled_embed.session_unavailable': {
    LocaleDto.en: 'Browser session unavailable: {error}',
    LocaleDto.ko: '브라우저 세션을 사용할 수 없습니다: {error}',
  },

  // Expanded modal.
  'modal.header.close': {LocaleDto.en: 'esc · close', LocaleDto.ko: 'esc · 닫기'},
  'modal.tag.output': {LocaleDto.en: 'OUTPUT', LocaleDto.ko: '출력'},
  'modal.no_output': {LocaleDto.en: '(no output)', LocaleDto.ko: '(출력 없음)'},
  'modal.outcome.ok': {LocaleDto.en: 'ok', LocaleDto.ko: '성공'},
  'modal.outcome.error': {LocaleDto.en: 'error', LocaleDto.ko: '오류'},
  'modal.outcome.details': {LocaleDto.en: 'details', LocaleDto.ko: '상세'},
  'modal.output.primary': {LocaleDto.en: 'primary', LocaleDto.ko: '프라이머리'},
  'modal.pin.no_active_board': {
    LocaleDto.en: 'no active board to pin into',
    LocaleDto.ko: '핀할 활성 보드가 없습니다',
  },
  'modal.pin.failed': {
    LocaleDto.en: 'pin failed: {msg}',
    LocaleDto.ko: '핀 실패: {msg}',
  },
  'modal.pin.success': {
    LocaleDto.en: 'pinned {tool_id} to {board}',
    LocaleDto.ko: '{tool_id}을(를) {board}에 핀했습니다',
  },
  'modal.footer.source_prefix': {
    LocaleDto.en: 'source: ',
    LocaleDto.ko: '출처: ',
  },
  'modal.footer.toolkit_prefix': {
    LocaleDto.en: 'toolkit: ',
    LocaleDto.ko: '툴킷: ',
  },
  'modal.footer.invoker_prefix': {
    LocaleDto.en: 'invoker: ',
    LocaleDto.ko: '인보커: ',
  },
  'modal.action.run_short': {LocaleDto.en: 'Run', LocaleDto.ko: '실행'},
  'modal.action.cancel_run': {
    LocaleDto.en: 'Cancel run',
    LocaleDto.ko: '실행 취소',
  },
  'a11y.inline.running': {
    LocaleDto.en: 'Running tool',
    LocaleDto.ko: '도구 실행 중',
  },
  'inline.dispatch_failed': {
    LocaleDto.en: 'Unable to run this tool.',
    LocaleDto.ko: '이 도구를 실행할 수 없습니다.',
  },
  'inline.run': {LocaleDto.en: 'Run', LocaleDto.ko: '실행'},
  'modal.pill.live_output': {
    LocaleDto.en: 'Live output',
    LocaleDto.ko: '실시간 출력',
  },
  'modal.approval.title': {
    LocaleDto.en: 'Approval required',
    LocaleDto.ko: '승인이 필요합니다',
  },
  'modal.approval.body': {
    LocaleDto.en:
        '{tool} has a step that waits for a person to approve it. '
        'Approve and run it now?',
    LocaleDto.ko: '{tool}에는 사람이 승인해야 넘어가는 단계가 있습니다. 승인하고 지금 실행할까요?',
  },
  'modal.approval.approve': {
    LocaleDto.en: 'Approve & run',
    LocaleDto.ko: '승인하고 실행',
  },
  'modal.approval.cancel': {LocaleDto.en: 'Cancel', LocaleDto.ko: '취소'},
  'modal.approval.denied_body': {
    LocaleDto.en:
        '{tool} only accepts approval from: {surfaces}. This desktop app '
        'is not one of them, so the run would stop at the barrier.',
    LocaleDto.ko:
        '{tool}은(는) {surfaces}의 승인만 인정합니다. 데스크톱 앱은 여기에 없어서 실행이 승인 장벽에서 멈춥니다.',
  },
  'modal.approval.dismiss': {LocaleDto.en: 'Got it', LocaleDto.ko: '알겠습니다'},
  'modal.generic.no_inputs': {
    LocaleDto.en: '(no inputs — this tool runs without arguments)',
    LocaleDto.ko: '(입력 없음 — 인자 없이 실행됩니다)',
  },
  'modal.generic.file_path_suffix': {
    LocaleDto.en: '{label} (file path)',
    LocaleDto.ko: '{label} (파일 경로)',
  },
  'modal.generic.file_pick': {
    LocaleDto.en: 'Choose file',
    LocaleDto.ko: '파일 선택',
  },
  'modal.generic.file_clear': {
    LocaleDto.en: 'clear selection',
    LocaleDto.ko: '선택 지우기',
  },
  // File input field + typed selection failures — mirrors the
  // `modal.file.*` block in upeg-pegboard-ui/src/i18n.rs.
  'modal.file.empty_prompt': {
    LocaleDto.en: 'Select files or\ndrag them here.',
    LocaleDto.ko: '파일을 선택하거나\n여기로 끌어 놓으세요.',
  },
  'modal.file.selected_count': {
    LocaleDto.en: '{count} file(s) selected',
    LocaleDto.ko: '{count}개 파일 선택됨',
  },
  'modal.file.pick_failed': {
    LocaleDto.en: 'Could not select files.',
    LocaleDto.ko: '파일을 선택하지 못했습니다.',
  },
  'modal.file.error.invalid_policy': {
    LocaleDto.en: 'The file selection policy is invalid.',
    LocaleDto.ko: '파일 선택 정책이 올바르지 않습니다.',
  },
  'modal.file.error.empty_selection': {
    LocaleDto.en: 'No files were selected.',
    LocaleDto.ko: '선택된 파일이 없습니다.',
  },
  'modal.file.error.directory_selected': {
    LocaleDto.en: 'Folders cannot be selected: {file}',
    LocaleDto.ko: '폴더는 선택할 수 없습니다: {file}',
  },
  'modal.file.error.extension_not_allowed': {
    LocaleDto.en: 'File type is not allowed: {file}',
    LocaleDto.ko: '허용되지 않는 파일 형식입니다: {file}',
  },
  'modal.file.error.too_many_files': {
    LocaleDto.en: 'You can select at most {max} files.',
    LocaleDto.ko: '파일은 최대 {max}개까지 선택할 수 있습니다.',
  },
  'modal.file.error.too_many_nodes': {
    LocaleDto.en: 'The file structure exceeds the {max}-node limit.',
    LocaleDto.ko: '파일 구조가 최대 {max}개 노드를 초과했습니다.',
  },
  'modal.file.error.metadata_too_large': {
    LocaleDto.en: 'File names and MIME info are too large.',
    LocaleDto.ko: '파일 이름과 MIME 정보가 너무 큽니다.',
  },
  'modal.file.error.file_too_large': {
    LocaleDto.en: 'File exceeds the size limit: {file}',
    LocaleDto.ko: '파일 크기 제한을 초과했습니다: {file}',
  },
  'modal.file.error.total_too_large': {
    LocaleDto.en: 'The total file size exceeds the limit.',
    LocaleDto.ko: '전체 파일 크기 제한을 초과했습니다.',
  },
  'modal.file.error.read_failed': {
    LocaleDto.en: 'Could not read the file: {file}',
    LocaleDto.ko: '파일을 읽지 못했습니다: {file}',
  },
  'modal.hex.hint': {
    LocaleDto.en: 'e.g. ff or 0xCAFE',
    LocaleDto.ko: '예: ff 또는 0xCAFE',
  },
  'modal.hex.empty': {LocaleDto.en: '(empty)', LocaleDto.ko: '(비어 있음)'},
  'modal.hex.invalid': {
    LocaleDto.en: 'not a valid hex value',
    LocaleDto.ko: '올바른 hex 값이 아닙니다',
  },
  'modal.hex.shortcut_hints': {
    LocaleDto.en: '[F1] run · [F2] copy',
    LocaleDto.ko: '[F1] 실행 · [F2] 복사',
  },

  // Generic-form field validation errors — mirrors the
  // `modal.validation.*` block in upeg-pegboard-ui/src/i18n.rs.
  'modal.validation.required': {
    LocaleDto.en: 'required',
    LocaleDto.ko: '필수 입력입니다',
  },
  'modal.validation.invalid_value': {
    LocaleDto.en: 'invalid value',
    LocaleDto.ko: '올바르지 않은 값입니다',
  },
  'modal.validation.not_a_number': {
    LocaleDto.en: 'not a number',
    LocaleDto.ko: '숫자가 아닙니다',
  },
  'modal.validation.not_an_integer': {
    LocaleDto.en: 'not an integer',
    LocaleDto.ko: '정수가 아닙니다',
  },
  'modal.validation.invalid_format': {
    LocaleDto.en: 'invalid format',
    LocaleDto.ko: '형식이 올바르지 않습니다',
  },
  'modal.validation.not_a_url': {
    LocaleDto.en: 'not a url',
    LocaleDto.ko: 'URL이 아닙니다',
  },
  'modal.validation.min': {
    LocaleDto.en: 'min {value}',
    LocaleDto.ko: '최소 {value}',
  },
  'modal.validation.max': {
    LocaleDto.en: 'max {value}',
    LocaleDto.ko: '최대 {value}',
  },
  // Structured-output URL action (expanded_modal/structured_output.dart).
  'modal.output.open_url': {LocaleDto.en: 'open', LocaleDto.ko: '열기'},

  // Pin color dialog.
  'pin_color.title': {LocaleDto.en: 'Pin Color', LocaleDto.ko: '핀 색상'},
  'pin_color.invalid_hex': {
    LocaleDto.en: 'Invalid HEX color',
    LocaleDto.ko: '잘못된 HEX 색상입니다',
  },
  'pin_color.save': {LocaleDto.en: 'Save', LocaleDto.ko: '저장'},
  'pin_color.reset': {LocaleDto.en: 'Reset', LocaleDto.ko: '초기화'},

  // Settings + host attach.
  'settings.close': {LocaleDto.en: 'close', LocaleDto.ko: '닫기'},
  'settings.load_failed': {
    LocaleDto.en: 'failed to load tweaks: {msg}',
    LocaleDto.ko: '설정을 불러오지 못했습니다: {msg}',
  },
  'settings.section.host_attach': {
    LocaleDto.en: 'host attach',
    LocaleDto.ko: '호스트 연결',
  },
  'host_attach.base_url_label': {
    LocaleDto.en: 'Host URL',
    LocaleDto.ko: '호스트 주소',
  },
  'host_attach.token_label': {
    LocaleDto.en: 'Host token',
    LocaleDto.ko: '호스트 토큰',
  },
  'host_attach.check_button': {
    LocaleDto.en: 'Check connection',
    LocaleDto.ko: '연결 확인',
  },
  'host_attach.checking': {LocaleDto.en: 'checking…', LocaleDto.ko: '확인 중…'},
  'host_attach.connected': {LocaleDto.en: 'connected', LocaleDto.ko: '연결됨'},
  'host_attach.unreachable': {
    LocaleDto.en: 'connection failed · check that the daemon is running',
    LocaleDto.ko: '연결 실패 · 데몬이 실행 중인지 확인하세요',
  },
  'host_attach.notice.unauthorized_label': {
    LocaleDto.en: 'authentication failed',
    LocaleDto.ko: '인증 실패',
  },
  'host_attach.notice.unauthorized_hint': {
    LocaleDto.en: 'check the host token in Settings',
    LocaleDto.ko: '설정에서 host 토큰을 확인하세요',
  },
  'host_attach.notice.unreachable_label': {
    LocaleDto.en: 'host connection failed',
    LocaleDto.ko: '호스트 연결 실패',
  },
  'host_attach.notice.unreachable_hint': {
    LocaleDto.en: 'check that the daemon is running',
    LocaleDto.ko: '데몬이 실행 중인지 확인하세요',
  },
  'host_attach.notice.tool_error_label': {
    LocaleDto.en: 'run error',
    LocaleDto.ko: '실행 오류',
  },
  'host_attach.notice.response_too_large_hint': {
    LocaleDto.en: "the host's answer was too large to read",
    LocaleDto.ko: '호스트 응답이 너무 커서 읽을 수 없습니다',
  },
  'host_attach.notice.malformed_response_hint': {
    LocaleDto.en: "the host's answer was malformed",
    LocaleDto.ko: '호스트 응답 형식이 올바르지 않습니다',
  },
  'host_attach.notice.invalid_file_hint': {
    LocaleDto.en: 'the host returned a file this app could not read',
    LocaleDto.ko: '호스트가 읽을 수 없는 파일을 반환했습니다',
  },

  // Backup export / import section (backup_section.dart). The button and
  // failure rows mirror the pre-existing Rust keys; dialog/result rows
  // are new.
  'settings.backup.export': {LocaleDto.en: 'export', LocaleDto.ko: '보내기'},
  'settings.backup.import': {LocaleDto.en: 'import', LocaleDto.ko: '가져오기'},
  'settings.backup.export_dialog_title': {
    LocaleDto.en: 'Save backup',
    LocaleDto.ko: '백업 저장',
  },
  'settings.backup.import_dialog_title': {
    LocaleDto.en: 'Open backup',
    LocaleDto.ko: '백업 열기',
  },
  'settings.backup.exported': {
    LocaleDto.en: 'Saved backup to {path}',
    LocaleDto.ko: '백업을 {path}에 저장했습니다',
  },
  'settings.backup.export_failed': {
    LocaleDto.en: 'export failed: {msg}',
    LocaleDto.ko: '보내기 실패: {msg}',
  },
  'settings.backup.imported': {
    LocaleDto.en: 'Restored {boards} boards, {layouts} layouts, {memos} memos',
    LocaleDto.ko: '보드 {boards}개, 레이아웃 {layouts}개, 메모 {memos}개를 복원했습니다',
  },
  'settings.backup.import_failed': {
    LocaleDto.en: 'import failed: {msg}',
    LocaleDto.ko: '가져오기 실패: {msg}',
  },

  // Surface-unsupported honest state.
  'surface.unsupported.label': {
    LocaleDto.en: 'not supported on this surface',
    LocaleDto.ko: '이 표면에서는 미지원',
  },
  'surface.unsupported.attach_hint': {
    LocaleDto.en: 'can run via host attach — connect a host in Settings',
    LocaleDto.ko: '호스트 연결로 실행 가능 — 설정에서 host를 연결하세요',
  },
  'surface.unsupported.hint.no_process_spawn': {
    LocaleDto.en: 'needs a subprocess — desktop only',
    LocaleDto.ko: '서브프로세스 필요 — 데스크탑 전용',
  },
  'surface.unsupported.hint.no_loader_runtime': {
    LocaleDto.en: 'needs the native runtime — desktop only',
    LocaleDto.ko: '네이티브 런타임 필요 — 데스크탑 전용',
  },
  'surface.unsupported.hint.no_wasm_host': {
    LocaleDto.en: 'needs the wasm plugin host — desktop only',
    LocaleDto.ko: 'wasm 플러그인 호스트 필요 — 데스크탑 전용',
  },
  'surface.unsupported.hint.native_only_tool': {
    LocaleDto.en: 'native-only tool — desktop only',
    LocaleDto.ko: '네이티브 전용 도구 — 데스크탑 전용',
  },

  // Pin context menu (also the CustomSemanticsAction labels).
  'pin.menu.open': {LocaleDto.en: 'open', LocaleDto.ko: '열기'},
  'pin.menu.edit_color': {LocaleDto.en: 'edit color', LocaleDto.ko: '색상 변경'},
  'pin.menu.reset_size': {LocaleDto.en: 'reset size', LocaleDto.ko: '크기 재설정'},
  'pin.menu.unpin': {LocaleDto.en: 'unpin', LocaleDto.ko: '핀 해제'},

  // Pin resize handle (board_canvas.dart).
  'pin.resize.handle_tooltip': {
    LocaleDto.en: 'resize pin',
    LocaleDto.ko: '핀 크기 조절',
  },

  // Keyboard cheatsheet overlay (widgets/cheatsheet_overlay.dart).
  // Mirrors the `keys.*` rows in upeg-pegboard-ui/src/i18n.rs for the
  // catalog slice exercised by widget tests
  // (test_helpers/fake_binding_catalog.dart).
  'keys.title': {LocaleDto.en: 'Keyboard shortcuts', LocaleDto.ko: '키보드 단축키'},
  'keys.footer.close': {LocaleDto.en: 'close', LocaleDto.ko: '닫기'},
  'keys.requires_focus': {
    LocaleDto.en: 'focused pin',
    LocaleDto.ko: '핀 포커스 필요',
  },
  'keys.scope.board': {LocaleDto.en: 'Board', LocaleDto.ko: '보드'},
  'keys.scope.resize': {LocaleDto.en: 'Resize pin', LocaleDto.ko: '핀 크기 조절'},
  'keys.cmd.run': {LocaleDto.en: 'Run', LocaleDto.ko: '실행'},
  'keys.cmd.open': {LocaleDto.en: 'Open / Inspect', LocaleDto.ko: '열기 / 상세 보기'},
  'keys.cmd.search': {LocaleDto.en: 'Search', LocaleDto.ko: '검색'},
  'keys.cmd.switch_board': {
    LocaleDto.en: 'Switch to board slot',
    LocaleDto.ko: '보드 슬롯 전환',
  },
  'keys.cmd.toggle_pin': {
    LocaleDto.en: 'Toggle pin',
    LocaleDto.ko: '핀 고정 / 해제',
  },
  'keys.cmd.edit_pin_color': {
    LocaleDto.en: 'Edit pin color',
    LocaleDto.ko: '핀 색상 편집',
  },
  'keys.cmd.start_resize': {
    LocaleDto.en: 'Resize pin',
    LocaleDto.ko: '핀 크기 조절',
  },
  'keys.cmd.cheatsheet': {
    LocaleDto.en: 'Keyboard cheatsheet',
    LocaleDto.ko: '키보드 치트시트',
  },
  'keys.cmd.reset_span': {
    LocaleDto.en: 'Reset to manifest footprint',
    LocaleDto.ko: '기본 크기로 재설정',
  },
  'keys.cmd.cancel': {LocaleDto.en: 'Cancel', LocaleDto.ko: '취소'},

  // Pin accessibility semantics (label/value/hint).
  'a11y.pin.label': {
    LocaleDto.en: '{name} · {kind} pin',
    LocaleDto.ko: '{name} · {kind} 핀',
  },
  'a11y.pin.label_plain': {
    LocaleDto.en: '{name} pin',
    LocaleDto.ko: '{name} 핀',
  },
  'a11y.pin.hint_run': {LocaleDto.en: 'tap to run', LocaleDto.ko: '탭하면 실행'},
  'a11y.pin.running': {LocaleDto.en: 'running', LocaleDto.ko: '실행 중'},
  'a11y.pin.stale': {LocaleDto.en: 'stale result', LocaleDto.ko: '오래된 결과'},
  'a11y.pin.result_ok': {
    LocaleDto.en: 'OK · {preview}',
    LocaleDto.ko: '성공 · {preview}',
  },
  'a11y.pin.result_ok_empty': {LocaleDto.en: 'OK', LocaleDto.ko: '성공'},
  'a11y.pin.result_error': {
    LocaleDto.en: 'ERROR · {preview}',
    LocaleDto.ko: '오류 · {preview}',
  },
  'a11y.pin.result_error_empty': {LocaleDto.en: 'ERROR', LocaleDto.ko: '오류'},
  'a11y.pin.restored': {
    LocaleDto.en: 'restored result',
    LocaleDto.ko: '복원된 결과',
  },

  // Restored last-run timestamp badge (pin.dart).
  'pin.last_run.just_now': {
    LocaleDto.en: 'last run · just now',
    LocaleDto.ko: '지난 실행 · 방금',
  },
  'pin.last_run.minutes_ago': {
    LocaleDto.en: 'last run · {minutes}m ago',
    LocaleDto.ko: '지난 실행 · {minutes}분 전',
  },
  'pin.last_run.hours_ago': {
    LocaleDto.en: 'last run · {hours}h ago',
    LocaleDto.ko: '지난 실행 · {hours}시간 전',
  },
  'pin.last_run.days_ago': {
    LocaleDto.en: 'last run · {days}d ago',
    LocaleDto.ko: '지난 실행 · {days}일 전',
  },

  // Provider-not-configured honest state.
  'pin.provider_not_configured.label': {
    LocaleDto.en: 'needs setup',
    LocaleDto.ko: '설정 필요',
  },
  'pin.provider_not_configured.hint': {
    LocaleDto.en: 'provider not configured',
    LocaleDto.ko: '프로바이더가 설정되지 않았습니다',
  },
  'pin.provider_not_configured.cli_hint': {
    LocaleDto.en: 'Set up a credential in the terminal:',
    LocaleDto.ko: '터미널에서 credential을 설정하세요:',
  },
  'pin.provider_not_configured.copy_tooltip': {
    LocaleDto.en: 'copy command',
    LocaleDto.ko: '명령 복사',
  },

  // Provider-not-configured message (tool_roles.dart key).
  'pin.provider_not_configured.message': {
    LocaleDto.en: 'needs setup (provider not configured)',
    LocaleDto.ko: '설정 필요 (프로바이더가 설정되지 않았습니다)',
  },
};

/// `translate` fake — catalog lookup with En fallback, then key-as-marker.
String i18nTestTranslate(String key, LocaleDto locale) {
  final row = i18nTestCatalog[key];
  if (row == null) return key;
  return row[locale] ?? row[LocaleDto.en] ?? key;
}

/// `translateArgs` fake — same lookup + `{name}` substitution.
String i18nTestTranslateArgs(
  String key,
  LocaleDto locale,
  List<String> argKeys,
  List<String> argVals,
) {
  var out = i18nTestTranslate(key, locale);
  for (var i = 0; i < argKeys.length; i++) {
    out = out.replaceAll('{${argKeys[i]}}', argVals[i]);
  }
  return out;
}

/// Spread into `ProviderScope(overrides: [...i18nTestOverrides])`.
/// (Type left inferred — Riverpod 3 only exports the `Override` type
/// from its `misc.dart` library.)
final i18nTestOverrides = [
  i18nTranslateOverride.overrideWithValue(i18nTestTranslate),
  i18nTranslateArgsOverride.overrideWithValue(i18nTestTranslateArgs),
];

/// Convenience: resolve the En copy for `key` (with optional args), so
/// tests assert against the catalog row instead of a re-typed literal.
String i18nEn(String key, [Map<String, String>? args]) =>
    _resolve(key, LocaleDto.en, args);

/// Convenience: resolve the Ko copy for `key` (with optional args).
String i18nKo(String key, [Map<String, String>? args]) =>
    _resolve(key, LocaleDto.ko, args);

String _resolve(String key, LocaleDto locale, Map<String, String>? args) {
  if (args == null || args.isEmpty) return i18nTestTranslate(key, locale);
  return i18nTestTranslateArgs(
    key,
    locale,
    args.keys.toList(growable: false),
    args.values.toList(growable: false),
  );
}
