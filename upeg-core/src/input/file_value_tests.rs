use super::*;

fn 바이트_파일() -> FileValue {
    FileValue {
        name: "a.png".to_string(),
        mime: Some("image/png".to_string()),
        content: FileContent::Bytes(vec![0, 1, 255]),
    }
}

fn 디렉터리(entries: Vec<FileValue>) -> FileValue {
    FileValue {
        name: "dir".to_string(),
        mime: None,
        content: FileContent::Directory(entries),
    }
}

/// Nest `depth` file nodes on one root-to-leaf path (the root counts as 1).
fn 중첩_트리(depth: usize) -> FileValue {
    let mut node = 바이트_파일();
    for _ in 1..depth {
        node = 디렉터리(vec![node]);
    }
    node
}

#[test]
fn serde_인코딩과_수동_인코딩이_바이트_단위로_같다() {
    for file in [
        바이트_파일(),
        디렉터리(vec![바이트_파일(), 디렉터리(vec![])]),
    ] {
        let via_serde = serde_json::to_value(&file).expect("serde encodes FileValue");
        let via_manual = file_value_to_json(file);
        assert_eq!(via_serde, via_manual);
        assert_eq!(
            serde_json::to_string(&via_serde).expect("serde value re-encodes"),
            serde_json::to_string(&via_manual).expect("manual value re-encodes"),
        );
    }
}

#[test]
fn 바이트_본문은_패딩된_standard_base64_문자열로_인코딩한다() {
    // Given
    let file = 바이트_파일();

    // When
    let via_serde = serde_json::to_value(&file).expect("FileValue를 직렬화해야 한다");
    let via_manual = file_value_to_json(file);

    // Then
    assert_eq!(via_serde["content"]["bytes"], "AAH/");
    assert_eq!(via_manual["content"]["bytes"], "AAH/");
}

#[test]
fn 빈_바이트_본문은_빈_base64_문자열로_인코딩한다() {
    // Given
    let file = FileValue {
        content: FileContent::Bytes(Vec::new()),
        ..바이트_파일()
    };

    // When
    let json = serde_json::to_value(file).expect("FileValue를 직렬화해야 한다");

    // Then
    assert_eq!(json["content"]["bytes"], "");
}

#[test]
fn 패딩된_standard_base64_문자열은_바이트로_디코딩한다() {
    // Given
    let json = r#"{"name":"a","is_dir":false,"content":{"kind":"bytes","bytes":"AP8="}}"#;

    // When
    let file = serde_json::from_str::<FileValue>(json).expect("정규 base64를 디코딩해야 한다");

    // Then
    assert_eq!(file.content, FileContent::Bytes(vec![0, 255]));
}

#[test]
fn legacy_바이트_배열은_거부한다() {
    // Given
    let json = r#"{"name":"a","is_dir":false,"content":{"kind":"bytes","bytes":[0,255]}}"#;

    // When
    let result = serde_json::from_str::<FileValue>(json);

    // Then
    result.expect_err("legacy 바이트 배열을 거부해야 한다");
}

#[test]
fn 비정규_base64_문자열은_모두_거부한다() {
    const 비정규_본문: &[&str] = &["Z g==", "-w==", "Zg", "Zg===", "A===", "Zh=="];

    for bytes in 비정규_본문 {
        // Given
        let json = format!(
            r#"{{"name":"a","is_dir":false,"content":{{"kind":"bytes","bytes":"{bytes}"}}}}"#
        );

        // When
        let result = serde_json::from_str::<FileValue>(&json);

        // Then
        result.expect_err("비정규 base64를 거부해야 한다");
    }
}

#[test]
fn is_dir은_본문_variant를_그대로_따라간다() {
    assert!(!바이트_파일().is_dir());
    assert!(디렉터리(vec![]).is_dir());
}

#[test]
fn 키_순서는_name_is_dir_mime_content_이다() {
    let json = serde_json::to_string(&바이트_파일()).expect("serde encodes FileValue");
    assert!(
        json.starts_with(r#"{"name":"a.png","is_dir":false,"mime":"image/png","content":"#),
        "unexpected key order: {json}"
    );
}

#[test]
fn 라운드트립하면_원래_값이_그대로_돌아온다() {
    for file in [바이트_파일(), 디렉터리(vec![바이트_파일()]), 중첩_트리(8)] {
        let json = serde_json::to_string(&file).expect("serde encodes FileValue");
        let decoded: FileValue = serde_json::from_str(&json).expect("serde decodes FileValue");
        assert_eq!(decoded, file);
    }
}

#[test]
fn mime이_없으면_키_자체가_사라진다() {
    let file = FileValue {
        mime: None,
        ..바이트_파일()
    };
    let json = serde_json::to_value(&file).expect("serde encodes FileValue");
    assert_eq!(json, file_value_to_json(file));
    assert!(json.get(FILE_KEY_MIME).is_none());
}

#[test]
fn is_dir이_true인데_bytes_본문이면_거부한다() {
    let json = r#"{"name":"a","is_dir":true,"content":{"kind":"bytes","bytes":""}}"#;
    let error = serde_json::from_str::<FileValue>(json).expect_err("inconsistent is_dir");
    assert!(error.to_string().contains(FILE_KEY_IS_DIR), "{error}");
}

#[test]
fn is_dir이_false인데_directory_본문이면_거부한다() {
    let json = r#"{"name":"a","is_dir":false,"content":{"kind":"directory","entries":[]}}"#;
    let error = serde_json::from_str::<FileValue>(json).expect_err("inconsistent is_dir");
    assert!(error.to_string().contains(FILE_KEY_IS_DIR), "{error}");
}

#[test]
fn 모르는_키가_있으면_거부한다() {
    let json = r#"{"name":"a","is_dir":false,"content":{"kind":"bytes","bytes":""},"x":1}"#;
    serde_json::from_str::<FileValue>(json).expect_err("unknown FileValue key");

    let json = r#"{"name":"a","is_dir":false,"content":{"kind":"bytes","bytes":"","x":1}}"#;
    serde_json::from_str::<FileValue>(json).expect_err("unknown FileContent key");
}

#[test]
fn base64_문자열이_아닌_본문은_거부한다() {
    for bytes in ["256", "-1", "null", "true"] {
        let json = format!(
            r#"{{"name":"a","is_dir":false,"content":{{"kind":"bytes","bytes":{bytes}}}}}"#
        );
        serde_json::from_str::<FileValue>(&json).expect_err("문자열이 아닌 본문을 거부해야 한다");
    }
}

// The depth guard is exercised through `from_value`, not `from_str`:
// `from_str` applies serde_json's own 128-*JSON*-level recursion limit,
// and one file level costs three JSON levels, so it rejects deep trees
// before this guard ever sees them. `from_value` has no such limit, which
// is precisely why the guard has to exist.
#[test]
fn 최대_깊이까지는_디코딩되고_한_단계_더_깊으면_거부한다() {
    let 허용 = file_value_to_json(중첩_트리(MAX_FILE_NESTING_DEPTH));
    serde_json::from_value::<FileValue>(허용).expect("max depth decodes");

    let 초과 = file_value_to_json(중첩_트리(MAX_FILE_NESTING_DEPTH + 1));
    let error = serde_json::from_value::<FileValue>(초과).expect_err("over max depth");
    assert!(error.to_string().contains("depth"), "{error}");
}

#[test]
fn 깊이_초과로_실패해도_다음_디코딩에_영향을_주지_않는다() {
    let 초과 = file_value_to_json(중첩_트리(MAX_FILE_NESTING_DEPTH + 1));
    serde_json::from_value::<FileValue>(초과).expect_err("over max depth");

    // The thread-local level counter must have unwound; otherwise this
    // shallow decode would inherit the failed run's depth.
    let json = file_value_to_json(바이트_파일());
    serde_json::from_value::<FileValue>(json).expect("counter unwound after failure");
}
