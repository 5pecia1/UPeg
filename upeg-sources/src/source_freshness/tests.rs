use super::*;

fn fixture(label: &str) -> (PathBuf, RuntimeSourceConfig) {
    let root = std::env::temp_dir().join(format!(
        "upeg-source-snapshot-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let config = RuntimeSourceConfig {
        toolkits_dir: Some(root.join("toolkits")),
        project_manifest: None,
        wasm_dir: None,
        mcp_import_dir: Some(root.join("imports")),
    };
    (root, config)
}

#[test]
fn source_snapshot_detects_file_change_addition_and_deletion() {
    let (root, config) = fixture("mutations");
    let directory = config.toolkits_dir.as_ref().expect("toolkits path");
    std::fs::create_dir_all(directory).expect("directory");
    let path = directory.join("tool.toml");
    std::fs::write(&path, "id = 'initial'\n").expect("declaration");
    record_sources_for_root(&root, &config);
    assert!(validate_sources_for_root(&root, &config).is_ok());
    std::fs::write(directory.join("notes.md"), "notes only change").expect("unrelated file");
    assert!(validate_sources_for_root(&root, &config).is_ok());

    for content in [Some("id = 'changed'\n"), None, Some("id = 'added'\n")] {
        match content {
            Some(content) => std::fs::write(&path, content).expect("edit declaration"),
            None => std::fs::remove_file(&path).expect("delete declaration"),
        }
        assert!(matches!(
            validate_sources_for_root(&root, &config),
            Err(LoadedSourcesError::Changed { .. })
        ));
        record_sources_for_root(&root, &config);
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_snapshot_tracks_config_and_each_toolkit_in_marker_directory() {
    let (root, mut config) = fixture("project-directory");
    let marker = root.join("project/.upeg");
    std::fs::create_dir_all(marker.join("toolkits")).expect("project marker");
    config.project_manifest = Some(crate::project::ProjectManifestLookup {
        path: marker.clone(),
        origin: crate::project::ProjectManifestOrigin::Detected,
    });
    record_sources_for_root(&root, &config);
    assert!(validate_sources_for_root(&root, &config).is_ok());
    std::fs::write(marker.join("project.toml"), "schema_version = 1\n").expect("project config");
    assert!(matches!(
        validate_sources_for_root(&root, &config),
        Err(LoadedSourcesError::Changed { .. })
    ));
    record_sources_for_root(&root, &config);
    let toolkit = marker.join("toolkits/check.toml");
    std::fs::write(&toolkit, "id = 'check'\n").expect("project toolkit");
    assert!(matches!(
        validate_sources_for_root(&root, &config),
        Err(LoadedSourcesError::Changed { .. })
    ));
    record_sources_for_root(&root, &config);
    std::fs::remove_file(toolkit).expect("remove toolkit");
    assert!(matches!(
        validate_sources_for_root(&root, &config),
        Err(LoadedSourcesError::Changed { .. })
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_snapshots_do_not_interfere_across_config_roots() {
    let (first_root, first) = fixture("first-root");
    let (second_root, second) = fixture("second-root");
    record_sources_for_root(&first_root, &first);
    assert!(validate_sources_for_root(&second_root, &second).is_ok());
    record_sources_for_root(&second_root, &second);
    let directory = first.toolkits_dir.as_ref().expect("toolkits path");
    std::fs::create_dir_all(directory).expect("directory");
    std::fs::write(directory.join("new.toml"), "id = 'new'\n").expect("new declaration");

    assert!(validate_sources_for_root(&first_root, &first).is_err());
    assert!(validate_sources_for_root(&second_root, &second).is_ok());
    let _ = std::fs::remove_dir_all(first_root);
}

#[test]
fn lazy_import_load_does_not_overwrite_the_boot_snapshot() {
    let (root, config) = fixture("deferred-import");
    record_sources_for_root(&root, &config);
    let directory = config.mcp_import_dir.as_ref().expect("import path");
    std::fs::create_dir_all(directory).expect("directory");
    std::fs::write(directory.join("invalid.toml"), "invalid = [").expect("invalid declaration");
    crate::load_mcp_imports_for_host(&config);

    assert!(matches!(
        validate_sources_for_root(&root, &config),
        Err(LoadedSourcesError::Changed { .. })
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn changed_source_dir_config_requires_restart() {
    let (root, config) = fixture("changed-path");
    record_sources_for_root(&root, &config);
    let mut changed = config;
    changed.toolkits_dir = Some(root.join("other-toolkits"));

    assert!(matches!(
        validate_sources_for_root(&root, &changed),
        Err(LoadedSourcesError::Changed { .. })
    ));
}

#[test]
fn unreadable_source_is_not_treated_as_ready_config() {
    let (root, config) = fixture("unreadable-source");
    let path = config
        .toolkits_dir
        .as_ref()
        .expect("toolkits path")
        .join("directory.toml");
    std::fs::create_dir_all(&path).expect("directory in place of declaration");
    record_sources_for_root(&root, &config);
    assert!(matches!(
        validate_sources_for_root(&root, &config),
        Err(LoadedSourcesError::Unavailable { .. })
    ));
    std::fs::remove_dir_all(&path).expect("clear read error");
    assert!(matches!(
        validate_sources_for_root(&root, &config),
        Err(LoadedSourcesError::Unavailable { .. })
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(feature = "wasm-plugin")]
#[test]
fn wasm_plugin_change_is_detected_as_source_change() {
    let (root, mut config) = fixture("wasm-source");
    let directory = root.join("wasm");
    std::fs::create_dir_all(&directory).expect("wasm directory");
    let path = directory.join("tool.wasm");
    std::fs::write(&path, b"before").expect("existing binary");
    config.wasm_dir = Some(directory);
    record_sources_for_root(&root, &config);
    std::fs::write(&path, b"after").expect("binary change");

    assert!(matches!(
        validate_sources_for_root(&root, &config),
        Err(LoadedSourcesError::Changed { .. })
    ));
    let _ = std::fs::remove_dir_all(root);
}
