use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// 文件传输项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferItem {
    pub id: String,
    pub file_name: String,
    pub file_size: u64,
    pub direction: String,        // "send" | "receive"
    pub target_ip: String,
    pub progress: f64,            // 0.0 ~ 100.0
    pub status: String,           // "pending" | "transferring" | "completed" | "failed"
    pub error: Option<String>,
    pub speed_bytes_per_sec: u64,
}

/// 文件传输管理器
pub struct FileTransfer {
    transfers: Arc<Mutex<Vec<TransferItem>>>,
    running: Arc<AtomicBool>,
    transfer_port: u16,
}

impl FileTransfer {
    pub fn new() -> Self {
        Self {
            transfers: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(AtomicBool::new(true)),
            transfer_port: 9528,
        }
    }

    /// 发送文件到目标设备
    pub async fn send_file(
        &self,
        target_ip: &str,
        local_path: &str,
    ) -> Result<String> {
        let path = Path::new(local_path);
        if !path.exists() {
            return Err(anyhow::anyhow!("文件不存在: {}", local_path));
        }

        let file_size = std::fs::metadata(local_path)?.len();
        let file_name = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let id = format!("send-{}-{}", target_ip, file_name);

        // 添加传输记录
        {
            let mut transfers = self.transfers.lock().await;
            transfers.push(TransferItem {
                id: id.clone(),
                file_name: file_name.clone(),
                file_size,
                direction: "send".into(),
                target_ip: target_ip.to_string(),
                progress: 0.0,
                status: "pending".into(),
                error: None,
                speed_bytes_per_sec: 0,
            });
        }

        // 在后台执行传输
        let transfers = self.transfers.clone();
        let running = self.running.clone();
        let local_path = local_path.to_string();
        let target_ip = target_ip.to_string();

        tokio::spawn(async move {
            if let Err(e) = do_send_file(&target_ip, &local_path, &id, transfers.clone(), running).await {
                let mut t = transfers.lock().await;
                if let Some(item) = t.iter_mut().find(|x| x.id == id) {
                    item.status = "failed".into();
                    item.error = Some(e.to_string());
                }
            }
        });

        Ok(id)
    }

    /// 获取所有传输记录
    pub async fn get_transfers(&self) -> Vec<TransferItem> {
        self.transfers.lock().await.clone()
    }

    /// 取消传输
    pub async fn cancel_transfer(&self, id: &str) -> Result<()> {
        let mut transfers = self.transfers.lock().await;
        if let Some(item) = transfers.iter_mut().find(|x| x.id == id) {
            item.status = "cancelled".into();
        }
        Ok(())
    }

    /// 启动文件传输服务端（在被控端运行）
    pub fn start_transfer_server(&self, port: u16) -> Result<()> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
            .context("无法绑定文件传输端口")?;
        listener.set_nonblocking(true)?;

        let running = self.running.clone();
        std::thread::spawn(move || {
            listener.set_nonblocking(false).ok();
            for stream in listener.incoming() {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(stream) = stream {
                    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
                    stream.set_write_timeout(Some(Duration::from_secs(30))).ok();
                    handle_transfer_request(stream).ok();
                }
            }
        });

        Ok(())
    }

    /// 停止传输服务器
    pub fn stop_transfer_server(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// 发送文件到目标设备
async fn do_send_file(
    target_ip: &str,
    local_path: &str,
    transfer_id: &str,
    transfers: Arc<Mutex<Vec<TransferItem>>>,
    running: Arc<AtomicBool>,
) -> Result<()> {
    let addr = format!("{}:9528", target_ip);
    let mut stream = TcpStream::connect_timeout(
        &addr.parse()?,
        Duration::from_secs(10),
    )?;

    let path = Path::new(local_path);
    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let file_size = std::fs::metadata(local_path)?.len();

    // 发送协议头: "FILE|filename|filesize\n"
    let header = format!("FILE|{}|{}\n", file_name, file_size);
    stream.write_all(header.as_bytes())?;

    // 等待确认
    let mut ack = [0u8; 4];
    stream.read_exact(&mut ack)?;
    if &ack != b"OK\n" {
        return Err(anyhow::anyhow!("目标设备拒绝接收文件"));
    }

    // 发送文件内容
    let mut file = std::fs::File::open(local_path)?;
    let mut buffer = vec![0u8; 65536];
    let mut sent: u64 = 0;
    let start_time = std::time::Instant::now();

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 { break; }
        stream.write_all(&buffer[..n])?;
        sent += n as u64;

        // 更新进度
        let elapsed = start_time.elapsed().as_secs().max(1);
        let speed = sent / elapsed;

        let mut t = transfers.lock().await;
        if let Some(item) = t.iter_mut().find(|x| x.id == transfer_id) {
            item.progress = (sent as f64 / file_size as f64) * 100.0;
            item.speed_bytes_per_sec = speed;
            if item.status == "cancelled" {
                return Ok(());
            }
        }

        if !running.load(Ordering::Relaxed) {
            return Err(anyhow::anyhow!("传输已停止"));
        }
    }

    // 标记完成
    let mut t = transfers.lock().await;
    if let Some(item) = t.iter_mut().find(|x| x.id == transfer_id) {
        item.progress = 100.0;
        item.status = "completed".into();
    }

    Ok(())
}

fn handle_transfer_request(stream: TcpStream) -> Result<()> {
    use std::io::{BufRead, BufReader, Read, Write};

    // Read header into a local scope so the BufReader borrow drops
    let file_name: String;
    let file_size: u64;
    {
        let mut reader = BufReader::new(&stream);
        let mut header = String::new();
        reader.read_line(&mut header)?;
        let parts: Vec<&str> = header.trim().split('|').collect();
        if parts.len() < 3 || parts[0] != "FILE" {
            return Err(anyhow::anyhow!("bad transfer protocol"));
        }
        file_name = parts[1].to_string();
        file_size = parts[2].parse()?;
    }
    // BufReader is dropped, stream is free

    let download_dir = dirs_next::download_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    let save_path = download_dir.join(&file_name);

    let mut file = std::fs::File::create(&save_path)?;
    let mut buffer = vec![0u8; 65536];
    let mut received: u64 = 0;

    // Write ACK then read file data from the raw stream
    let mut stream = stream;
    stream.write_all(b"OK\n")?;

    while received < file_size {
        let remaining = file_size - received;
        let to_read = buffer.len().min(remaining as usize);
        let n = stream.read(&mut buffer[..to_read])?;
        if n == 0 { break; }
        file.write_all(&buffer[..n])?;
        received += n as u64;
    }

    Ok(())
}
