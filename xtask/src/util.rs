use crate::config::Config;
use anyhow::Context;
use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use telnet::{Event, Telnet};
use which::which;

const BACKGROUND_TERM_GRACE: Duration = Duration::from_millis(1500);

struct BackgroundRun {
    child: Child,
    started_at: Instant,
    timeout: Duration,
    cmd_debug: String,
    terminated: bool,
}

impl BackgroundRun {
    fn terminate(&mut self) -> anyhow::Result<()> {
        if self.terminated {
            return Ok(());
        }

        if self.child.try_wait()?.is_some() {
            self.terminated = true;
            return Ok(());
        }

        #[cfg(unix)]
        {
            let pgid = self.child.id() as i32;
            signal_process_group(pgid, libc::SIGTERM)?;

            let start = Instant::now();
            while start.elapsed() < BACKGROUND_TERM_GRACE {
                if self.child.try_wait()?.is_some() {
                    self.terminated = true;
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(50));
            }

            signal_process_group(pgid, libc::SIGKILL)?;
        }

        #[cfg(not(unix))]
        {
            self.child.kill()?;
        }

        // Ensure the process is fully reaped.
        let _ = self.child.wait();
        self.terminated = true;
        Ok(())
    }

    fn ensure_alive_within_timeout(&mut self) -> anyhow::Result<()> {
        if self.terminated {
            return Ok(());
        }

        if let Some(status) = self.child.try_wait()? {
            self.terminated = true;
            if status.success() {
                eprintln!("[ INFO ] background command exited successfully: {}", self.cmd_debug);
                return Ok(());
            }
            return Err(anyhow::anyhow!(
                "[ ERROR ] command failed with status {}: {}",
                status,
                self.cmd_debug
            ));
        }

        if self.started_at.elapsed() >= self.timeout {
            self.terminate()?;
            return Err(anyhow::anyhow!(
                "[ ERROR ] command timed out after {} seconds: {}",
                self.timeout.as_secs(),
                self.cmd_debug
            ));
        }

        Ok(())
    }
}

impl Drop for BackgroundRun {
    fn drop(&mut self) {
        if let Err(e) = self.terminate() {
            eprintln!("[ WARN ] failed to terminate background run ({}): {e:#}", self.cmd_debug);
        }
    }
}

#[cfg(unix)]
fn signal_process_group(pgid: i32, sig: i32) -> io::Result<()> {
    let rc = unsafe { libc::kill(-pgid, sig) };
    if rc == 0 {
        return Ok(());
    }

    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }

    Err(err)
}

#[cfg(unix)]
fn signal_pid(pid: i32, sig: i32) -> io::Result<()> {
    let rc = unsafe { libc::kill(pid, sig) };
    if rc == 0 {
        return Ok(());
    }

    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }

    Err(err)
}

#[cfg(unix)]
fn pid_exists(pid: i32) -> io::Result<bool> {
    let rc = unsafe { libc::kill(pid, 0) };
    if rc == 0 {
        return Ok(true);
    }

    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(false);
    }

    // EPERM means the process exists but we lack permission to signal it.
    if err.raw_os_error() == Some(libc::EPERM) {
        return Ok(true);
    }

    Err(err)
}

#[cfg(unix)]
fn terminate_pid(pid: i32) -> io::Result<()> {
    signal_pid(pid, libc::SIGTERM)?;

    let start = Instant::now();
    while start.elapsed() < BACKGROUND_TERM_GRACE {
        if !pid_exists(pid)? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    signal_pid(pid, libc::SIGKILL)?;
    Ok(())
}

#[cfg(unix)]
fn is_qemu_pid(pid: i32) -> bool {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline"))
        .map(|data| String::from_utf8_lossy(&data).replace('\0', " "))
        .unwrap_or_default();

    let mut text = String::new();
    text.push_str(&comm);
    text.push(' ');
    text.push_str(&cmdline);
    text.to_ascii_lowercase().contains("qemu")
}

#[cfg(unix)]
fn kill_qemu_on_port_via_lsof(port: u16) -> anyhow::Result<()> {
    if which("lsof").is_err() {
        eprintln!("[ WARN ] `lsof` not found, skip pre-exec qemu cleanup on tcp:{}", port);
        return Ok(());
    }

    let target = format!("-iTCP:{port}");
    let output = Command::new("lsof")
        .args(["-t", &target])
        .output()
        .with_context(|| format!("[ ERROR ] failed to run lsof for tcp:{}", port))?;

    // lsof returns non-zero when there is no match; treat as empty result.
    if output.stdout.is_empty() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut pids = BTreeSet::new();
    for line in stdout.lines() {
        if let Ok(pid) = line.trim().parse::<i32>() {
            if pid > 0 {
                pids.insert(pid);
            }
        }
    }

    if pids.is_empty() {
        return Ok(());
    }

    let mut killed = Vec::new();
    let mut skipped = Vec::new();

    for pid in pids {
        if is_qemu_pid(pid) {
            terminate_pid(pid)
                .with_context(|| format!("[ ERROR ] failed to terminate qemu pid {}", pid))?;
            killed.push(pid);
        } else {
            skipped.push(pid);
        }
    }

    if !killed.is_empty() {
        eprintln!("[ INFO ] pre-exec cleaned qemu on tcp:{} (pids={:?})", port, killed);
    }

    if !skipped.is_empty() {
        eprintln!(
            "[ WARN ] tcp:{} is also occupied by non-qemu pids (not killed): {:?}",
            port, skipped
        );
    }

    Ok(())
}

#[cfg(not(unix))]
fn kill_qemu_on_port_via_lsof(_port: u16) -> anyhow::Result<()> {
    Ok(())
}

pub fn run(cmd: &mut Command) -> anyhow::Result<()> {
    run_with_timeout(cmd, None)
}

pub fn run_with_timeout(cmd: &mut Command, timeout: Option<u64>) -> anyhow::Result<()> {
    eprintln!("[ INFO ] Running: $ {:?}", cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("[ ERROR ] Failed to start command {:?}: {}", cmd, e))?;

    if let Some(timeout_secs) = timeout {
        let start = std::time::Instant::now();
        loop {
            match child.try_wait()? {
                Some(status) => {
                    if !status.success() {
                        return Err(anyhow::anyhow!(
                            "[ ERROR ] command failed with status {}: {:?}",
                            status,
                            cmd
                        ));
                    }
                    return Ok(());
                }
                None => {
                    if start.elapsed().as_secs() >= timeout_secs {
                        child.kill()?;
                        return Err(anyhow::anyhow!(
                            "[ ERROR ] command timed out after {} seconds: {:?}",
                            timeout_secs,
                            cmd
                        ));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    } else {
        let status = child.wait()?;
        if !status.success() {
            return Err(anyhow::anyhow!(
                "[ ERROR ] command failed with status {}: {:?}",
                status,
                cmd
            ));
        }
        Ok(())
    }
}

pub fn download(url: &str, dest: &Path) -> anyhow::Result<()> {
    if dest.exists() {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let is_bz2 = url.ends_with(".bz2");
    let download_path = if is_bz2 { dest.with_added_extension("bz2") } else { dest.to_path_buf() };

    eprintln!("[ INFO ] Downloading {} -> {}", url, download_path.display());
    let mut cmd = Command::new("curl");
    cmd.args(["-L", "-o", download_path.to_str().unwrap(), url]);
    run(&mut cmd)?;

    if is_bz2 {
        eprintln!("[ INFO ] Decompressing {}...", download_path.display());
        let mut cmd = Command::new("bunzip2");
        cmd.arg(download_path.to_str().unwrap());
        run(&mut cmd)?;
    }

    Ok(())
}

pub fn objdump(cfg: &Config) -> anyhow::Result<()> {
    let is_uefi = cfg.system.bootloader == crate::arch::Bootloader::Uefi;
    let target = cfg.system.arch.target_triple();
    let kernel_name = if is_uefi { "kernel.efi" } else { "kernel" };
    let elf =
        PathBuf::from("target").join(target).join(cfg.system.profile.as_str()).join(kernel_name);
    let bin = cfg.system.arch.llvm_tool("objdump");
    let tool = which(&bin)
        .or_else(|_| which("rust-objdump"))
        .or_else(|_| which("llvm-objdump"))
        .map_err(|_| anyhow::anyhow!("[ ERROR ] install {} or llvm-objdump first", bin))?;
    let mut cmd = Command::new(tool);
    cmd.args(["-d", "--all-headers", "--source", elf.to_str().unwrap()]);
    run(&mut cmd)
}

pub fn size(cfg: &Config) -> anyhow::Result<()> {
    let is_uefi = cfg.system.bootloader == crate::arch::Bootloader::Uefi;
    let target = cfg.system.arch.target_triple();
    let kernel_name = if is_uefi { "kernel.efi" } else { "kernel" };
    let elf =
        PathBuf::from("target").join(target).join(cfg.system.profile.as_str()).join(kernel_name);
    let bin = cfg.system.arch.llvm_tool("size");
    let tool = which(&bin)
        .or_else(|_| which("llvm-size"))
        .map_err(|_| anyhow::anyhow!("[ ERROR ] install {} or llvm-size first", bin))?;
    let mut cmd = Command::new(tool);
    cmd.args(["-A", elf.to_str().unwrap()]);
    run(&mut cmd)
}

pub fn strip(cfg: &Config, file: &Path) -> anyhow::Result<()> {
    let bin = cfg.system.arch.llvm_tool("objcopy");
    let tool = which(&bin)
        .or_else(|_| which("rust-objcopy"))
        .or_else(|_| which("llvm-objcopy"))
        .map_err(|_| anyhow::anyhow!("[ ERROR ] install {} or llvm-objcopy first", bin))?;
    let mut cmd = Command::new(tool);
    cmd.arg("--strip-all").arg(file);
    run(&mut cmd)
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut iter = s.chars().peekable();
    while let Some(ch) = iter.next() {
        if ch == '\u{1b}' {
            if iter.peek() == Some(&'[') {
                let _ = iter.next();
                for c in iter.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

fn tail_for_prompt(s: &str, max_chars: usize) -> String {
    let collected: Vec<char> = s.chars().rev().take(max_chars).collect();
    collected.into_iter().rev().collect()
}

fn looks_like_shell_prompt(buf: &str) -> bool {
    let sanitized = strip_ansi(buf);
    let tail = tail_for_prompt(&sanitized, 1024);
    let mut lines = tail.lines().rev();
    let last_line = lines.next().unwrap_or("").trim_end_matches('\r');
    let prompt_line = if last_line.is_empty() { lines.next().unwrap_or("") } else { last_line };
    let prompt_line = prompt_line.trim_end_matches('\r');

    prompt_line.ends_with("# ")
        || prompt_line.ends_with("#")
        || prompt_line.ends_with("$ ")
        || prompt_line.ends_with("$")
}

fn wait_for_prompt(
    conn: &mut Telnet,
    rolling: &mut String,
    timeout: Duration,
    background_run: &mut Option<BackgroundRun>,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();

    loop {
        if let Some(bg) = background_run.as_mut() {
            bg.ensure_alive_within_timeout()?;
        }

        match conn.read_timeout(Duration::from_millis(100)) {
            Ok(Event::Data(payload)) => {
                if payload.is_empty() {
                    continue;
                }

                lock.write_all(&payload)?;
                lock.flush()?;

                rolling.push_str(&String::from_utf8_lossy(&payload));
                if rolling.chars().count() > 16 * 1024 {
                    *rolling = tail_for_prompt(rolling, 8 * 1024);
                }

                if looks_like_shell_prompt(rolling) {
                    // Give terminal-side prompt negotiation (e.g. CSI 6n/CPR) a brief
                    // settle window so scripted input is not consumed by the probe parser.
                    let settle_for = Duration::from_millis(180);
                    let settle_start = Instant::now();
                    let mut stable = true;
                    while settle_start.elapsed() < settle_for {
                        match conn.read_timeout(Duration::from_millis(30)) {
                            Ok(Event::Data(extra)) => {
                                if extra.is_empty() {
                                    continue;
                                }
                                stable = false;
                                lock.write_all(&extra)?;
                                lock.flush()?;
                                rolling.push_str(&String::from_utf8_lossy(&extra));
                                if rolling.chars().count() > 16 * 1024 {
                                    *rolling = tail_for_prompt(rolling, 8 * 1024);
                                }
                            }
                            Ok(Event::TimedOut) => {}
                            Ok(Event::Error(msg)) => {
                                return Err(anyhow::anyhow!(
                                    "[ ERROR ] telnet stream error: {}",
                                    msg
                                ));
                            }
                            Ok(_) => {}
                            Err(e) => {
                                return Err(anyhow::anyhow!(
                                    "[ ERROR ] failed to read serial stream: {}",
                                    e
                                ));
                            }
                        }
                    }
                    if stable || looks_like_shell_prompt(rolling) {
                        return Ok(());
                    }
                }
            }
            Ok(Event::TimedOut) => {}
            Ok(Event::Error(msg)) => {
                return Err(anyhow::anyhow!("[ ERROR ] telnet stream error: {}", msg));
            }
            Ok(_) => {}
            Err(e) => {
                return Err(anyhow::anyhow!("[ ERROR ] failed to read serial stream: {}", e));
            }
        }

        if start.elapsed() >= timeout {
            let tail = tail_for_prompt(rolling, 400);
            return Err(anyhow::anyhow!(
                "[ ERROR ] timed out waiting for shell prompt after {}s\n[ DEBUG ] serial tail:\n{}",
                timeout.as_secs(),
                tail
            ));
        }

        std::thread::sleep(Duration::from_millis(20));
    }
}

fn telnet_write_all(conn: &mut Telnet, data: &[u8]) -> anyhow::Result<()> {
    let mut offset = 0;
    while offset < data.len() {
        let n = conn.write(&data[offset..])?;
        if n == 0 {
            return Err(anyhow::anyhow!("[ ERROR ] serial write returned 0 bytes"));
        }
        offset += n;
    }
    Ok(())
}

pub fn attach(cfg: &Config, port: Option<u16>) -> anyhow::Result<()> {
    let port = port.unwrap_or(cfg.qemu.serial_port.unwrap_or(5555));
    let host = "127.0.0.1";
    eprintln!("[ INFO ] Attaching to PCI UART backend at {host}:{port} ...");

    if which("telnet").is_ok() {
        let mut cmd = Command::new("telnet");
        cmd.arg(host).arg(port.to_string());
        return run(&mut cmd);
    }

    if which("nc").is_ok() {
        let mut cmd = Command::new("nc");
        cmd.arg(host).arg(port.to_string());
        return run(&mut cmd);
    }

    if which("ncat").is_ok() {
        let mut cmd = Command::new("ncat");
        cmd.arg(host).arg(port.to_string());
        return run(&mut cmd);
    }

    anyhow::bail!("[ ERROR ] No attach client found. Please install one of: telnet, nc, ncat")
}

pub fn exec(
    cfg: &Config,
    script: &Path,
    port: Option<u16>,
    run_in_background: bool,
    run_log: &Path,
    run_timeout_secs: u64,
    connect_timeout_secs: u64,
    prompt_timeout_secs: u64,
) -> anyhow::Result<()> {
    let port = port.unwrap_or(cfg.qemu.serial_port.unwrap_or(5555));
    if run_in_background {
        kill_qemu_on_port_via_lsof(port)?;
    }

    let script_text = std::fs::read_to_string(script)
        .with_context(|| format!("[ ERROR ] failed to read script: {}", script.display()))?;

    let commands: Vec<String> = script_text
        .lines()
        .map(|line| line.trim_end_matches('\r').trim())
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("#!"))
        .filter(|line| !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect();

    if commands.is_empty() {
        return Err(anyhow::anyhow!(
            "[ ERROR ] script contains no executable lines: {}",
            script.display()
        ));
    }

    let host = "127.0.0.1";
    let run_timeout = Duration::from_secs(run_timeout_secs.max(1));
    let connect_timeout = Duration::from_secs(connect_timeout_secs.max(1));
    let prompt_timeout = Duration::from_secs(prompt_timeout_secs.max(1));
    let mut background_run: Option<BackgroundRun> = None;

    if run_in_background {
        let log_path = if run_log.is_absolute() {
            run_log.to_path_buf()
        } else {
            std::env::current_dir()?.join(run_log)
        };
        if let Some(parent) = log_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let mut cmd = crate::qemu::qemu_cmd(cfg)?;
        let log = OpenOptions::new().create(true).write(true).truncate(true).open(&log_path)?;
        let log_err = log.try_clone()?;
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::from(log));
        cmd.stderr(Stdio::from(log_err));

        #[cfg(unix)]
        unsafe {
            cmd.pre_exec(|| {
                let rc = libc::setpgid(0, 0);
                if rc == 0 {
                    Ok(())
                } else {
                    Err(io::Error::last_os_error())
                }
            });
        }

        let cmd_debug = format!("{:?}", cmd);

        eprintln!(
            "[ INFO ] Starting `run` in background and logging to {} (timeout={}s)",
            log_path.display(),
            run_timeout.as_secs()
        );
        let child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("[ ERROR ] Failed to start command {:?}: {}", cmd, e))?;
        eprintln!("[ INFO ] Background run started (pid={})", child.id());
        background_run = Some(BackgroundRun {
            child,
            started_at: Instant::now(),
            timeout: run_timeout,
            cmd_debug,
            terminated: false,
        });
    }

    eprintln!(
        "[ INFO ] Connecting to serial endpoint {}:{} (script: {})",
        host,
        port,
        script.display()
    );

    let connect_start = Instant::now();
    let mut conn = loop {
        if let Some(bg) = background_run.as_mut() {
            bg.ensure_alive_within_timeout()?;
        }

        match Telnet::connect((host, port), 4096) {
            Ok(c) => break c,
            Err(e) => {
                if connect_start.elapsed() >= connect_timeout {
                    return Err(anyhow::anyhow!(
                        "[ ERROR ] failed to connect {}:{} within {}s: {}",
                        host,
                        port,
                        connect_timeout.as_secs(),
                        e
                    ));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    };
    let mut rolling = String::new();

    eprintln!("[ INFO ] Waiting for initial shell prompt...");
    wait_for_prompt(&mut conn, &mut rolling, prompt_timeout, &mut background_run)?;
    for (idx, line) in commands.iter().enumerate() {
        if let Some(bg) = background_run.as_mut() {
            bg.ensure_alive_within_timeout()?;
        }

        eprintln!("\n[ EXEC ] ({}/{}) {}", idx + 1, commands.len(), line);
        telnet_write_all(&mut conn, line.as_bytes())?;
        telnet_write_all(&mut conn, b"\r\n")?;
        wait_for_prompt(&mut conn, &mut rolling, prompt_timeout, &mut background_run)?;
    }

    eprintln!("\n[ INFO ] Script execution finished: {}", script.display());

    if let Some(mut bg) = background_run.take() {
        bg.terminate()?;
    }

    Ok(())
}
