use sandboxrs_windows::Sandbox;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r"C:\repo".to_string());

    let sandbox = Sandbox::builder(&workspace)
        .read_write(std::env::temp_dir())
        .build()?;

    let output = sandbox
        .command("cargo")
        .args(["test", "--workspace"])
        .env("RUST_BACKTRACE", "1")
        .output()?;

    println!("backend: {}", output.backend);
    println!("status: {:?}", output.status);
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    Ok(())
}
