/// 后台服务模式 - 被控端运行
/// 提供: 设备信息查询、远程熄屏/唤醒、文件传输服务

use anyhow::Result;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::process::Command;

use crate::device_discovery::RemoteDevice;
use crate::file_transfer::FileTransfer;

/// 后台服务管理器
pub struct BackgroundService {
    running: Arc<AtomicBool>,
    command_port: u16,
    file_transfer: Arc<FileTransfer>,
}

impl BackgroundService {
    pub fn new(file_transfer: Arc<FileTransfer>) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(true)),
            command_port: 9527,
            file_transfer,
        }
    }

    /// 启动所有后台服务
    pub fn start(&self) -> Result<()> {
        self.start_command_server()?;
        self.file_transfer.start_transfer_server(9528)?;
        Ok(())
    }

    /// 停止后台服务
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        self.file_transfer.stop_transfer_server();
    }

    /// 命令监听服务 (端口 9527)
    fn start_command_server(&self) -> Result<()> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.command_port))?;
        let running = self.running.clone();

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(stream) = stream {
                    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
                    std::thread::spawn(|| handle_command(stream));
                }
            }
        });

        Ok(())
    }
}

/// 处理远程命令
fn handle_command(mut stream: TcpStream) {
    let command = {
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() {
            return;
        }
        line.trim().to_string()
    };

    let response = match command.as_str() {
        "DEVICE_INFO" => handle_device_info(),
        "SCREEN_OFF" => handle_screen_off(),
        "WAKE_DISPLAY" => handle_wake_display(),
        "LOCK_WORKSTATION" => handle_lock_workstation(),
        "PING" => "PONG\n".to_string(),
        _ => format!("UNKNOWN_COMMAND: {}\n", command),
    };

    let _ = stream.write_all(response.as_bytes());
}

/// 获取本机设备信息
fn handle_device_info() -> String {
    let device = RemoteDevice {
        virtual_ip: get_vnt_ip().unwrap_or_else(|| "10.26.0.0".into()),
        hostname: get_hostname(),
        os: get_os_type(),
        rtt_ms: 0,
        online: true,
        vnt_remote_port: 9527,
        services: vec![
            "rdp".into(),
            "file_transfer".into(),
            "screen_off".into(),
        ],
    };

    serde_json::to_string(&device).unwrap_or_default() + "\n"
}

/// 远程熄屏
fn handle_screen_off() -> String {
    #[cfg(target_os = "windows")]
    {
        Command::new("powershell")
            .args(["-Command",
                "Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class P {[DllImport(\"user32.dll\")]public static extern int SendMessage(int hWnd,int hMsg,int wParam,int lParam);public static void Off(){SendMessage(-1,0x0112,0xF170,2);}}'; [P]::Off()"
            ])
            .output()
            .ok();
        "SCREEN_OFF_OK\n".to_string()
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("pmset")
            .args(["displaysleepnow"])
            .output()
            .ok();
        "SCREEN_OFF_OK\n".to_string()
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xset")
            .args(["dpms", "force", "off"])
            .output()
            .ok();
        "SCREEN_OFF_OK\n".to_string()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "SCREEN_OFF_UNSUPPORTED\n".to_string()
    }
}

/// 唤醒屏幕
fn handle_wake_display() -> String {
    #[cfg(target_os = "windows")]
    {
        Command::new("powershell")
            .args(["-Command",
                "Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class W {[DllImport(\"user32.dll\")]public static extern int SendMessage(int hWnd,int hMsg,int wParam,int lParam);public static void Wake(){SendMessage(-1,0x0112,0xF170,-1);}}'; [W]::Wake()"
            ])
            .output()
            .ok();
        "WAKE_DISPLAY_OK\n".to_string()
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("caffeinate")
            .args(["-u", "-t", "1"])
            .output()
            .ok();
        "WAKE_DISPLAY_OK\n".to_string()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        "WAKE_DISPLAY_UNSUPPORTED\n".to_string()
    }
}

/// 锁定工作站
fn handle_lock_workstation() -> String {
    #[cfg(target_os = "windows")]
    {
        Command::new("rundll32.exe")
            .args(["user32.dll,LockWorkStation"])
            .output()
            .ok();
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("pmset")
            .args(["displaysleepnow"])
            .output()
            .ok();
    }

    "LOCK_OK\n".to_string()
}

fn get_hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

fn get_os_type() -> String {
    if cfg!(target_os = "windows") { "windows".into() }
    else if cfg!(target_os = "macos") { "macos".into() }
    else if cfg!(target_os = "linux") { "linux".into() }
    else { "unknown".into() }
}

fn get_vnt_ip() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args(["-Command",
                "(Get-NetIPAddress -InterfaceAlias 'vnt-tun*' -AddressFamily IPv4).IPAddress"])
            .output()
            .ok()?;
        let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !ip.is_empty() { Some(ip) } else { None }
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ifconfig")
            .args(["vnt-tun"])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(ip) = line.trim()
                .strip_prefix("inet ")
                .and_then(|s| s.split_whitespace().next()) {
                return Some(ip.to_string());
            }
        }
        None
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let output = Command::new("ip")
            .args(["addr", "show", "vnt-tun"])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(ip) = line.trim()
                .strip_prefix("inet ")
                .and_then(|s| s.split('/').next()) {
                return Some(ip.to_string());
            }
        }
        None
    }
}
