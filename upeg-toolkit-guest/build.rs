const TOOLKITS: &[&str] = &[
    "COLOR",
    "CONVERT",
    "CSV",
    "DEVCONTAINER",
    "ETH",
    "HASH",
    "ID",
    "MEDIA",
    "NET",
    "NUM",
    "QR",
    "SECURITY",
    "TEXT",
    "TIME",
    "WEATHER",
];
const GUEST_ABI: &str = include_str!("../upeg-toolkit-catalog/src/guest_abi.txt");
const METADATA_PATH: &str = "../upeg-toolkit-catalog/src/metadata.snapshot.json";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selected: Vec<_> = TOOLKITS
        .iter()
        .filter(|name| std::env::var_os(format!("CARGO_FEATURE_{name}")).is_some())
        .collect();
    let [selected] = selected.as_slice() else {
        return Err(std::io::Error::other(format!(
            "upeg-toolkit-guest requires exactly one toolkit feature, got {selected:?}"
        ))
        .into());
    };
    println!(
        "cargo:rustc-env=UPEG_TOOLKIT_ID={}",
        selected.to_lowercase()
    );
    println!("cargo:rerun-if-changed={METADATA_PATH}");
    println!("cargo:rerun-if-changed=../upeg-toolkit-catalog/src/guest_abi.txt");
    let bytes = std::fs::read(METADATA_PATH)?;
    let metadata: serde_json::Value = serde_json::from_slice(&bytes)?;
    let canonical = serde_jcs::to_vec(&(GUEST_ABI, metadata))?;
    use sha2::Digest as _;
    let digest = format!("{:x}", sha2::Sha256::digest(canonical));
    println!("cargo:rustc-env=UPEG_GUEST_ABI_DIGEST={digest}");
    Ok(())
}
