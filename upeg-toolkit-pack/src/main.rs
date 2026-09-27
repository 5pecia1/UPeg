#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::get_unwrap,
        reason = "tests use fixture assertions"
    )
)]

use flate2::{Compression, GzBuilder, read::GzDecoder};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use upeg_core::{StaticToolMeta, ToolMeta, ToolkitMeta};
use upeg_toolkit_catalog::{
    Catalog, MetadataSnapshot, NATIVE_PACK_TOOLKITS as NATIVE, NativeArtifact, SCHEMA_VERSION,
    ToolMetadata, ToolkitMetadata, ToolkitPackage, WEB_PACK_TOOLKITS as WEB, WebArtifact,
    abi_digest, sha256_hex,
};
const NATIVE_MAX_BYTES: u64 = 25 * 1024 * 1024;
const EXPANDED_MAX_BYTES: u64 = 64 * 1024 * 1024;

fn main() {
    if let Err(error) = run() {
        eprintln!("toolkit pack: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("prepare-web") => prepare_web(Path::new(option(&args, "--out")?)),
        Some("verify-metadata") => verify_metadata(),
        Some("build-web") => build_web(Path::new(option(&args, "--out")?)),
        Some("verify-web") => verify_web(
            Path::new(option(&args, "--root")?),
            option(&args, "--app-version")?,
        ),
        Some("build-native") => build_native(
            option(&args, "--target")?,
            Path::new(option(&args, "--out")?),
            option(&args, "--base-url")?,
            args.iter().any(|arg| arg == "--flat-release-assets"),
        ),
        Some("verify-native") => verify_native(
            Path::new(option(&args, "--root")?),
            option(&args, "--target")?,
            option(&args, "--app-version")?,
        ),
        _ => Err("usage: prepare-web --out FILE | verify-metadata | build-web --out DIR | verify-web --root DIR --app-version V | build-native --target TRIPLE --out DIR --base-url URL | verify-native --root DIR --target TRIPLE --app-version V".into()),
    }
}

fn option<'a>(args: &'a [String], key: &str) -> Result<&'a str, String> {
    args.windows(2)
        .find(|pair| pair[0] == key)
        .map(|pair| pair[1].as_str())
        .ok_or_else(|| format!("missing {key}"))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn cargo_target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target"))
}

fn build_lock() -> Result<fs::File, String> {
    let path = workspace_root().join("target/toolkit-pack-build.lock");
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.lock().map_err(|error| error.to_string())?;
    Ok(file)
}

fn source_snapshot() -> Result<MetadataSnapshot, String> {
    // Linking this call makes every #[tool]/#[toolkit] submission visible,
    // including the handwritten GUI-only metadata.
    upeg_tools::register_all();
    let mut toolkits = BTreeMap::<String, ToolkitMetadata>::new();
    for meta in upeg_core::inventory::iter::<ToolkitMeta>() {
        toolkits.insert(
            meta.id.to_owned(),
            ToolkitMetadata {
                id: meta.id.to_owned(),
                tags: meta.tags.iter().map(|tag| (*tag).to_owned()).collect(),
                description: meta.description.to_owned(),
                tools: Vec::new(),
            },
        );
    }
    for static_meta in upeg_core::inventory::iter::<StaticToolMeta>() {
        let meta = ToolMeta::from_static(static_meta).map_err(|error| error.to_string())?;
        toolkits
            .entry(meta.toolkit.to_owned())
            .or_insert_with(|| ToolkitMetadata {
                id: meta.toolkit.to_owned(),
                tags: Vec::new(),
                description: String::new(),
                tools: Vec::new(),
            })
            .tools
            .push(ToolMetadata::from(&meta));
    }
    let mut toolkits: Vec<_> = toolkits.into_values().collect();
    for toolkit in &mut toolkits {
        toolkit.tools.sort_by(|a, b| a.id.cmp(&b.id));
    }
    Ok(MetadataSnapshot {
        schema_version: SCHEMA_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        toolkits,
    })
}

fn saved_snapshot() -> Result<MetadataSnapshot, String> {
    serde_json::from_str(upeg_toolkit_catalog::EMBEDDED_METADATA)
        .map_err(|error| format!("embedded metadata: {error}"))
}

fn prepare_web(out: &Path) -> Result<(), String> {
    let snapshot = source_snapshot()?;
    let bytes = serde_json::to_vec_pretty(&snapshot).map_err(|error| error.to_string())?;
    write_bytes(
        &workspace_root().join("upeg-toolkit-catalog/src/metadata.snapshot.json"),
        &bytes,
    )?;
    let digest = abi_digest(&snapshot).map_err(|error| error.to_string())?;
    write_bytes(out, format!("{digest}\n").as_bytes())?;
    writeln!(std::io::stdout().lock(), "{digest}").map_err(|error| error.to_string())?;
    Ok(())
}

fn verify_metadata() -> Result<(), String> {
    if serde_json::to_value(source_snapshot()?).map_err(|error| error.to_string())?
        != serde_json::to_value(saved_snapshot()?).map_err(|error| error.to_string())?
    {
        return Err("metadata.snapshot.json has drifted; run prepare-web".into());
    }
    Ok(())
}

fn new_catalog(snapshot: MetadataSnapshot) -> Result<Catalog, String> {
    let abi = abi_digest(&snapshot).map_err(|error| error.to_string())?;
    let mut catalog = Catalog {
        schema_version: SCHEMA_VERSION,
        app_version: snapshot.app_version.clone(),
        abi_digest: abi,
        catalog_digest: String::new(),
        toolkits: snapshot
            .toolkits
            .into_iter()
            .map(|metadata| ToolkitPackage {
                version: env!("CARGO_PKG_VERSION").to_owned(),
                requires_host: NATIVE.contains(&metadata.id.as_str())
                    && !WEB.contains(&metadata.id.as_str()),
                metadata,
                web: None,
                native: BTreeMap::new(),
            })
            .collect(),
    };
    catalog.catalog_digest = catalog
        .digest_without_self()
        .map_err(|error| error.to_string())?;
    Ok(catalog)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn artifact_hash(path: &Path) -> Result<(String, u64), String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok((sha256_hex(&bytes), bytes.len() as u64))
}

fn write_catalog(path: &Path, catalog: &mut Catalog) -> Result<(), String> {
    catalog.catalog_digest = catalog
        .digest_without_self()
        .map_err(|error| error.to_string())?;
    write_bytes(
        path,
        &serde_json::to_vec_pretty(catalog).map_err(|error| error.to_string())?,
    )
}

fn verify_glue(path: &Path) -> Result<(), String> {
    let glue = fs::read_to_string(path).map_err(|error| error.to_string())?;
    if glue.contains("from './")
        || glue.contains("from \"./")
        || glue.contains("import('./")
        || glue.contains("import(\"./")
        || glue.contains("snippets/")
    {
        return Err(format!(
            "{} imports an unverified relative asset",
            path.display()
        ));
    }
    Ok(())
}

fn local_web_path(root: &Path, url: &str) -> Result<PathBuf, String> {
    let relative = url
        .strip_prefix("/toolkits/")
        .ok_or("artifact URL must start /toolkits/")?;
    let path = Path::new(relative);
    if path
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("unsafe artifact path {url}"));
    }
    Ok(root.join("toolkits").join(path))
}

fn build_web(out: &Path) -> Result<(), String> {
    let _build_lock = build_lock()?;
    verify_metadata()?;
    let out = if out.is_absolute() {
        out.to_path_buf()
    } else {
        workspace_root().join(out)
    };
    if out.file_name().and_then(|name| name.to_str()) != Some("toolkits") {
        return Err("web pack output must be a dedicated `toolkits` directory".into());
    }
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|error| format!("{}: {error}", out.display()))?;
    }
    fs::create_dir_all(&out).map_err(|error| error.to_string())?;
    let mut catalog = new_catalog(saved_snapshot()?)?;
    for toolkit in &mut catalog.toolkits {
        let id = toolkit.metadata.id.as_str();
        if !WEB.contains(&id) {
            continue;
        }
        let stage = out.join(".staging").join(id);
        if stage.exists() {
            fs::remove_dir_all(&stage).map_err(|error| error.to_string())?;
        }
        fs::create_dir_all(&stage).map_err(|error| error.to_string())?;
        let name = format!("upeg_toolkit_{id}");
        let status = Command::new("wasm-pack")
            .arg("build")
            .arg(workspace_root().join("upeg-toolkit-guest"))
            .args([
                "--target",
                "web",
                "--release",
                "--no-opt",
                "--no-typescript",
            ])
            .arg("--out-dir")
            .arg(&stage)
            .arg("--out-name")
            .arg(&name)
            .args(["--no-default-features", "--features", id])
            .status()
            .map_err(|error| format!("wasm-pack: {error}"))?;
        if !status.success() {
            return Err(format!("web pack build failed for {id}: {status}"));
        }
        let js = stage.join(format!("{name}.js"));
        let wasm = stage.join(format!("{name}_bg.wasm"));
        verify_glue(&js)?;
        let (js_sha256, js_size) = artifact_hash(&js)?;
        let (sha256, size) = artifact_hash(&wasm)?;
        let bundle_digest = sha256_hex(format!("{js_sha256}:{sha256}").as_bytes());
        let dir = out.join(id).join(&toolkit.version).join(&bundle_digest);
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        fs::copy(&js, dir.join(format!("{name}.js"))).map_err(|error| error.to_string())?;
        fs::copy(&wasm, dir.join(format!("{name}_bg.wasm"))).map_err(|error| error.to_string())?;
        fs::remove_dir_all(&stage).map_err(|error| error.to_string())?;
        toolkit.web = Some(WebArtifact {
            js: format!(
                "/toolkits/{id}/{}/{bundle_digest}/{name}.js",
                toolkit.version
            ),
            js_sha256,
            js_size,
            wasm: format!(
                "/toolkits/{id}/{}/{bundle_digest}/{name}_bg.wasm",
                toolkit.version
            ),
            sha256,
            size,
        });
    }
    let _ = fs::remove_dir(out.join(".staging"));
    write_catalog(&out.join("catalog.json"), &mut catalog)?;
    verify_web(
        out.parent().ok_or("toolkit output has no parent")?,
        &catalog.app_version,
    )
}

fn verify_web(root: &Path, app_version: &str) -> Result<(), String> {
    let catalog: Catalog = serde_json::from_slice(
        &fs::read(root.join("toolkits/catalog.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let abi = abi_digest(&saved_snapshot()?).map_err(|error| error.to_string())?;
    catalog.validate(&abi).map_err(|error| error.to_string())?;
    if catalog.app_version != app_version {
        return Err(format!(
            "app version {} != {app_version}",
            catalog.app_version
        ));
    }
    let mut seen = 0;
    for toolkit in &catalog.toolkits {
        let id = toolkit.metadata.id.as_str();
        if !WEB.contains(&id) {
            if toolkit.web.is_some() {
                return Err(format!("native-only toolkit {id} has web artifact"));
            }
            continue;
        }
        let web = toolkit
            .web
            .as_ref()
            .ok_or_else(|| format!("missing web pack {id}"))?;
        let bundle_digest = sha256_hex(format!("{}:{}", web.js_sha256, web.sha256).as_bytes());
        let expected_prefix = format!("/toolkits/{id}/{}/{bundle_digest}/", toolkit.version);
        if !web.js.starts_with(&expected_prefix) || !web.wasm.starts_with(&expected_prefix) {
            return Err(format!("web pack {id} lacks content-addressed URLs"));
        }
        let js = local_web_path(root, &web.js)?;
        let wasm = local_web_path(root, &web.wasm)?;
        verify_glue(&js)?;
        if artifact_hash(&js)? != (web.js_sha256.clone(), web.js_size)
            || artifact_hash(&wasm)? != (web.sha256.clone(), web.size)
        {
            return Err(format!("web pack checksum mismatch: {id}"));
        }
        seen += 1;
    }
    if seen != WEB.len() {
        return Err(format!("web pack count {seen} != {}", WEB.len()));
    }
    Ok(())
}

fn build_native(target: &str, out: &Path, base_url: &str, flat: bool) -> Result<(), String> {
    let _build_lock = build_lock()?;
    if !base_url.starts_with("https://") {
        return Err("native base URL must use https".into());
    }
    verify_metadata()?;
    let out = if out.is_absolute() {
        out.to_path_buf()
    } else {
        workspace_root().join(out)
    };
    let mut catalog = new_catalog(saved_snapshot()?)?;
    for toolkit in &mut catalog.toolkits {
        let id = toolkit.metadata.id.as_str();
        if !NATIVE.contains(&id) {
            continue;
        }
        let status = Command::new("cargo")
            .args([
                "build",
                "-p",
                "upeg-toolkit-guest",
                "--bin",
                "upeg-toolkit-guest",
                "--release",
                "--target",
                target,
                "--no-default-features",
                "--features",
            ])
            .arg(format!("native-runner,{id}"))
            .status()
            .map_err(|error| format!("cargo build: {error}"))?;
        if !status.success() {
            return Err(format!("native pack build failed for {id}: {status}"));
        }
        let extension = if target.contains("windows") {
            ".exe"
        } else {
            ""
        };
        let name = if flat {
            format!("toolkits-{target}-{id}-{}{extension}.gz", toolkit.version)
        } else {
            format!("upeg-toolkit-{id}{extension}.gz")
        };
        let source = cargo_target_dir()
            .join(target)
            .join("release")
            .join(format!("upeg-toolkit-guest{extension}"));
        let bytes = fs::read(&source).map_err(|error| format!("{}: {error}", source.display()))?;
        if bytes.len() as u64 > EXPANDED_MAX_BYTES {
            return Err(format!("{id} sidecar exceeds expanded maximum"));
        }
        let expanded_sha256 = sha256_hex(&bytes);
        let expanded_size = bytes.len() as u64;
        let mut encoder = GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), Compression::best());
        encoder
            .write_all(&bytes)
            .map_err(|error| error.to_string())?;
        let compressed = encoder.finish().map_err(|error| error.to_string())?;
        if compressed.len() as u64 > NATIVE_MAX_BYTES {
            return Err(format!(
                "{id} compressed sidecar is {} bytes; max {NATIVE_MAX_BYTES}",
                compressed.len()
            ));
        }
        let dest = if flat {
            out.join(&name)
        } else {
            out.join(id).join(&toolkit.version).join(&name)
        };
        write_bytes(&dest, &compressed)?;
        let (sha256, size) = artifact_hash(&dest)?;
        toolkit.native.insert(
            target.to_owned(),
            NativeArtifact {
                url: if flat {
                    format!("{}/{}", base_url.trim_end_matches('/'), name)
                } else {
                    format!(
                        "{}/{}/{}/{}",
                        base_url.trim_end_matches('/'),
                        id,
                        toolkit.version,
                        name
                    )
                },
                sha256,
                size,
                encoding: "gzip".to_owned(),
                expanded_sha256,
                expanded_size,
            },
        );
    }
    let catalog_name = if flat {
        format!("toolkits-{target}-catalog.json")
    } else {
        "catalog.json".to_owned()
    };
    write_catalog(&out.join(catalog_name), &mut catalog)?;
    verify_native(&out, target, &catalog.app_version)
}

fn verify_native(root: &Path, target: &str, app_version: &str) -> Result<(), String> {
    let catalog_path = if root.join("catalog.json").exists() {
        root.join("catalog.json")
    } else {
        root.join(format!("toolkits-{target}-catalog.json"))
    };
    let catalog: Catalog =
        serde_json::from_slice(&fs::read(catalog_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let abi = abi_digest(&saved_snapshot()?).map_err(|error| error.to_string())?;
    catalog.validate(&abi).map_err(|error| error.to_string())?;
    if catalog.app_version != app_version {
        return Err(format!(
            "app version {} != {app_version}",
            catalog.app_version
        ));
    }
    let mut seen = 0;
    for toolkit in &catalog.toolkits {
        let id = toolkit.metadata.id.as_str();
        if !NATIVE.contains(&id) {
            continue;
        }
        let artifact = toolkit
            .native
            .get(target)
            .ok_or_else(|| format!("missing native pack {id}"))?;
        let filename = artifact
            .url
            .rsplit('/')
            .next()
            .ok_or("native URL has no filename")?;
        let path = if root.join(filename).exists() {
            root.join(filename)
        } else {
            root.join(id).join(&toolkit.version).join(filename)
        };
        let (sha256, size) = artifact_hash(&path)?;
        if size > NATIVE_MAX_BYTES
            || (sha256, size) != (artifact.sha256.clone(), artifact.size)
            || artifact.encoding != "gzip"
            || artifact.expanded_size > EXPANDED_MAX_BYTES
        {
            return Err(format!("native pack invalid: {id}"));
        }
        let compressed = fs::read(&path).map_err(|error| error.to_string())?;
        let mut expanded = Vec::new();
        GzDecoder::new(compressed.as_slice())
            .take(EXPANDED_MAX_BYTES + 1)
            .read_to_end(&mut expanded)
            .map_err(|error| error.to_string())?;
        if expanded.len() as u64 != artifact.expanded_size
            || sha256_hex(&expanded) != artifact.expanded_sha256
        {
            return Err(format!("native expanded checksum mismatch: {id}"));
        }
        seen += 1;
    }
    if seen != NATIVE.len() {
        return Err(format!("native pack count {seen} != {}", NATIVE.len()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_metadata_matches_every_tool_annotation() {
        verify_metadata().expect("generated metadata must match all source annotations");
        let snapshot = saved_snapshot().expect("snapshot parses");
        assert_eq!(snapshot.toolkits.len(), 17);
        assert_eq!(
            snapshot
                .toolkits
                .iter()
                .map(|toolkit| toolkit.tools.len())
                .sum::<usize>(),
            73
        );
        for id in [
            "memo.scratch",
            "memo.create",
            "embed.transform_tools",
            "time.epoch",
            "net.status",
        ] {
            assert!(
                snapshot
                    .toolkits
                    .iter()
                    .flat_map(|toolkit| &toolkit.tools)
                    .any(|tool| tool.id == id)
            );
        }
    }

    #[test]
    fn every_executable_toolkit_has_an_artifact_target() {
        let snapshot = saved_snapshot().expect("snapshot parses");
        for toolkit in snapshot.toolkits {
            if toolkit.id != "memo"
                && toolkit
                    .tools
                    .iter()
                    .any(|tool| tool.invoker == upeg_core::Invoker::Function)
            {
                assert!(
                    NATIVE.contains(&toolkit.id.as_str()),
                    "missing native pack for {}",
                    toolkit.id
                );
            }
        }
    }
}
