// VNTRemote - 基于 VNT 的远程桌面管理工具
//
// 使用方式:
//   vnt-remote                    # GUI 模式（控制端）
//   vnt-remote --service          # 后台服务模式（被控端）
//   vnt-remote --service --install  # 安装为系统服务
//   vnt-remote --version          # 显示版本

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("VNTRemote v{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // 处理 --service 参数：后台服务模式
    if args.iter().any(|a| a == "--service") {
        run_service_mode();
        return;
    }

    // GUI 模式
    vnt_remote_lib::run_app();
}

/// 后台服务模式入口
/// 在办公电脑上运行，提供远程控制的能力
fn run_service_mode() {
    println!("VNTRemote 后台服务启动中...");

    let file_transfer = std::sync::Arc::new(
        vnt_remote_lib::file_transfer::FileTransfer::new()
    );
    let service = vnt_remote_lib::service::BackgroundService::new(file_transfer);

    if let Err(e) = service.start() {
        eprintln!("后台服务启动失败: {}", e);
        std::process::exit(1);
    }

    println!("VNTRemote 后台服务运行中 (端口: 9527/9528)");
    println!("按 Ctrl+C 停止服务");

    // 保持进程运行
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
