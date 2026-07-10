# VNTRemote

基于 VNT 的远程桌面管理工具 — P2P 直连，无需公网服务器，跨平台远程办公。

## 适用场景

| 场景 | 说明 |
|------|------|
| 🏢 **在家访问公司电脑** | 通过 VNT P2P 隧道直连公司办公电脑 |
| 💤 **远程熄屏节能** | 远程关闭办公电脑显示器，省电静音 |
| 🔑 **无密码登录** | 凭证加密存储，一键 RDP 无需每次输密码 |
| 📁 **文件传输** | P2P 直传文件，不经过第三方服务器 |
| 🚀 **开机自启** | 系统启动时自动接入 VNT 网络 |

## 架构

```
                    VNTRemote GUI (Tauri + React)
┌─────────────────────────────────────────────────────────┐
│  仪表盘   设备列表   文件传输   设置   关于              │
└──────┬─────────┬──────────┬──────────┬─────────────────┘
       │         │          │          │
┌──────▼─────────▼──────────▼──────────▼─────────────────┐
│                Rust 核心层                                │
│  vnt_manager | device_discovery | file_transfer         │
│  auth_manager | updater | service                       │
└──────────────────────┬──────────────────────────────────┘
                       │
         ┌─────────────┼─────────────┐
         │             │             │
         ▼             ▼             ▼
      VNT-CLI      RDP/VNC      GitHub
    (P2P隧道)    (远程桌面)    (自动更新)
```

### 工作模式

```
┌──────────────────────┐     ┌──────────────────────┐
│   控制端 (你的电脑)    │     │ 被控端 (公司办公电脑)  │
│                      │     │                      │
│  VNTRemote GUI       │     │  VNTRemote --service  │
│  ┌────────────────┐  │     │  ┌────────────────┐  │
│  │ 设备发现        │  │     │  │ 命令服务 :9527 │  │
│  │ RDP启动        │  │     │  │ 熄屏/唤醒      │  │
│  │ 文件传输        │◄┼─────┼─►│ 文件接收 :9528 │  │
│  │ 凭证管理        │  │     │  │ 设备信息       │  │
│  └────────────────┘  │     │  └────────────────┘  │
│         │            │     │         │            │
│         ▼            │     │         ▼            │
│   VNT 虚拟网卡       │     │   VNT 虚拟网卡       │
│   10.26.0.X         │     │   10.26.0.Y         │
└──────────────────────┘     └──────────────────────┘
         │                           │
         └─────────── VNT P2P ───────┘
                  (NAT穿透直连)
```

## 核心功能

### VNT 连接管理
- 配置 Token、服务器地址、虚拟 IP
- 启动/停止 VNT 连接
- 实时显示连接状态、P2P 直连/中继模式
- 支持 UDP/TCP/WebSocket 多种协议

### 设备发现
- 自动扫描 VNT 虚拟子网（10.26.0.0/24）
- 显示在线设备列表（IP、主机名、操作系统）
- 一键 RDP 远程桌面

### 远程桌面
- **Windows**: 调用系统 MSTSC (`mstsc /v:IP`)
- **macOS**: 调用系统 VNC (`open vnc://IP`)
- 凭证加密存储（AES-256-GCM），实现无密码连接
- 支持密码 / PIN 两种凭证类型

### 远程熄屏 💤
- 向被控端发送熄屏指令
- Windows: 通过 PowerShell 调用 Win32 API
- macOS: 通过 `pmset displaysleepnow`
- 同时支持屏幕唤醒

### 文件传输
- P2P 直连传输，不经过服务器
- 基于 TCP 的自定义传输协议
- 实时进度显示
- 传输速度展示

### 自动更新
- 启动时检查 GitHub 最新版本
- 静默下载更新包
- 跨平台安装（Windows MSI/EXE, macOS DMG）

### 开机自启 🔄
- 系统启动时自动运行
- 可配置自动连接 VNT 网络

## 快速开始

### 1. 安装 VNT

VNT 是 P2P 组网工具，需要在每台设备上安装。

**Windows**:
```powershell
# 下载 vnt-cli-x86_64-pc-windows-msvc.zip
# 解压到 C:\Program Files\VNT\ 并加入 PATH
```

**macOS**:
```bash
brew install vnt
# 或手动下载 vnt-cli-x86_64-apple-darwin.zip
```

> 下载: https://github.com/vnt-dev/vnt/releases
> 官方公共服务器: `vnt.rustvnt.com:29872`

### 2. 验证 VNT 组网

所有设备使用相同的 Token 即加入同一虚拟局域网。

```bash
# Windows (管理员)
vnt-cli -k YOUR_TOKEN --ip 10.26.0.100 -s vnt.rustvnt.com:29872 -n MY_PC

# macOS/Linux
sudo ./vnt-cli -k YOUR_TOKEN --ip 10.26.0.200 -s vnt.rustvnt.com:29872 -n MY_MAC
```

确认两台设备能互相 ping 通后，再安装 VNTRemote。

### 3. 在被控端启动后台服务

公司办公电脑上运行服务模式：

```bash
# Windows
vnt-remote --service

# macOS
./vnt-remote --service
```

输出 `VNTRemote 后台服务运行中 (端口: 9527/9528)` 表示成功。

### 4. 在控制端打开 GUI

自己的电脑上运行 GUI：

```bash
./vnt-remote
# 或双击 VNTRemote 图标
```

在"设置"页面填入 VNT 配置参数（Token、服务器、虚拟 IP），点击"连接 VNT"。

连接后在"设备列表"页面点击"扫描网络"，即可发现在线设备并远程控制。

## 构建

### GitHub Actions (推荐)

推送代码到 GitHub 后自动构建：

1. 推送代码到 GitHub
2. 创建 Tag 触发 Release: `git tag v1.0.0 && git push --tags`
3. GitHub Actions 自动构建 Windows/macOS/Linux 三平台安装包
4. Release 页面自动上传产物

### 本地构建

需要安装:
- **Rust**: https://rustup.rs
- **Node.js**: https://nodejs.org (v18+)
- **Tauri CLI**: `cargo install tauri-cli --version "^2"`
- **Windows**: 还需要 WebView2 (Win10+ 自带)

```bash
# 安装前端依赖
cd frontend
npm install

# 开发模式运行
cd ..
cargo tauri dev

# 构建生产版本 (Windows)
cargo tauri build --bundles msi

# 构建生产版本 (macOS)
cargo tauri build --bundles dmg

# 构建生产版本 (Linux)
cargo tauri build --bundles deb
```

### Windows 额外依赖

Windows 构建需安装:
1. **Visual Studio Build Tools** (安装时勾选 "Desktop development with C++")
2. **WebView2** (Windows 10 1803+ 已内置)
3. **WiX Toolset** (用于 MSI 安装包): `cargo install tauri-cli` 后会自动安装

### macOS 额外依赖

```bash
# 安装 Xcode Command Line Tools
xcode-select --install
```

## 配置说明

| 参数 | 说明 | 默认值 |
|------|------|--------|
| Token | 组网标识，相同 Token 的设备在同一网络 | - |
| Server | VNT 注册/中继服务器地址 | vnt.rustvnt.com:29872 |
| Virtual IP | 虚拟局域网 IP 地址 | 10.26.0.100 |
| Device Name | 设备显示名称 | 主机名 |
| Protocol | 网络协议 (UDP/TCP/WS) | udp |
| Encryption | 是否启用加密 | 是 |
| Auto-start | 开机自动启动 | 否 |
| Auto-connect | 启动时自动连接 VNT | 否 |

## 凭证管理

VNTRemote 使用 AES-256-GCM 加密存储 RDP 凭证：

- **加密密钥**: 基于机器硬件信息（UUID + 主机名）派生
- **存储位置**: `~/.config/vnt-remote/credentials.json` (macOS/Linux) 或 `%APPDATA%/vnt-remote/credentials.json` (Windows)
- **安全性**: 凭证仅在本地解密，不通过网络传输明文密码

## 项目结构

```
vnt-remote/
├── .github/workflows/       # GitHub Actions 自动构建
├── frontend/                # Tauri 前端 (React + TypeScript)
│   ├── src/
│   │   ├── App.tsx          # 主应用 (5个页面)
│   │   ├── App.css          # 全局样式 (暗色主题)
│   │   └── main.tsx         # 入口
│   ├── package.json
│   └── index.html
├── src-tauri/               # Rust 后端
│   ├── src/
│   │   ├── main.rs          # 入口 (GUI / --service)
│   │   ├── lib.rs           # Tauri 命令注册
│   │   ├── vnt_manager.rs   # VNT 进程管理
│   │   ├── device_discovery.rs  # 设备发现 + 远程命令
│   │   ├── file_transfer.rs # 文件传输
│   │   ├── updater.rs       # 自动更新
│   │   ├── auth.rs          # 凭证加密存储
│   │   └── service.rs       # 后台服务模式
│   ├── Cargo.toml
│   └── tauri.conf.json
├── scripts/                 # 构建脚本
│   ├── build-windows.bat
│   └── build-macos.sh
└── README.md
```

## 技术栈

| 层 | 技术 |
|-----|------|
| 后端语言 | Rust 2021 |
| GUI 框架 | Tauri 2.0 |
| 前端 | React 18 + TypeScript + Vite |
| 网络 | VNT (P2P VPN) |
| 远程桌面 | RDP (Windows) / VNC (macOS) |
| 加密 | AES-256-GCM + SHA-256 |
| 文件传输 | TCP 自定义协议 |
| 自动更新 | GitHub Releases API |
| 跨平台 | Windows 10+ / macOS 12+ / Linux |
