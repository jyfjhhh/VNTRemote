use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::process::{Command, Child, Stdio};
use std::io::{BufRead, BufReader};
use tokio::sync::watch;

/// VNT 连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VntConfig {
    pub token: String,
    pub server: String,
    pub virtual_ip: String,
    pub device_name: String,
    pub password: Option<String>,
    pub use_encryption: bool,
    pub protocol: String, // "udp" | "tcp" | "ws"
}

impl Default for VntConfig {
    fn default() -> Self {
        Self {
            token: String::new(),
            server: "vnt.rustvnt.com:29872".into(),
            virtual_ip: "10.26.0.100".into(),
            device_name: hostname(),
            password: None,
            use_encryption: true,
            protocol: "udp".into(),
        }
    }
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

/// VNT 连接状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VntStatus {
    pub connected: bool,
    pub mode: String,         // "p2p" | "relay" | "disconnected"
    pub virtual_ip: String,
    pub peers: Vec<VntPeer>,
    pub uptime_secs: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// VNT 网络中的对端设备
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VntPeer {
    pub id: String,
    pub name: String,
    pub virtual_ip: String,
    pub public_ip: Option<String>,
    pub is_p2p: bool,
    pub rtt_ms: Option<u64>,
    pub status: String,
}

/// VNT 子进程管理器
pub struct VntManager {
    config: Mutex<VntConfig>,
    child: Mutex<Option<Child>>,
    status_tx: watch::Sender<VntStatus>,
    status_rx: watch::Receiver<VntStatus>,
}

impl VntManager {
    pub fn new() -> Self {
        let initial_status = VntStatus {
            connected: false,
            mode: "disconnected".into(),
            virtual_ip: String::new(),
            peers: Vec::new(),
            uptime_secs: 0,
            rx_bytes: 0,
            tx_bytes: 0,
        };
        let (tx, rx) = watch::channel(initial_status);
        Self {
            config: Mutex::new(VntConfig::default()),
            child: Mutex::new(None),
            status_tx: tx,
            status_rx: rx,
        }
    }

    pub fn status_rx(&self) -> watch::Receiver<VntStatus> {
        self.status_rx.clone()
    }

    /// 启动 VNT 连接
    pub fn start(&self, config: VntConfig) -> Result<()> {
        let mut child_guard = self.child.lock().unwrap();
        if child_guard.is_some() {
            return Ok(()); // 已经在运行
        }

        *self.config.lock().unwrap() = config.clone();

        // 构建 VNT CLI 命令
        let mut cmd = Command::new(find_vnt_binary());
        cmd.arg("-k").arg(&config.token)
            .arg("-s").arg(&config.server)
            .arg("--ip").arg(&config.virtual_ip)
            .arg("-n").arg(&config.device_name)
            .arg("--cmd"); // JSON 输出模式

        if config.use_encryption {
            if let Some(pwd) = &config.password {
                cmd.arg("-w").arg(pwd);
                cmd.arg("-W");
            }
        }

        match config.protocol.as_str() {
            "tcp" => { cmd.arg("--tcp"); }
            "ws" => { cmd.arg("--ws"); }
            _ => {} // udp 默认
        }

        // 管理员权限提示
        #[cfg(target_os = "windows")]
        {
            // Windows 需要管理员权限创建虚拟网卡
            // 这里通过 VNT 自身提权或外部提权
        }

        let child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("无法启动 VNT CLI，请确认已安装 vnt-cli")?;

        // 启动异步读取 stdout
        let stdout = child.stdout.take().unwrap();
        let status_tx = self.status_tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if let Ok(status) = serde_json::from_str::<VntCliOutput>(&line) {
                        let _ = status_tx.send(status.into());
                    }
                }
            }
        });

        *child_guard = Some(child);
        Ok(())
    }

    /// 停止 VNT 连接
    pub fn stop(&self) -> Result<()> {
        let mut child_guard = self.child.lock().unwrap();
        if let Some(mut child) = child_guard.take() {
            child.kill().ok();
            child.wait().ok();
        }
        let disconnected = VntStatus {
            connected: false,
            mode: "disconnected".into(),
            virtual_ip: String::new(),
            peers: Vec::new(),
            uptime_secs: 0,
            rx_bytes: 0,
            tx_bytes: 0,
        };
        self.status_tx.send(disconnected).ok();
        Ok(())
    }

    /// 获取当前配置
    pub fn get_config(&self) -> VntConfig {
        self.config.lock().unwrap().clone()
    }

    /// 更新配置
    pub fn update_config(&self, config: VntConfig) -> Result<()> {
        self.stop()?;
        self.config.lock().unwrap().clone_from(&config);
        // 保存到持久化存储由上层管理
        Ok(())
    }
}

/// VNT CLI JSON 输出格式
#[derive(Debug, Deserialize)]
struct VntCliOutput {
    #[serde(default)]
    pub r#type: String,
    pub ip: Option<String>,
    pub peers: Option<Vec<VntCliPeer>>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VntCliPeer {
    pub id: Option<String>,
    pub name: Option<String>,
    pub ip: Option<String>,
    pub p2p: Option<bool>,
    pub rtt: Option<u64>,
    pub status: Option<String>,
}

impl From<VntCliOutput> for VntStatus {
    fn from(output: VntCliOutput) -> Self {
        let peers = output.peers.unwrap_or_default()
            .into_iter()
            .map(|p| VntPeer {
                id: p.id.unwrap_or_default(),
                name: p.name.unwrap_or_default(),
                virtual_ip: p.ip.unwrap_or_default(),
                public_ip: None,
                is_p2p: p.p2p.unwrap_or(false),
                rtt_ms: p.rtt,
                status: p.status.unwrap_or("unknown".into()),
            })
            .collect();

        VntStatus {
            connected: output.status.as_deref() == Some("connected"),
            mode: if output.status.as_deref() == Some("connected") { "p2p".into() } else { "disconnected".into() },
            virtual_ip: output.ip.unwrap_or_default(),
            peers,
            uptime_secs: 0,
            rx_bytes: 0,
            tx_bytes: 0,
        }
    }
}

/// 查找 VNT 可执行文件路径
fn find_vnt_binary() -> String {
    // 先尝试同目录下
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));
    
    if let Some(dir) = exe_dir {
        let candidates = ["vnt-cli", "vnt-cli.exe", "vnt", "vnt.exe"];
        for name in &candidates {
            let path = dir.join(name);
            if path.exists() {
                return path.to_string_lossy().to_string();
            }
        }
    }

    // 再尝试 PATH
    "vnt-cli".into()
}
