use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::TcpStream;
use std::time::Duration;
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// 发现的远程设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDevice {
    pub virtual_ip: String,
    pub hostname: String,
    pub os: String,           // windows / macos / linux
    pub rtt_ms: u64,
    pub online: bool,
    pub vnt_remote_port: u16, // VNTRemote 服务监听端口
    pub services: Vec<String>, // 可用服务: "rdp", "vnc", "file_transfer", "screen_off"
}

/// 设备发现管理器
pub struct DeviceDiscovery {
    devices: Arc<Mutex<HashMap<String, RemoteDevice>>>,
}

impl DeviceDiscovery {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 扫描 VNT 虚拟子网 (10.26.0.0/24)
    pub async fn scan_subnet(&self) -> Result<Vec<RemoteDevice>> {
        let mut devices = Vec::new();
        let base_ip = "10.26.0";
        let port = 9527u16; // VNTRemote 服务监听端口

        // 并发扫描 1-254
        let mut handles = Vec::new();
        for i in 1..=254 {
            let ip = format!("{}.{}", base_ip, i);
            handles.push(tokio::spawn(async move {
                check_device(&ip, port).await
            }));
        }

        for handle in handles {
            if let Ok(Some(device)) = handle.await {
                devices.push(device);
            }
        }

        // 更新缓存
        let mut cache = self.devices.lock().await;
        for device in &devices {
            cache.insert(device.virtual_ip.clone(), device.clone());
        }

        Ok(devices)
    }

    /// 获取缓存的设备列表
    pub async fn get_cached_devices(&self) -> Vec<RemoteDevice> {
        self.devices.lock().await.values().cloned().collect()
    }

    /// 发送远程熄屏命令
    pub async fn send_screen_off(&self, target_ip: &str) -> Result<String> {
        let addr = format!("{}:9527", target_ip);
        let mut stream = TcpStream::connect_timeout(
            &addr.parse()?,
            Duration::from_secs(5),
        )?;
        
        use std::io::Write;
        stream.write_all(b"SCREEN_OFF\n")?;
        
        let mut response = String::new();
        use std::io::Read;
        stream.read_to_string(&mut response)?;
        
        Ok(response.trim().to_string())
    }

    /// 发送唤醒命令 (远程开机/唤醒屏幕)
    pub async fn send_wake_up(&self, target_ip: &str) -> Result<String> {
        let addr = format!("{}:9527", target_ip);
        let mut stream = TcpStream::connect_timeout(
            &addr.parse()?,
            Duration::from_secs(5),
        )?;
        
        use std::io::Write;
        stream.write_all(b"WAKE_DISPLAY\n")?;
        
        let mut response = String::new();
        use std::io::Read;
        stream.read_to_string(&mut response)?;
        
        Ok(response.trim().to_string())
    }

    /// 获取设备系统信息
    pub async fn get_device_info(&self, target_ip: &str) -> Result<RemoteDevice> {
        let addr = format!("{}:9527", target_ip);
        let mut stream = TcpStream::connect_timeout(
            &addr.parse()?,
            Duration::from_secs(3),
        )?;
        
        use std::io::Write;
        stream.write_all(b"DEVICE_INFO\n")?;
        
        let mut response = String::new();
        use std::io::Read;
        stream.read_to_string(&mut response)?;
        
        let device: RemoteDevice = serde_json::from_str(&response)?;
        Ok(device)
    }
}

async fn check_device(ip: &str, port: u16) -> Option<RemoteDevice> {
    // 先尝试 TCP 连接 VNTRemote 服务端口
    let addr = format!("{}:{}", ip, port);
    if let Ok(mut stream) = TcpStream::connect_timeout(&addr.parse().ok()?, Duration::from_millis(300)) {
        use std::io::Write;
        stream.write_all(b"DEVICE_INFO\n").ok()?;
        let mut response = String::new();
        use std::io::Read;
        stream.read_to_string(&mut response).ok()?;
        if let Ok(device) = serde_json::from_str::<RemoteDevice>(&response) {
            return Some(device);
        }
    }
    None
}
