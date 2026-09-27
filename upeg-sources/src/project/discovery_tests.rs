#[cfg(test)]
use super::*;

mod tests {
    use super::*;

    /// Builds `root/home` (a stand-in `$HOME`) and returns `(root, home)`
    /// after clearing any leftovers from a previous run.
    fn temp_home_tree(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(name);
        let home = root.join("home");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&home).unwrap();
        (root, home)
    }

    fn write_manifest(dir: &Path, _id: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::create_dir_all(dir.join(PROJECT_MANIFEST_FILE)).unwrap();
    }

    #[test]
    fn project_manifest_detection_walks_up_to_nearest_ancestor() {
        let (root, home) = temp_home_tree("upeg_project_detect_parent");
        let nested = home.join("a/b/c");
        std::fs::create_dir_all(&nested).unwrap();
        write_manifest(&home, "project");

        let got = detect_project_manifest_from(&nested, Some(&home)).expect("manifest");
        assert_eq!(got, home.join(PROJECT_MANIFEST_FILE));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_detection_outside_home_has_no_unrelated_home_fallback() {
        let root = std::env::temp_dir().join("upeg_project_detect_home_root");
        let cwd = root.join("work/outside");
        let home = root.join("home");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&cwd).unwrap();
        write_manifest(&home, "home");

        let got = detect_project_manifest_from(&cwd, Some(&home));
        assert_eq!(got, None);

        let _ = std::fs::remove_dir_all(&root);
    }

    /// B-4 repro: an ancestor `.upeg` sitting OUTSIDE `$HOME` must
    /// never be auto-loaded just because cwd is a descendant of it —
    /// e.g. `cd /tmp/evilroot/sub` must not pick up
    /// `/tmp/evilroot/.upeg` when `$HOME` is somewhere else
    /// entirely.
    #[test]
    fn cwd_outside_home_does_not_load_ancestor_manifest() {
        let (root, home) = temp_home_tree("upeg_project_detect_outside_home");
        let evil_root = root.join("evilroot");
        let cwd = evil_root.join("sub");
        std::fs::create_dir_all(&cwd).unwrap();
        write_manifest(&evil_root, "evil"); // ancestor of cwd, outside $HOME
        // $HOME has no manifest of its own.

        let got = detect_project_manifest_from(&cwd, Some(&home));
        assert_eq!(
            got, None,
            ".upeg in an ancestor directory outside home must be ignored"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Sibling to the outside-$HOME case: cwd itself is still always
    /// checked even when it sits outside $HOME.
    #[test]
    fn cwd_manifest_is_loaded_even_outside_home() {
        let (root, home) = temp_home_tree("upeg_project_detect_outside_home_cwd_itself");
        let cwd = root.join("elsewhere");
        write_manifest(&cwd, "elsewhere");

        let got = detect_project_manifest_from(&cwd, Some(&home)).expect("manifest");
        assert_eq!(got, cwd.join(PROJECT_MANIFEST_FILE));

        let _ = std::fs::remove_dir_all(&root);
    }

    /// B-4 hole at the env boundary: `HOME=""` (what `var_os` returns
    /// for an exported-but-empty `HOME`) and `HOME=/` both make
    /// `starts_with` true for every path, which would turn the whole
    /// filesystem into "inside $HOME" and re-open the outside-$HOME
    /// ancestor walk. A relative `HOME` is unusable for the same
    /// reason. All three must behave exactly like "no home".
    #[test]
    fn degenerate_home_value_is_treated_as_no_home() {
        let (root, _home) = temp_home_tree("upeg_project_detect_degenerate_home");
        let evil_root = root.join("evilroot");
        let cwd = evil_root.join("sub");
        std::fs::create_dir_all(&cwd).unwrap();
        write_manifest(&evil_root, "evil"); // ancestor of cwd

        for degenerate in ["", "/", "relative/home"] {
            let got = detect_project_manifest_from(&cwd, Some(Path::new(degenerate)));
            assert_eq!(
                got, None,
                "degenerate HOME ({degenerate:?}) must not unlock the ancestor walk"
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn degenerate_home_value_cannot_unlock_an_ancestor_walk() {
        let (root, _home) = temp_home_tree("upeg_project_detect_degenerate_home_fallback");
        let cwd = root.join("elsewhere");
        std::fs::create_dir_all(&cwd).unwrap();

        for degenerate in ["", "/", "relative/home"] {
            let scope = SearchScope::classify(&cwd, Some(Path::new(degenerate)));
            assert!(matches!(scope, SearchScope::OutsideHome), "{degenerate:?}");
            assert!(
                !scope.in_walk(&cwd),
                "degenerate HOME must not unlock walking"
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn normal_absolute_home_is_used_verbatim() {
        let (root, home) = temp_home_tree("upeg_project_usable_home");
        assert_eq!(usable_home(Some(&home)), Some(home.as_path()));
        assert_eq!(usable_home(None), None);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn without_home_does_not_walk_ancestors() {
        let root = std::env::temp_dir().join("upeg_project_detect_no_home");
        let nested = root.join("a/b");
        let _ = std::fs::remove_dir_all(&root);
        write_manifest(&root, "root_only");
        std::fs::create_dir_all(&nested).unwrap();

        let got = detect_project_manifest_from(&nested, None);
        assert_eq!(
            got, None,
            "without home it must not walk ancestor directories at all"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn override_parsing_treats_empty_as_detect() {
        assert_eq!(
            parse_project_manifest_override(None),
            ProjectManifestOverride::Detect
        );
        assert_eq!(
            parse_project_manifest_override(Some("")),
            ProjectManifestOverride::Detect
        );
        assert_eq!(
            parse_project_manifest_override(Some("   ")),
            ProjectManifestOverride::Detect
        );
    }

    #[test]
    fn override_parsing_treats_off_case_insensitively_as_disabled() {
        assert_eq!(
            parse_project_manifest_override(Some("off")),
            ProjectManifestOverride::Disabled
        );
        assert_eq!(
            parse_project_manifest_override(Some("OFF")),
            ProjectManifestOverride::Disabled
        );
        assert_eq!(
            parse_project_manifest_override(Some(" Off ")),
            ProjectManifestOverride::Disabled
        );
    }

    #[test]
    fn override_parsing_treats_absolute_path_as_explicit() {
        let absolute = if cfg!(windows) {
            "C:\\proj\\.upeg"
        } else {
            "/proj/.upeg"
        };
        assert_eq!(
            parse_project_manifest_override(Some(absolute)),
            ProjectManifestOverride::Explicit(PathBuf::from(absolute))
        );
    }

    #[test]
    fn override_parsing_falls_back_to_detect_for_relative_path_and_returns_warning() {
        assert_eq!(
            parse_project_manifest_override(Some("relative/.upeg")),
            ProjectManifestOverride::Detect
        );
        let warning = project_manifest_override_relative_path_warning(Some("relative/.upeg"));
        assert!(warning.is_some());
        assert!(warning.unwrap().contains(PROJECT_MANIFEST_PATH_ENV));
    }

    #[test]
    fn no_warning_for_non_relative_path() {
        assert_eq!(project_manifest_override_relative_path_warning(None), None);
        assert_eq!(
            project_manifest_override_relative_path_warning(Some("off")),
            None
        );
        let absolute = if cfg!(windows) {
            "C:\\proj\\.upeg"
        } else {
            "/proj/.upeg"
        };
        assert_eq!(
            project_manifest_override_relative_path_warning(Some(absolute)),
            None
        );
    }

    #[test]
    fn explicit_absolute_path_wins_over_detectable_manifest() {
        let (root, home) = temp_home_tree("upeg_project_resolve_explicit_wins");
        let cwd = home.join("proj");
        write_manifest(&cwd, "detectable");
        let explicit_dir = root.join("explicit");
        write_manifest(&explicit_dir, "explicit");
        let explicit_path = explicit_dir.join(PROJECT_MANIFEST_FILE);

        let over = ProjectManifestOverride::Explicit(explicit_path.clone());
        let got = resolve_project_manifest(&over, &cwd, Some(&home)).expect("manifest");

        assert_eq!(
            got.path, explicit_path,
            "must be the explicit path, not the detected one"
        );
        assert_eq!(got.origin, ProjectManifestOrigin::EnvOverride);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_explicit_path_is_not_replaced_by_detection() {
        let (root, home) = temp_home_tree("upeg_project_resolve_explicit_missing");
        let cwd = home.join("proj");
        write_manifest(&cwd, "detectable");
        let missing = root.join("missing").join(PROJECT_MANIFEST_FILE);

        let over = ProjectManifestOverride::Explicit(missing);
        let got = resolve_project_manifest(&over, &cwd, Some(&home));

        assert_eq!(
            got, None,
            "missing explicit file must yield no manifest instead of falling back to detection"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn off_disables_even_when_manifest_is_detectable() {
        let (root, home) = temp_home_tree("upeg_project_resolve_disabled");
        let cwd = home.join("proj");
        write_manifest(&cwd, "detectable");

        let got = resolve_project_manifest(&ProjectManifestOverride::Disabled, &cwd, Some(&home));

        assert_eq!(got, None);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn detect_override_behaves_like_default_detection() {
        let (root, home) = temp_home_tree("upeg_project_resolve_detect");
        let cwd = home.join("proj");
        write_manifest(&home, "home_level");
        std::fs::create_dir_all(&cwd).unwrap();

        let got = resolve_project_manifest(&ProjectManifestOverride::Detect, &cwd, Some(&home))
            .expect("manifest");

        assert_eq!(got.path, home.join(PROJECT_MANIFEST_FILE));
        assert_eq!(got.origin, ProjectManifestOrigin::Detected);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn override_labels_distinguish_all_three_states() {
        assert_eq!(
            project_manifest_override_label(&ProjectManifestOverride::Detect),
            "detect"
        );
        assert_eq!(
            project_manifest_override_label(&ProjectManifestOverride::Disabled),
            "off"
        );
        assert_eq!(
            project_manifest_override_label(&ProjectManifestOverride::Explicit(PathBuf::from(
                "/x"
            ))),
            "explicit"
        );
    }
}
