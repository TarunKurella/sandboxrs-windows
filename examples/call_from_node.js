const { spawn } = require("child_process");

// sandboxrs-windows 0.1.0 exec adapter for non-Rust runtimes.
const sandboxrs = spawn(
  "sandboxrs",
  [
    "exec",
    "--json",
    "--workspace",
    process.env.SANDBOXRS_WORKSPACE || "C:\\repo",
    "--",
    "cargo",
    "test",
  ],
  { stdio: ["ignore", "pipe", "pipe"] }
);

let stdout = "";
let stderr = "";
sandboxrs.stdout.on("data", (chunk) => (stdout += chunk));
sandboxrs.stderr.on("data", (chunk) => (stderr += chunk));
sandboxrs.on("close", (code) => {
  if (code !== 0) {
    console.error(stderr);
    process.exit(code || 1);
  }
  const result = JSON.parse(stdout);
  process.stdout.write(result.stdout);
  process.exit(result.exit_code || 0);
});
