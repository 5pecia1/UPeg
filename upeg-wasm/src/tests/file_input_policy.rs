use super::*;

fn raw_정책(
    max_count: u32,
    extensions: &[&str],
    max_file_bytes: Option<u64>,
    max_total_bytes: Option<u64>,
) -> PluginFileInputPolicy {
    PluginFileInputPolicy {
        max_count,
        extensions: extensions
            .iter()
            .map(|extension| (*extension).to_string())
            .collect(),
        max_file_bytes,
        max_total_bytes,
    }
}

fn file_필드(policy: Option<PluginFileInputPolicy>) -> PluginInputField {
    let mut field = PluginInputField::required("input", PluginInputKind::File);
    field.file_policy = policy;
    field
}

fn file_정책(kind: &InputKind) -> &upeg_core::FileInputPolicy {
    match kind {
        InputKind::File(policy) => policy,
        other => panic!("File kind를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 플러그인_file_정책은_코어_정책으로_손실없이_변환된다() {
    let spec = PluginInputSpec::new([file_필드(Some(raw_정책(
        3,
        &["png", "tar.gz"],
        Some(5_000_000),
        Some(10_000_000),
    )))]);

    let core = plugin_input_spec_to_core(&spec).expect("유효한 정책을 코어로 낮춘다");
    let policy = file_정책(&core.fields[0].kind);

    assert_eq!(policy.max_count(), 3);
    assert_eq!(policy.extensions().collect::<Vec<_>>(), ["png", "tar.gz"]);
    assert_eq!(policy.max_file_bytes(), Some(5_000_000));
    assert_eq!(policy.max_total_bytes(), Some(10_000_000));
}

#[test]
fn 생략된_플러그인_file_정책은_코어_기본값으로_변환된다() {
    let spec = PluginInputSpec::new([file_필드(None)]);

    let core = plugin_input_spec_to_core(&spec).expect("생략한 정책에 기본값을 적용한다");
    let policy = file_정책(&core.fields[0].kind);

    assert_eq!(policy.max_count(), 1);
    assert_eq!(policy.extensions().count(), 0);
    assert_eq!(policy.max_file_bytes(), None);
    assert_eq!(policy.max_total_bytes(), None);
}

#[test]
fn 플러그인_file_정책은_코어_invariant를_통과하지_못하면_거부된다() {
    let spec = PluginInputSpec::new([file_필드(Some(raw_정책(0, &[], None, None)))]);

    match plugin_input_spec_to_core(&spec) {
        Err(LoadError::InvalidInputSpec { detail }) => {
            assert!(detail.contains("max_count"), "실제 오류: {detail}");
        }
        other => panic!("0인 max_count 거부를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 플러그인_file_max_count도_코어_상한에서_수렴한다() {
    for (max_count, is_valid) in [
        (upeg_core::MAX_FILE_INPUT_COUNT, true),
        (upeg_core::MAX_FILE_INPUT_COUNT + 1, false),
    ] {
        let spec = PluginInputSpec::new([file_필드(Some(raw_정책(max_count, &[], None, None)))]);

        let result = plugin_input_spec_to_core(&spec);

        assert_eq!(
            result.is_ok(),
            is_valid,
            "max_count={max_count}의 Core 수렴 결과가 달라야 한다: {result:?}"
        );
    }
}

#[test]
fn file이_아닌_플러그인_입력의_file_정책은_거부된다() {
    let mut field = PluginInputField::required("input", PluginInputKind::String);
    field.file_policy = Some(raw_정책(1, &[], None, None));
    let spec = PluginInputSpec::new([field]);

    match plugin_input_spec_to_core(&spec) {
        Err(LoadError::InvalidInputSpec { detail }) => {
            assert!(detail.contains("File"), "실제 오류: {detail}");
        }
        other => panic!("File이 아닌 입력의 정책 거부를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 플러그인_file_정책은_json_schema에_확장으로_노출된다() {
    let spec = PluginInputSpec::new([file_필드(Some(raw_정책(
        3,
        &["png", "tar.gz"],
        Some(5_000_000),
        Some(10_000_000),
    )))]);
    let core = plugin_input_spec_to_core(&spec).expect("유효한 정책을 코어로 낮춘다");

    let schema = core.to_json_schema_value();

    assert_eq!(
        schema["properties"]["input"]["x-upeg-file-policy"],
        serde_json::json!({
            "maxCount": 3,
            "extensions": ["png", "tar.gz"],
            "maxFileBytes": 5_000_000,
            "maxTotalBytes": 10_000_000
        })
    );
}

#[test]
fn 플러그인에서_노출한_file_정책_schema는_코어로_다시_가져온다() {
    let spec = PluginInputSpec::new([file_필드(Some(raw_정책(
        3,
        &["png", "tar.gz"],
        Some(5_000_000),
        Some(10_000_000),
    )))]);
    let core = plugin_input_spec_to_core(&spec).expect("유효한 정책을 코어로 낮춘다");
    let schema = core.to_json_schema_value();

    let imported = InputSpec::try_from(&schema).expect("노출한 schema를 다시 가져온다");
    let policy = file_정책(&imported.fields[0].kind);

    assert_eq!(policy.max_count(), 3);
    assert_eq!(policy.extensions().collect::<Vec<_>>(), ["png", "tar.gz"]);
    assert_eq!(policy.max_file_bytes(), Some(5_000_000));
    assert_eq!(policy.max_total_bytes(), Some(10_000_000));
}
