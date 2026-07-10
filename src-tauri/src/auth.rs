/// RDP 凭证管理：加密存储密码，实现无密码体验
/// 方案：用户首次输入密码后加密存储在本地，后续自动注入

use anyhow::{Context, Result};
use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// 认证凭证类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CredentialType {
    Password(String),   // Windows 本地密码
    Pin(String),        // Windows PIN (需转为密码认证)
    SmartCard,          // 智能卡
    SavedSession,       // 已保存的会话凭证
}

/// 存储的凭证记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialEntry {
    pub target_ip: String,
    pub username: String,
    pub encrypted_password: String,  // base64(AES-GCM 加密)
    pub credential_type: String,     // "password" | "pin" | "saved"
    pub display_name: String,
    pub created_at: String,
}

/// 认证管理器
pub struct AuthManager {
    credentials: Mutex<HashMap<String, CredentialEntry>>,
    storage_path: PathBuf,
    cipher: Aes256Gcm,
}

impl AuthManager {
    pub fn new() -> Result<Self> {
        // 使用机器级标识生成密钥
        let machine_key = get_machine_key();
        let key = Key::<Aes256Gcm>::from_slice(&machine_key);
        let cipher = Aes256Gcm::new(key);

        let storage_path = get_storage_path()?;
        let credentials = Self::load_credentials(&storage_path)?;

        Ok(Self {
            credentials: Mutex::new(credentials),
            storage_path,
            cipher,
        })
    }

    /// 保存凭证
    pub fn save_credential(
        &self,
        target_ip: &str,
        username: &str,
        password: &str,
        credential_type: &str,
        display_name: &str,
    ) -> Result<()> {
        // 加密密码
        let nonce = generate_nonce();
        let ciphertext = self.cipher
            .encrypt(&nonce, password.as_bytes())
            .context("加密密码失败")?;

        // 打包: nonce + ciphertext
        let mut packed = nonce.to_vec();
        packed.extend_from_slice(&ciphertext);
        let encrypted = BASE64.encode(&packed);

        let entry = CredentialEntry {
            target_ip: target_ip.to_string(),
            username: username.to_string(),
            encrypted_password: encrypted,
            credential_type: credential_type.to_string(),
            display_name: display_name.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        self.credentials.lock().unwrap()
            .insert(target_ip.to_string(), entry);
        self.save_credentials()?;

        Ok(())
    }

    /// 获取解密后的密码
    pub fn get_password(&self, target_ip: &str) -> Result<Option<String>> {
        let creds = self.credentials.lock().unwrap();
        if let Some(entry) = creds.get(target_ip) {
            let packed = BASE64.decode(&entry.encrypted_password)
                .context("Base64 解码失败")?;
            
            if packed.len() < 12 {
                return Err(anyhow::anyhow!("凭证数据损坏"));
            }

            let nonce = Nonce::from_slice(&packed[..12]);
            let ciphertext = &packed[12..];

            let plaintext = self.cipher
                .decrypt(nonce, ciphertext)
                .context("解密密码失败，可能是机器密钥已变更")?;

            Ok(Some(String::from_utf8(plaintext)?))
        } else {
            Ok(None)
        }
    }

    /// 删除凭证
    pub fn delete_credential(&self, target_ip: &str) -> Result<()> {
        self.credentials.lock().unwrap().remove(target_ip);
        self.save_credentials()?;
        Ok(())
    }

    /// 列出所有已保存的凭证
    pub fn list_credentials(&self) -> Vec<CredentialEntry> {
        self.credentials.lock().unwrap()
            .values().cloned().collect()
    }

    /// 启动 RDP 连接 (自动注入凭证实现无密码登录)
    pub fn launch_rdp(&self, target_ip: &str) -> Result<()> {
        let password = self.get_password(target_ip)?
            .ok_or_else(|| anyhow::anyhow!("未找到 {} 的凭证", target_ip))?;

        let creds = self.credentials.lock().unwrap();
        let entry = creds.get(target_ip)
            .ok_or_else(|| anyhow::anyhow!("凭证不存在"))?;

        #[cfg(target_os = "windows")]
        {
            // Windows: 使用 cmdkey 注入凭证，然后启动 mstsc
            use std::process::Command;

            // 注意: cmdkey 需要管理员权限，这里尝试写入 RDP 文件替代
            let rdp_content = format!(
                "full address:s:{}\nusername:s:{}\nprompt for credentials:i:0\nauthentication level:i:0\n",
                target_ip, entry.username
            );

            let rdp_path = std::env::temp_dir().join("vnt_remote.rdp");
            std::fs::write(&rdp_path, rdp_content)?;

            // 通过 cmdkey 添加凭据 (临时)
            Command::new("cmdkey")
                .arg(format!("/generic:TERMSRV/{}", target_ip))
                .arg(format!("/user:{}", entry.username))
                .arg(format!("/pass:{}", password))
                .output()
                .ok();

            // 启动 mstsc
            open::that(rdp_path.to_str().unwrap())?;
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: 使用 Microsoft Remote Desktop 或 VNC
            // 将密码写入钥匙串
            use std::process::Command;
            
            Command::new("security")
                .args([
                    "add-internet-password",
                    "-a", &entry.username,
                    "-s", target_ip,
                    "-w", &password,
                    "-r", "rdp ",
                    "-U",
                ])
                .output()
                .ok();

            // 尝试打开微软远程桌面或 VNC
            open::that(format!("vnc://{}:{}@{}", entry.username, password, target_ip))?;
        }

        Ok(())
    }

    /// 获取 RDP 文件内容（用于 Windows 无密码连接）
    pub fn get_rdp_file_content(&self, target_ip: &str) -> Result<String> {
        let password = self.get_password(target_ip)?
            .ok_or_else(|| anyhow::anyhow!("未找到凭证"))?;

        let creds = self.credentials.lock().unwrap();
        let entry = creds.get(target_ip)
            .ok_or_else(|| anyhow::anyhow!("凭证不存在"))?;

        Ok(format!(
            "full address:s:{}\nusername:s:{}\n" \
            "prompt for credentials:i:0\nauthentication level:i:0\n" \
            "redirectclipboard:i:1\nredirectdrives:i:1\n" \
            "redirectprinters:i:0\nredirectsmartcards:i:0\n" \
            "devicestoredirect:s:*\n" \
            "drivestoredirect:s:*\n" \
            "session bpp:i:32\n" \
            "winposstr:s:0,1,0,0,1920,1080\n" \
            "compression:i:1\n" \
            "keyboardhook:i:2\n" \
            "audiocapturemode:i:0\n" \
            "videoplaybackmode:i:1\n" \
            "connection type:i:2\n" \
            "networkautodetect:i:1\n" \
            "bandwidthautodetect:i:1\n" \
            "enablecredsspsupport:i:0\n" \
            "authentication level:i:0\n",
            target_ip, entry.username
        ))
    }

    fn save_credentials(&self) -> Result<()> {
        let creds = self.credentials.lock().unwrap();
        let json = serde_json::to_string_pretty(&*creds)?;
        std::fs::write(&self.storage_path, json)?;
        Ok(())
    }

    fn load_credentials(path: &PathBuf) -> Result<HashMap<String, CredentialEntry>> {
        if path.exists() {
            let data = std::fs::read_to_string(path)?;
            Ok(serde_json::from_str(&data).unwrap_or_default())
        } else {
            Ok(HashMap::new())
        }
    }
}

/// 生成 12 字节随机 nonce
fn generate_nonce() -> Nonce {
    let mut nonce = [0u8; 12];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    *Nonce::from_slice(&nonce)
}

/// 获取机器级唯一密钥
fn get_machine_key() -> [u8; 32] {
    let machine_id = get_machine_id();
    let mut hasher = Sha256::new();
    hasher.update(b"VNTRemote-AES-Key-V1:");
    hasher.update(machine_id.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// 获取机器标识
fn get_machine_id() -> String {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("wmic")
            .args(["csproduct", "get", "UUID"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let line = line.trim();
                if !line.is_empty() && line != "UUID" {
                    return line.to_string();
                }
            }
        }
    }

    // 备选方案：机器 hostname + OS 信息
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".into());
    
    let os = if cfg!(target_os = "windows") { "win" }
        else if cfg!(target_os = "macos") { "mac" }
        else { "linux" };

    format!("{}-{}", hostname, os)
}

fn get_storage_path() -> Result<PathBuf> {
    let app_dir = dirs_next::config_dir()
        .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?
        .join("vnt-remote");
    
    std::fs::create_dir_all(&app_dir)?;
    Ok(app_dir.join("credentials.json"))
}
