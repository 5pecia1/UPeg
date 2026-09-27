fn main() -> Result<(), std::env::VarError> {
    println!(
        "cargo:rustc-env=UPEG_NATIVE_TARGET={}",
        std::env::var("TARGET")?
    );
    Ok(())
}
