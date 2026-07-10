use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::io::AsyncWriteExt;

/// 版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub version: String,
    pub download_url: String,
    pub release_notes: String,
    pub published_at: String,
    pub file_size: u64,
}

/// 更新进度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProgress {
    pub stage: String,
    pub progress: f64,
    pub message: String,
}

/// 自动更新管理器
pub struct Updater {
    current_version: String,
    progress: Mutex<UpdateProgress>,
}

impl Updater {
    pub fn new() -> Self {
        Self {
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            progress: Mutex::new(UpdateProgress {
                stage: "done".into(),
                progress: 100.0,
                message: "就绪".into(),
            }),
        }
    }

    /// 从 GitHub Releases 检查最新版本
    pub async fn check_for_update(&self) -> Result<Option<ReleaseInfo>> {
        self.set_progress("checking", 0.0, "正在检查更新...");

        let url = "https://api.github.com/repos/jyfjhhh/VNTRemote/releases/latest";
        let client = reqwest::Client::builder()
            .user_agent("VNTRemote/1.0")
            .build()?;

        let response = client.get(url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .context("无法访问 GitHub API")?;

        if !response.status().is_success() {
            self.set_progress("done", 100.0, "检查更新失败");
            return Ok(None);
        }

        let release: GitHubRelease = response.json().await?;
        let latest = release.tag_name.trim_start_matches('v').to_string();

        if compare_versions(&latest, &self.current_version) > 0 {
            let asset = release.assets.first();
            let info = ReleaseInfo {
                version: latest,
                download_url: asset.map(|a| a.browser_download_url.clone())
                    .unwrap_or_default(),
                release_notes: release.body.unwrap_or_default(),
                published_at: release.published_at.unwrap_or_default(),
                file_size: asset.map(|a| a.size as u64).unwrap_or(0),
            };
            self.set_progress("done", 100.0, &format!("发现新版本 v{}", info.version));
            Ok(Some(info))
        } else {
            self.set_progress("done", 100.0, "已是最新版本");
            Ok(None)
        }
    }

    /// 下载并安装更新
    pub async fn download_and_install(&self, release: &ReleaseInfo) -> Result<()> {
        self.set_progress("downloading", 0.0, "正在下载更新...");

        let download_path = std::env::temp_dir().join("vnt-remote-update");
        std::fs::create_dir_all(&download_path)?;

        let file_name = release.download_url.split('/').last().unwrap_or("update");
        let file_path = download_path.join(file_name);

        let client = reqwest::Client::builder()
            .user_agent("VNTRemote/1.0")
            .build()?;

        let response = client.get(&release.download_url)
            .send()
            .await?;

        let total_size = response.content_length().unwrap_or(1);
        let mut downloaded: u64 = 0;
        let mut file = tokio::fs::File::create(&file_path).await?;

        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            let progress = (downloaded as f64 / total_size as f64) * 90.0;
            self.set_progress("downloading", progress,
                &format!("下载中... {:.1}MB / {:.1}MB",
                    downloaded as f64 / 1_000_000.0,
                    total_size as f64 / 1_000_000.0));
        }
        file.flush().await?;

        self.set_progress("installing", 95.0, "正在安装更新...");

        #[cfg(target_os = "windows")]
        self.install_windows(&file_path)?;
        #[cfg(target_os = "macos")]
        self.install_macos(&file_path)?;
        #[cfg(target_os = "linux")]
        self.install_linux(&file_path)?;

        self.set_progress("done", 100.0, &format!("已更新到 v{}", release.version));
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn install_windows(&self, file: &PathBuf) -> Result<()> {
        use std::process::Command;
        let exe_dir = std::env::current_exe()?
            .parent().unwrap().to_path_buf();

        if file.extension().map(|e| e == "msi").unwrap_or(false) {
            Command::new("msiexec")
                .args(["/i", file.to_str().unwrap(), "/quiet", "/norestart"])
                .spawn()?;
        } else if file.extension().map(|e| e == "exe").unwrap_or(false) {
            Command::new(file).args(["/S"]).spawn()?;
        } else {
            let target = exe_dir.join("vnt-remote.exe");
            std::fs::copy(file, &target)?;
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn install_macos(&self, file: &PathBuf) -> Result<()> {
        use std::process::Command;
        if file.extension().map(|e| e == "dmg").unwrap_or(false) {
            Command::new("hdiutil")
                .args(["attach", file.to_str().unwrap()]).output()?;
            Command::new("ditto")
                .args(["/Volumes/VNTRemote/VNTRemote.app", "/Applications/VNTRemote.app"]).output()?;
            Command::new("hdiutil")
                .args(["detach", "/Volumes/VNTRemote"]).output()?;
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn install_linux(&self, file: &PathBuf) -> Result<()> {
        let target = std::path::Path::new("/usr/local/bin/vnt-remote");
        std::fs::copy(file, target)?;
        use std::process::Command;
        Command::new("chmod").args(["+x", target.to_str().unwrap()]).output()?;
        Ok(())
    }

    fn set_progress(&self, stage: &str, progress: f64, message: &str) {
        if let Ok(mut p) = self.progress.lock() {
            p.stage = stage.into();
            p.progress = progress;
            p.message = message.into();
        }
    }

    pub fn get_progress(&self) -> UpdateProgress {
        self.progress.lock().unwrap().clone()
    }
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    published_at: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    browser_download_url: String,
    size: u64,
}

fn compare_versions(latest: &str, current: &str) -> i32 {
    let l: Vec<u32> = latest.split('.')
        .filter_map(|s| s.parse().ok()).collect();
    let c: Vec<u32> = current.split('.')
        .filter_map(|s| s.parse().ok()).collect();
    for i in 0..l.len().max(c.len()) {
        let lv = l.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if lv > cv { return 1; }
        if lv < cv { return -1; }
    }
    0
}
