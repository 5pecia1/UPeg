import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;

/// Mirrors shared UI feedback strings from the Rust En/Ko catalog.
const Map<String, Map<LocaleDto, String>> uiWorkflowTestCatalog = {
  'modal.pin.failed_generic': {
    LocaleDto.en: 'Could not pin tool. Try again.',
    LocaleDto.ko: '도구를 핀으로 추가하지 못했습니다. 다시 시도하세요.',
  },
  'pin.state.load_failed': {
    LocaleDto.en: 'Saved state could not be loaded. Retry.',
    LocaleDto.ko: '저장된 상태를 불러오지 못했습니다. 다시 시도하세요.',
  },
  'pin.state.save_failed': {
    LocaleDto.en: 'Changes could not be saved. Retry.',
    LocaleDto.ko: '변경 내용을 저장하지 못했습니다. 다시 시도하세요.',
  },
  'pin.state.retry': {LocaleDto.en: 'Retry save', LocaleDto.ko: '저장 재시도'},
  'modal.outcome.stale': {
    LocaleDto.en: 'Result from previous input · run again',
    LocaleDto.ko: '이전 입력의 결과 · 다시 실행하세요',
  },
  'modal.outcome.input_unknown': {
    LocaleDto.en: 'Input used for this result is unavailable',
    LocaleDto.ko: '이 결과를 만든 입력 정보가 없습니다',
  },
  'modal.outcome.restored': {
    LocaleDto.en: 'Saved result',
    LocaleDto.ko: '저장된 결과',
  },
  'modal.outcome.truncated': {
    LocaleDto.en: 'Saved result is incomplete',
    LocaleDto.ko: '저장된 결과의 일부만 표시됩니다',
  },
  'modal.action.cancel_requested': {
    LocaleDto.en: 'Cancel requested…',
    LocaleDto.ko: '취소 요청 중…',
  },
  'desktop.tab.create_failed': {
    LocaleDto.en: 'Could not create board. Try again.',
    LocaleDto.ko: '보드를 만들지 못했습니다. 다시 시도하세요.',
  },
  'desktop.tab.rename_failed': {
    LocaleDto.en: 'Could not rename board. Try again.',
    LocaleDto.ko: '보드 이름을 바꾸지 못했습니다. 다시 시도하세요.',
  },
  'desktop.tab.delete_failed': {
    LocaleDto.en: 'Could not delete board. Try again.',
    LocaleDto.ko: '보드를 삭제하지 못했습니다. 다시 시도하세요.',
  },
  'desktop.tab.more': {LocaleDto.en: 'More actions', LocaleDto.ko: '더보기'},
  'palette.tag_filter': {
    LocaleDto.en: 'Tag filter: {tag}',
    LocaleDto.ko: '태그 필터: {tag}',
  },
  'palette.clear_tag_filter': {
    LocaleDto.en: 'Show all tools',
    LocaleDto.ko: '모든 도구 보기',
  },
  'palette.search_current_tag': {
    LocaleDto.en: 'Search current tag: {tag}',
    LocaleDto.ko: '현재 태그에서 검색: {tag}',
  },
  'a11y.pin.hint_open': {
    LocaleDto.en: 'tap to open full view',
    LocaleDto.ko: '탭하면 전체 보기 열기',
  },
  'pin.result_saved.just_now': {
    LocaleDto.en: 'result saved · just now',
    LocaleDto.ko: '결과 저장 · 방금',
  },
  'pin.result_saved.minutes_ago': {
    LocaleDto.en: 'result saved · {minutes}m ago',
    LocaleDto.ko: '결과 저장 · {minutes}분 전',
  },
  'pin.result_saved.hours_ago': {
    LocaleDto.en: 'result saved · {hours}h ago',
    LocaleDto.ko: '결과 저장 · {hours}시간 전',
  },
  'pin.result_saved.days_ago': {
    LocaleDto.en: 'result saved · {days}d ago',
    LocaleDto.ko: '결과 저장 · {days}일 전',
  },
  'clipboard.copy': {LocaleDto.en: 'Copy', LocaleDto.ko: '복사'},
  'clipboard.copied': {LocaleDto.en: 'Copied', LocaleDto.ko: '복사했습니다'},
  'clipboard.copy_failed': {
    LocaleDto.en: 'Could not copy. Try again.',
    LocaleDto.ko: '복사하지 못했습니다. 다시 시도하세요.',
  },
  'clipboard.no_text_result': {
    LocaleDto.en:
        'No text result to copy. Run the tool or use its file action.',
    LocaleDto.ko: '복사할 텍스트 결과가 없습니다. 도구를 실행하거나 파일 동작을 사용하세요.',
  },
  'clipboard.copied_previous_result': {
    LocaleDto.en: 'Copied previous result',
    LocaleDto.ko: '이전 실행 결과를 복사했습니다',
  },
  'clipboard.copied_input_unknown': {
    LocaleDto.en: 'Copied result; its input history is unavailable',
    LocaleDto.ko: '결과를 복사했습니다. 실행 당시 입력 정보는 없습니다',
  },
  'inline.copy_result': {LocaleDto.en: 'Copy result', LocaleDto.ko: '결과 복사'},
  'inline.copy_previous_result': {
    LocaleDto.en: 'Copy previous result',
    LocaleDto.ko: '이전 결과 복사',
  },
  'inline.result.current_short': {LocaleDto.en: 'current', LocaleDto.ko: '현재'},
  'inline.result.previous_short': {
    LocaleDto.en: 'previous',
    LocaleDto.ko: '이전',
  },
  'inline.result.unknown_short': {
    LocaleDto.en: 'input unknown',
    LocaleDto.ko: '입력 미상',
  },
  'modal.pin.add_new': {
    LocaleDto.en: 'Add as new pin',
    LocaleDto.ko: '새 핀으로 추가',
  },
  'modal.pin.add_to_board': {
    LocaleDto.en: 'Add to board',
    LocaleDto.ko: '보드에 추가',
  },
  'modal.pin.copied_and_editing': {
    LocaleDto.en:
        'New pin added with current inputs and result. Now editing the new pin.',
    LocaleDto.ko: '현재 입력과 결과를 복사해 새 핀을 추가했습니다. 이제 새 핀을 편집합니다.',
  },
  'modal.pin.input_copied_and_editing': {
    LocaleDto.en: 'New pin added with current inputs. Now editing the new pin.',
    LocaleDto.ko: '현재 입력을 복사해 새 핀을 추가했습니다. 이제 새 핀을 편집합니다.',
  },
  'modal.pin.added_and_editing': {
    LocaleDto.en: 'Added to board. Now editing the new pin.',
    LocaleDto.ko: '보드에 추가했습니다. 이제 새 핀을 편집합니다.',
  },
  'modal.hex.preview_label': {
    LocaleDto.en: 'decimal preview',
    LocaleDto.ko: '십진수 미리보기',
  },
  // Native MCP/HTTP status.
  'desktop.status.network_off': {
    LocaleDto.en: 'MCP/HTTP: off',
    LocaleDto.ko: 'MCP/HTTP: 꺼짐',
  },
  'desktop.status.network_loopback': {
    LocaleDto.en: 'MCP/HTTP: local',
    LocaleDto.ko: 'MCP/HTTP: 로컬 전용',
  },
  'desktop.status.network_non_loopback': {
    LocaleDto.en: 'MCP/HTTP: non-local possible',
    LocaleDto.ko: 'MCP/HTTP: 외부 주소 가능',
  },
  'desktop.status.network_off_help': {
    LocaleDto.en:
        'No active MCP/HTTP interface in this app or a discovered host. This does not indicate internet or Host attach status.',
    LocaleDto.ko:
        '이 앱이나 발견된 호스트에서 활성화된 MCP/HTTP 인터페이스가 없습니다. 인터넷이나 원격 Host attach 상태는 뜻하지 않습니다.',
  },
  'desktop.status.network_loopback_help': {
    LocaleDto.en:
        'This app or a discovered host has an MCP/HTTP interface limited to loopback. This does not indicate internet or Host attach status.',
    LocaleDto.ko:
        '이 앱이나 발견된 호스트의 MCP/HTTP 인터페이스가 로컬 주소로 제한됩니다. 인터넷이나 원격 Host attach 상태는 뜻하지 않습니다.',
  },
  'desktop.status.network_non_loopback_help': {
    LocaleDto.en:
        'This app permits a non-loopback MCP/HTTP bind, or a discovered host may use one. External access and Host attach are not verified.',
    LocaleDto.ko:
        '이 앱에서 외부 주소 바인딩을 허용하거나 발견된 호스트가 외부 주소를 사용할 수 있습니다. 외부 접속 가능 여부나 원격 Host attach 상태는 확인하지 않았습니다.',
  },
  // Hex copy guidance.
  'modal.hex.shortcut_hints': {
    LocaleDto.en: '[F1] run',
    LocaleDto.ko: '[F1] 실행',
  },
  'modal.hex.output_copy_hint': {
    LocaleDto.en: '[F2] copy OUTPUT result',
    LocaleDto.ko: '[F2] OUTPUT 결과 복사',
  },
};
