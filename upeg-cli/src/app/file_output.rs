//! Writing a tool's `File` output to disk for `upeg call`.
//!
//! The tool-supplied `FileValue.name` is untrusted: on the PRD §5.2 L4 attach
//! path it arrives verbatim from whatever host the CLI is attached to. This
//! module is the only place that turns such a name into a filesystem path, and
//! it is generic over every tool — so the containment lives here rather than in
//! any one toolkit.

use crate::error::CliError;

/// The operator's answers to "where does a `File` output go, and may it
/// replace what is already there" — `--out` and `--force`. They are only ever
/// read together, so they travel together.
#[derive(Debug, Default)]
pub(super) struct FileOutputOptions {
    /// `--out`: an explicit destination file, or a directory to place the
    /// tool's own named file into. `None` means the current directory.
    pub(super) out: Option<std::path::PathBuf>,
    /// `--force`: permission to truncate an existing destination.
    pub(super) force: bool,
}

/// A tool-supplied file name proven safe to join onto an output directory.
///
/// Construction goes through [`OutputFileName::parse`] and nothing else, so a
/// value of this type cannot name a parent directory, an absolute path, or a
/// nested path — holding one *is* the proof that `dir.join(name)` stays under
/// `dir`. `resolve_file_output_path` takes this rather than `&str` so a future
/// caller cannot reintroduce the unchecked join by accident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OutputFileName(String);

impl OutputFileName {
    /// Accept a tool-supplied name only if it is already a bare filename.
    ///
    /// `Path::join` is why this is not cosmetic: `join("../../etc/cron.d/x")`
    /// escapes the base, and `join("/etc/passwd")` *discards* the base and
    /// returns the absolute path outright.
    ///
    /// The check is "exactly one [`Component::Normal`] and nothing else",
    /// which is the definition of a bare filename rather than a blocklist of
    /// the spellings that bite — a root, a prefix, a `.`, a `..`, an embedded
    /// separator, and the empty string are each *not* that one component, so
    /// each is refused without being enumerated.
    ///
    /// Refusing beats quietly reducing a path-ish name to its basename: a tool
    /// that names its output `/etc/passwd` is broken or hostile, and writing
    /// `passwd` for it would contain the damage while hiding the fact. The
    /// error points at `--out`, which is the operator's way to say where a
    /// file goes.
    pub(super) fn parse(name: &str) -> Result<Self, CliError> {
        let mut components = std::path::Path::new(name).components();
        let (Some(std::path::Component::Normal(only)), None) =
            (components.next(), components.next())
        else {
            return Err(Self::rejected(name));
        };
        only.to_str()
            .map(|only| Self(only.to_string()))
            .ok_or_else(|| Self::rejected(name))
    }

    fn rejected(name: &str) -> CliError {
        CliError::tool_failed(format!(
            "tool produced an unusable output file name `{name}`; \
             pass an explicit `--out <PATH>` to choose the destination"
        ))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// Resolve the on-disk destination for a `File` output: `out` verbatim when it
/// names a file, `out.join(name)` when it is (or ends like) a directory, and
/// the current directory joined with `name` when `out` is absent.
///
/// The two joining lanes go through [`OutputFileName`] and are therefore safe
/// by construction. The middle lane is an explicit `--out <file>`: the
/// operator's own word, honoured as given, and it never consults the tool's
/// name at all — so a hostile name is not merely contained there, it is unread.
pub(super) fn resolve_file_output_path(
    out: Option<&std::path::Path>,
    name: &str,
) -> Result<std::path::PathBuf, CliError> {
    match out {
        Some(path) if path.is_dir() => Ok(path.join(OutputFileName::parse(name)?.as_str())),
        Some(path) => Ok(path.to_path_buf()),
        None => {
            let name = OutputFileName::parse(name)?;
            std::env::current_dir()
                .map(|dir| dir.join(name.as_str()))
                .map_err(|e| {
                    CliError::tool_failed(format!("failed to resolve current directory: {e}"))
                })
        }
    }
}

/// If the success's primary output (or first output) is a `File`, write its
/// bytes to disk and return the destination path. Returns `Ok(None)` when the
/// tool produced no file output, so non-file tools flow through unchanged.
///
/// An existing destination is never truncated unless `--force` was passed —
/// see [`ensure_writable`].
pub(super) fn write_file_output(
    success: &upeg_core::ToolSuccess,
    options: &FileOutputOptions,
) -> Result<Option<String>, CliError> {
    let entry = success
        .primary_output_id
        .as_deref()
        .and_then(|primary| success.outputs.iter().find(|entry| entry.id == primary))
        .or_else(|| success.outputs.first());
    let Some(upeg_core::OutputValue::File(file)) = entry.map(|entry| &entry.value) else {
        return Ok(None);
    };
    let upeg_core::FileContent::Bytes(bytes) = &file.content else {
        return Err(CliError::tool_failed(
            "directory file outputs cannot be written to a single --out path".to_string(),
        ));
    };
    let target = resolve_file_output_path(options.out.as_deref(), &file.name)?;
    ensure_writable(&target, options.force)?;
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            CliError::tool_failed(format!("failed to create output directory: {e}"))
        })?;
    }
    std::fs::write(&target, bytes).map_err(|e| {
        CliError::tool_failed(format!("failed to write output {}: {e}", target.display()))
    })?;
    Ok(Some(target.display().to_string()))
}

/// Refuse to clobber an existing destination unless `--force` was passed.
///
/// Refusing beats prompting here: `upeg call` is routinely scripted and piped,
/// so a prompt would either hang a non-interactive run or have to guess a
/// default. Refusing is deterministic in both settings, and the error names the
/// flag that overrides it. This is the layer the guard belongs at now — writing
/// moved out of the tools, so a per-tool `overwrite` arg would only cover the
/// tools that remembered to declare it.
fn ensure_writable(target: &std::path::Path, force: bool) -> Result<(), CliError> {
    if force || !target.exists() {
        return Ok(());
    }
    Err(CliError::tool_failed(format!(
        "output {} already exists; pass --force to overwrite",
        target.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name a tool could send that must not become a path of its own.
    const 탈출_시도: &[&str] = &[
        "/etc/passwd",
        "/home/user/.bashrc",
        "../../etc/cron.d/evil",
        "..",
        "../evil",
        "a/../../evil",
        "sub/dir/file.txt",
        ".",
        "",
        "   ",
    ];

    /// Bytes a hostile tool would like written wherever it names.
    const 페이로드: &[u8] = b"pwned";

    /// Output id of the single `File` entry the fixtures below produce.
    const 출력_ID: &str = "file";

    fn 파일_출력(name: &str) -> upeg_core::ToolSuccess {
        use upeg_core::{FileContent, FileValue, OutputEntry, OutputKind, OutputValue};
        upeg_core::ToolSuccess::new(
            Some(출력_ID.to_string()),
            vec![OutputEntry {
                id: 출력_ID.to_string(),
                label: Some("File".to_string()),
                kind: OutputKind::File,
                value: OutputValue::File(FileValue {
                    name: name.to_string(),
                    mime: None,
                    content: FileContent::Bytes(페이로드.to_vec()),
                }),
            }],
        )
        .expect("test ToolSuccess is well-formed")
    }

    /// A fresh empty directory, named per-test so parallel runs don't collide.
    fn 임시_디렉터리(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "upeg_cli_file_output_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn 출력_옵션(out: Option<&std::path::Path>, force: bool) -> FileOutputOptions {
        FileOutputOptions {
            out: out.map(std::path::Path::to_path_buf),
            force,
        }
    }

    #[test]
    fn 절대경로나_상위_탈출_이름은_거부된다() {
        for name in 탈출_시도 {
            // `"   "` is a legal (if silly) component; everything else must go.
            if *name == "   " {
                continue;
            }
            OutputFileName::parse(name)
                .map(|parsed| parsed.0)
                .expect_err(&format!("`{name}` must be rejected"));
        }
    }

    #[test]
    fn 평범한_basename은_그대로_통과한다() {
        for name in ["a.png", "a-images.zip", "file.txt", ".hidden", "   "] {
            let parsed = OutputFileName::parse(name).expect("plain basename is accepted");
            assert_eq!(parsed.as_str(), name);
        }
    }

    #[test]
    fn 탈출_이름은_출력_디렉터리_밖에_쓰이지_못한다() {
        let dir = 임시_디렉터리("escape");
        for name in 탈출_시도 {
            if *name == "   " {
                continue;
            }
            let error = write_file_output(&파일_출력(name), &출력_옵션(Some(&dir), false))
                .err()
                .unwrap_or_else(|| panic!("`{name}` must not be written"));
            assert!(
                error.message().contains("unusable output file name"),
                "`{name}`: {}",
                error.message()
            );
        }
        // Nothing escaped the directory, and nothing landed inside it either.
        assert!(!std::path::Path::new("/etc/cron.d/evil").exists());
        let entries = std::fs::read_dir(&dir).expect("read temp dir");
        assert_eq!(entries.count(), 0, "no file may have been created");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 출력_디렉터리에_basename으로_기록된다() {
        let dir = 임시_디렉터리("basename");
        let written = write_file_output(&파일_출력("a.png"), &출력_옵션(Some(&dir), false))
            .expect("write succeeds")
            .expect("a file output was written");
        assert_eq!(written, dir.join("a.png").display().to_string());
        assert_eq!(
            std::fs::read(dir.join("a.png")).expect("read back"),
            페이로드
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 이미_존재하는_파일은_force_없이는_덮어쓰지_않는다() {
        let dir = 임시_디렉터리("no_clobber");
        let target = dir.join("a.png");
        std::fs::write(&target, b"original").expect("seed existing file");

        let error = write_file_output(&파일_출력("a.png"), &출력_옵션(Some(&dir), false))
            .expect_err("existing file must not be clobbered");
        assert!(error.message().contains("--force"), "{}", error.message());
        assert_eq!(std::fs::read(&target).expect("read back"), b"original");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn force를_주면_기존_파일을_덮어쓴다() {
        let dir = 임시_디렉터리("force");
        let target = dir.join("a.png");
        std::fs::write(&target, b"original").expect("seed existing file");

        write_file_output(&파일_출력("a.png"), &출력_옵션(Some(&dir), true))
            .expect("force overwrites");
        assert_eq!(std::fs::read(&target).expect("read back"), 페이로드);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 명시한_out_경로는_그대로_쓰인다() {
        let dir = 임시_디렉터리("explicit_out");
        let target = dir.join("chosen.bin");
        // The operator's own `--out` wins over the tool's name, and a hostile
        // name cannot redirect it — the name is not consulted on this lane.
        let written =
            write_file_output(&파일_출력("/etc/passwd"), &출력_옵션(Some(&target), false))
                .expect("explicit --out is honoured")
                .expect("a file output was written");
        assert_eq!(written, target.display().to_string());
        assert!(target.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
