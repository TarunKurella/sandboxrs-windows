use sandboxrs_windows::Sandbox;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r"C:\repo".to_string());

    let sandbox = Sandbox::builder(&workspace)
        .read_only(std::env::temp_dir())
        .build()?;

    for (program, args) in [
        ("cargo", vec!["check"]),
        ("git", vec!["status", "--short"]),
    ] {
        let mut command = sandbox.command(program);
        command.args(args);
        let output = command.output()?;
        println!("{}: {:?}", output.backend, output.status);
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }
    Ok(())
}
