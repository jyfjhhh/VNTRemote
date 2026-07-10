pub mod vnt_manager;
pub mod device_discovery;
pub mod file_transfer;
pub mod updater;
pub mod auth;
pub mod service;

use vnt_manager::{VntManager, VntConfig};
use device_discovery::{DeviceDiscovery, RemoteDevice};
use file_transfer::{FileTransfer, TransferItem};
use updater::{Updater, ReleaseInfo, UpdateProgress};
use auth::AuthManager;
use service::BackgroundService;

use std::sync::Arc;
use tauri::{Manager, State};

/// 应用状态
pub struct AppState {
    pub vnt_manager: VntManager,
    pub device_discovery: DeviceDiscovery,
    pub file_transfer: Arc<FileTransfer>,
    pub updater: Updater,
    pub auth_manager: AuthManager,
    pub background_service: Option<BackgroundService>,
}

// ===================== Tauri Commands =====================

// ---- VNT 管理 ----

#[tauri::command]
fn start_vnt(state: State<AppState>, config: VntConfig) -> Result<(), String> {
    state.vnt_manager.start(config).map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_vnt(state: State<AppState>) -> Result<(), String> {
    state.vnt_manager.stop().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_vnt_config(state: State<AppState>) -> VntConfig {
    state.vnt_manager.get_config()
}

#[tauri::command]
fn update_vnt_config(state: State<AppState>, config: VntConfig) -> Result<(), String> {
    state.vnt_manager.update_config(config).map_err(|e| e.to_string())
}

// ---- 设备发现 ----

#[tauri::command]
async fn scan_devices(state: State<'_, AppState>) -> Result<Vec<RemoteDevice>, String> {
    state.device_discovery.scan_subnet().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_cached_devices(state: State<'_, AppState>) -> Vec<RemoteDevice> {
    state.device_discovery.get_cached_devices().await
}

#[tauri::command]
async fn screen_off(state: State<'_, AppState>, target_ip: String) -> Result<String, String> {
    state.device_discovery.send_screen_off(&target_ip).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn wake_display(state: State<'_, AppState>, target_ip: String) -> Result<String, String> {
    state.device_discovery.send_wake_up(&target_ip).await.map_err(|e| e.to_string())
}

// ---- 文件传输 ----

#[tauri::command]
async fn send_file(
    state: State<'_, AppState>,
    target_ip: String,
    local_path: String,
) -> Result<String, String> {
    state.file_transfer.send_file(&target_ip, &local_path).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_transfers(state: State<'_, AppState>) -> Vec<TransferItem> {
    state.file_transfer.get_transfers().await
}

#[tauri::command]
async fn cancel_transfer(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.file_transfer.cancel_transfer(&id).await.map_err(|e| e.to_string())
}

// ---- 认证管理 ----

#[tauri::command]
fn save_credential(
    state: State<AppState>,
    target_ip: String,
    username: String,
    password: String,
    credential_type: String,
    display_name: String,
) -> Result<(), String> {
    state.auth_manager
        .save_credential(&target_ip, &username, &password, &credential_type, &display_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_credentials(state: State<AppState>) -> Vec<auth::CredentialEntry> {
    state.auth_manager.list_credentials()
}

#[tauri::command]
fn delete_credential(state: State<AppState>, target_ip: String) -> Result<(), String> {
    state.auth_manager.delete_credential(&target_ip).map_err(|e| e.to_string())
}

#[tauri::command]
fn launch_rdp(state: State<AppState>, target_ip: String) -> Result<(), String> {
    state.auth_manager.launch_rdp(&target_ip).map_err(|e| e.to_string())
}

#[tauri::command]
fn launch_rdp_with_creds(
    target_ip: String,
    username: String,
    password: String,
) -> Result<(), String> {
    // 直接使用提供的凭证，不存储
    launch_rdp_direct(&target_ip, &username, &password).map_err(|e| e.to_string())
}

fn launch_rdp_direct(target_ip: &str, username: &str, password: &str) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let rdp_content = format!(
            "full address:s:{}\nusername:s:{}\nprompt for credentials:i:0\n" \
            "authentication level:i:0\n",
            target_ip, username
        );
        let rdp_path = std::env::temp_dir().join("vnt_remote_quick.rdp");
        std::fs::write(&rdp_path, rdp_content)?;
        open::that(rdp_path)?;
    }

    #[cfg(target_os = "macos")]
    {
        open::that(format!("vnc://{}:{}@{}", username, password, target_ip))?;
    }

    Ok(())
}

// ---- 自动更新 ----

#[tauri::command]
async fn check_update(state: State<'_, AppState>) -> Result<Option<ReleaseInfo>, String> {
    state.updater.check_for_update().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn download_update(state: State<'_, AppState>, release: ReleaseInfo) -> Result<(), String> {
    state.updater.download_and_install(&release).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn get_update_progress(state: State<AppState>) -> UpdateProgress {
    state.updater.get_progress()
}

// ---- 系统 ----

#[tauri::command]
fn get_platform() -> String {
    if cfg!(target_os = "windows") { "windows".into() }
    else if cfg!(target_os = "macos") { "macos".into() }
    else if cfg!(target_os = "linux") { "linux".into() }
    else { "unknown".into() }
}

#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| e.to_string())
}

/// 初始化应用
pub fn run_app() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LoginItem,
        ))
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // 初始化各模块
            let file_transfer = Arc::new(FileTransfer::new());
            let bg_service = BackgroundService::new(file_transfer.clone());

            // 如果是服务模式（被控端），启动后台服务
            let args: Vec<String> = std::env::args().collect();
            if args.iter().any(|a| a == "--service") {
                if let Err(e) = bg_service.start() {
                    eprintln!("后台服务启动失败: {}", e);
                }
            }

            let state = AppState {
                vnt_manager: VntManager::new(),
                device_discovery: DeviceDiscovery::new(),
                file_transfer,
                updater: Updater::new(),
                auth_manager: AuthManager::new()
                    .expect("AuthManager 初始化失败"),
                background_service: Some(bg_service),
            };

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_vnt,
            stop_vnt,
            get_vnt_config,
            update_vnt_config,
            scan_devices,
            get_cached_devices,
            screen_off,
            wake_display,
            send_file,
            get_transfers,
            cancel_transfer,
            save_credential,
            get_credentials,
            delete_credential,
            launch_rdp,
            launch_rdp_with_creds,
            check_update,
            download_update,
            get_update_progress,
            get_platform,
            open_external,
        ])
        .run(tauri::generate_context!())
        .expect("VNTRemote 启动失败");
}
