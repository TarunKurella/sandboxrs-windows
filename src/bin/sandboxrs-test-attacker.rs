use std::fs;
use std::io::Write;
use std::process::Command;

// Test-only hostile helper. It is not shipped as part of the public product
// and is used by the shared backend contract and eval suites.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("read") if args.len() == 2 => match fs::read(&args[1]) {
            Ok(bytes) => {
                std::io::stdout().write_all(&bytes).unwrap();
                std::process::exit(0);
            }
            Err(err) => {
                eprintln!("read failed: {err}");
                std::process::exit(1);
            }
        },
        Some("write") if args.len() == 2 => match fs::write(&args[1], b"attacker") {
            Ok(()) => std::process::exit(0),
            Err(err) => {
                eprintln!("write failed: {err}");
                std::process::exit(1);
            }
        },
        Some("delete") if args.len() == 2 => match fs::remove_file(&args[1]) {
            Ok(()) => std::process::exit(0),
            Err(err) => {
                eprintln!("delete failed: {err}");
                std::process::exit(1);
            }
        },
        Some("spawn-read") if args.len() == 2 => {
            let output = Command::new(std::env::current_exe().unwrap())
                .arg("read")
                .arg(&args[1])
                .output()
                .unwrap();
            std::io::stdout().write_all(&output.stdout).unwrap();
            std::process::exit(output.status.code().unwrap_or(1));
        }
        Some("spawn-write") if args.len() == 2 => {
            let status = Command::new(std::env::current_exe().unwrap())
                .arg("write")
                .arg(&args[1])
                .status()
                .unwrap();
            std::process::exit(status.code().unwrap_or(1));
        }
        Some("spawn-delete") if args.len() == 2 => {
            let status = Command::new(std::env::current_exe().unwrap())
                .arg("delete")
                .arg(&args[1])
                .status()
                .unwrap();
            std::process::exit(status.code().unwrap_or(1));
        }
        Some("grandchild-read") if args.len() == 2 => {
            let status = Command::new(std::env::current_exe().unwrap())
                .arg("spawn-read")
                .arg(&args[1])
                .status()
                .unwrap();
            std::process::exit(status.code().unwrap_or(1));
        }
        Some("grandchild-write") if args.len() == 2 => {
            let status = Command::new(std::env::current_exe().unwrap())
                .arg("spawn-write")
                .arg(&args[1])
                .status()
                .unwrap();
            std::process::exit(status.code().unwrap_or(1));
        }
        Some("env") if args.len() == 2 => match std::env::var(&args[1]) {
            Ok(value) => {
                println!("{value}");
                std::process::exit(0);
            }
            Err(_) => {
                eprintln!("env missing");
                std::process::exit(1);
            }
        },
        Some("sleep") if args.len() <= 2 => {
            let seconds = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(30);
            std::thread::sleep(std::time::Duration::from_secs(seconds));
            std::process::exit(0);
        }
        Some("allocate-memory") if args.len() <= 2 => {
            let bytes: usize = args
                .get(1)
                .and_then(|v| v.parse::<usize>().ok())
                .map(|mb| mb * 1024 * 1024)
                .unwrap_or(128 * 1024 * 1024);
            let mut block = vec![0u8; bytes];
            block[0] = 1;
            block[bytes - 1] = 2;
            std::thread::sleep(std::time::Duration::from_secs(30));
            std::process::exit(0);
        }
        Some("spawn-many") if args.len() <= 2 => {
            let count: u32 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(5);
            let mut children = Vec::new();
            for _ in 0..count {
                children.push(
                    Command::new(std::env::current_exe().unwrap())
                        .arg("sleep")
                        .arg("30")
                        .spawn()
                        .unwrap(),
                );
            }
            for mut child in children {
                let _ = child.wait();
            }
            std::process::exit(0);
        }
        Some("spawn-child-sleep") if args.len() == 2 => {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .arg("sleep")
                .arg("120")
                .spawn()
                .unwrap();
            fs::write(&args[1], child.id().to_string()).unwrap();
            let _ = child.wait();
            std::process::exit(0);
        }
        _ => {
            eprintln!(
                "usage: attacker read|write|delete|spawn-read|spawn-write|spawn-delete|grandchild-read|grandchild-write PATH | env NAME | sleep [S] | allocate-memory [MB] | spawn-many [N] | spawn-child-sleep PIDFILE"
            );
            std::process::exit(2);
        }
    }
}
