fn main() -> Result<(), std::env::VarError> {
    let target = std::env::var("TARGET")?;
    println!("cargo:rustc-env=UPEG_NATIVE_TARGET={target}");
    let target_url = format!(
        "UPEG_TOOLKIT_CATALOG_URL_{}",
        target.replace('-', "_").to_ascii_uppercase()
    );
    println!("cargo:rerun-if-env-changed={target_url}");
    if let Ok(url) = std::env::var(target_url) {
        println!("cargo:rustc-env=UPEG_TOOLKIT_CATALOG_URL={url}");
    }
    Ok(())
}
