use std::process::Command;

fn main() {
    let output = Command::new("git").args(&["rev-parse", "--short", "HEAD"]).output();

    let git_hash = match output {
        Ok(o) if o.status.success() => String::from_utf8(o.stdout).unwrap(),
        _ => "unknown".to_string(),
    };

    println!("cargo:rustc-env=KERNEL_GIT_HASH={}", git_hash.trim());
    let now = chrono::Utc::now();
    let timestamp = now.format("%a %b %d %H:%M:%S UTC %Y").to_string();

    println!("cargo:rustc-env=KERNEL_BUILD_TIME={}", timestamp);
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");
}
