fn main() {
    // Ensure test binaries can find the Python shared library at runtime.
    // This is necessary when Python is installed in a non-standard location
    // (e.g. uv-managed interpreters).
    let python = std::env::var("PYO3_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let output = std::process::Command::new(&python)
        .args([
            "-c",
            "import sysconfig; print(sysconfig.get_config_var('LIBDIR'))",
        ])
        .output()
        .expect("Failed to query Python for LIBDIR");
    let libdir = String::from_utf8(output.stdout)
        .expect("Non-UTF-8 LIBDIR")
        .trim()
        .to_string();
    if !libdir.is_empty() {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{libdir}");
    }
    println!("cargo:rerun-if-env-changed=PYO3_PYTHON");
}
