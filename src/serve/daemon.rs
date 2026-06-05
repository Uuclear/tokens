use crate::paths::{default_config_dir, default_db_path};
use crate::serve::theme::UiTheme;
use anyhow::{bail, Context, Result};
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const PID_FILE: &str = "serve.pid";
const PORT_FILE: &str = "serve.port";

pub fn pid_path() -> PathBuf {
    default_config_dir().join(PID_FILE)
}

pub fn port_path() -> PathBuf {
    default_config_dir().join(PORT_FILE)
}

pub fn is_running() -> Result<bool> {
    let Some(pid) = read_pid()? else {
        return Ok(false);
    };
    Ok(process_alive(pid))
}

pub fn start_background(host: IpAddr, port: u16, ui: UiTheme) -> Result<()> {
    if is_running()? {
        bail!(
            "tokens serve 已在运行 (pid {})。使用 tokens serve --down 停止。",
            read_pid()?.unwrap_or(0)
        );
    }

    let exe = std::env::current_exe().context("current_exe")?;
    let db = default_db_path();
    let mut cmd = Command::new(&exe);
    cmd.arg("serve")
        .arg("--foreground")
        .arg("--host")
        .arg(host.to_string())
        .arg("--port")
        .arg(port.to_string())
        .arg("--db")
        .arg(&db);
    match ui {
        UiTheme::Default => {}
        UiTheme::Pixel => {
            cmd.arg("--pixel");
        }
        UiTheme::Terminal => {
            cmd.arg("--terminal");
        }
        UiTheme::Ink => {
            cmd.arg("--ink");
        }
        UiTheme::Paper => {
            cmd.arg("--paper");
        }
    }
    cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }

    #[cfg(not(windows))]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let child = cmd.spawn().context("spawn tokens serve --foreground")?;
    let pid = child.id();
    write_pid(pid)?;
    write_port(port)?;
    println!("tokens serve 已在后台启动 (pid {pid}, UI: {})", ui.label());
    let addr = format!("{host}:{port}");
    println!("监控页面: http://{addr}/");
    if host.is_unspecified() {
        println!("局域网/公网访问请使用本机 IP，例如 http://<你的IP>:{port}/");
    }
    Ok(())
}

pub fn stop() -> Result<()> {
    let Some(pid) = read_pid()? else {
        println!("tokens serve 未在运行。");
        return Ok(());
    };
    if !process_alive(pid) {
        cleanup_files()?;
        println!("tokens serve 未在运行（已清理过期 pid）。");
        return Ok(());
    }
    kill_process(pid)?;
    cleanup_files()?;
    println!("已停止 tokens serve (pid {pid})。");
    Ok(())
}

fn read_pid() -> Result<Option<u32>> {
    let path = pid_path();
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).context("read serve.pid")?;
    let pid: u32 = raw.trim().parse().context("parse serve.pid")?;
    Ok(Some(pid))
}

fn write_pid(pid: u32) -> Result<()> {
    fs::create_dir_all(default_config_dir())?;
    fs::write(pid_path(), pid.to_string())?;
    Ok(())
}

fn write_port(port: u16) -> Result<()> {
    fs::create_dir_all(default_config_dir())?;
    fs::write(port_path(), port.to_string())?;
    Ok(())
}

fn cleanup_files() -> Result<()> {
    let _ = fs::remove_file(pid_path());
    let _ = fs::remove_file(port_path());
    Ok(())
}

fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}")])
            .output();
        match out {
            Ok(o) => String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()),
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
            || Command::new("kill")
                .args(["-0", &pid.to_string()])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
    }
}

fn kill_process(pid: u32) -> Result<()> {
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status()
            .context("taskkill")?;
        if !status.success() {
            bail!("taskkill failed for pid {pid}");
        }
    }
    #[cfg(not(windows))]
    {
        let status = Command::new("kill")
            .arg(pid.to_string())
            .status()
            .context("kill")?;
        if !status.success() {
            bail!("kill failed for pid {pid}");
        }
    }
    Ok(())
}

pub fn cleanup_on_exit() {
    let _ = cleanup_files();
}
