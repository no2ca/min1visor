fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let ld_script = if std::env::var("CARGO_FEATURE_RPI4").is_ok() {
        "rpi4/linker.ld"
    } else if std::env::var("CARGO_FEATURE_QEMU_VIRT").is_ok() {
        "scripts/qemu.ld"
    } else {
        panic!("Either feature `rpi4` or `qemu-virt` must be enabled");
    };

    let ld_path = format!("{}/{}", manifest_dir, ld_script);
    println!("cargo:rustc-link-arg-bins=-T{}", ld_path);
}
