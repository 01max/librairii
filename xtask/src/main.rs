mod policy;
use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask failed: {error}");
            ExitCode::FAILURE
        }
    }
}
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(PathBuf::new, Path::to_owned)
}
fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}
fn command(program: &str, args: &[&str]) -> Result {
    eprintln!("+ {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(root())
        .status()?;
    if !status.success() {
        return Err(format!("{program} failed: {status}").into());
    }
    Ok(())
}
fn output(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root())
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}
fn check_tool(name: &str) -> Result {
    let pins: toml::Value = toml::from_str(&std::fs::read_to_string(
        root().join(".config/rust-tools.toml"),
    )?)?;
    let version = pins["tools"][name].as_str().ok_or("tool version missing")?;
    let actual = output(&cargo(), &[name.trim_start_matches("cargo-"), "--version"])?;
    if !actual.split_whitespace().any(|word| word == version) {
        return Err(format!(
            "{name} must be exactly {version}; found {actual}; run cargo xtask tools"
        )
        .into());
    }
    Ok(())
}
fn host() -> Result<String> {
    let version = output("rustc", &["-vV"])?;
    let pin: toml::Value = toml::from_str(&std::fs::read_to_string(
        root().join("rust-toolchain.toml"),
    )?)?;
    let expected = pin["toolchain"]["channel"]
        .as_str()
        .ok_or("missing stable pin")?;
    if !version
        .lines()
        .any(|line| line == format!("release: {expected}"))
    {
        return Err(format!("expected pinned stable Rust {expected}: {version}").into());
    }
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or("missing rustc host")?
        .to_owned();
    let combinations: toml::Value = toml::from_str(&std::fs::read_to_string(
        root().join(".config/rust-checks.toml"),
    )?)?;
    let features = combinations["targets"]
        .get(&host)
        .and_then(toml::Value::as_array)
        .ok_or("unsupported native host")?;
    if !features.is_empty() {
        return Err("update xtask for newly selected shipping features".into());
    }
    eprintln!(
        "Native target {host}; feature set empty/default; other targets remain unverified by this run."
    );
    Ok(host)
}
fn run() -> Result {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.as_slice() {
        [action] if action == "check" => {
            let target = host()?;
            eprintln!("Source {}", output("git", &["rev-parse", "HEAD"])?.trim());
            check_tool("cargo-nextest")?;
            policy::boundaries()?;
            command(&cargo(), &["fmt", "--all", "--", "--check"])?;
            command(
                &cargo(),
                &["build", "--workspace", "--locked", "--target", &target],
            )?;
            command(
                &cargo(),
                &[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--locked",
                    "--target",
                    &target,
                    "--",
                    "-D",
                    "warnings",
                ],
            )?;
            command(
                &cargo(),
                &[
                    "nextest",
                    "run",
                    "--workspace",
                    "--locked",
                    "--target",
                    &target,
                    "--profile",
                    "ci",
                ],
            )?;
            command(
                &cargo(),
                &[
                    "test",
                    "--workspace",
                    "--doc",
                    "--locked",
                    "--target",
                    &target,
                ],
            )?;
            eprintln!(
                "Foundation checks passed on {target}. Product differential/UI/package qualification remains pending implementation and preservation admission."
            );
            Ok(())
        }
        [action] if action == "tools" => {
            let pins: toml::Value = toml::from_str(&std::fs::read_to_string(
                root().join(".config/rust-tools.toml"),
            )?)?;
            for name in ["cargo-nextest", "cargo-deny", "cargo-audit"] {
                let version = pins["tools"][name].as_str().ok_or("tool pin missing")?;
                command(
                    &cargo(),
                    &["install", "--locked", "--version", version, name],
                )?;
            }
            Ok(())
        }
        _ => Err("usage: cargo xtask {check|tools}".into()),
    }
}
