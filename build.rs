use std::process::Command;

fn main() {
    // Query the Python interpreter for its shared library directory so that
    // test binaries (which embed the interpreter) can locate libpython at
    // runtime via an rpath entry.
    let output =
        Command::new(std::env::var("PYO3_PYTHON").unwrap_or_else(|_| "python3".to_string()))
            .args([
                "-c",
                "import sysconfig; print(sysconfig.get_config_var('LIBDIR'))",
            ])
            .output();

    if let Ok(out) = output {
        if out.status.success() {
            let libdir = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !libdir.is_empty() {
                println!("cargo:rustc-link-arg=-Wl,-rpath,{libdir}");
            }
        }
    }
}
