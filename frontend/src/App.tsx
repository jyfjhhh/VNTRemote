import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

// Types
interface VntStatus {
  connected: boolean;
  mode: string;
  virtual_ip: string;
  peers: VntPeer[];
  uptime_secs: number;
  rx_bytes: number;
  tx_bytes: number;
}

interface VntPeer {
  id: string;
  name: string;
  virtual_ip: string;
  public_ip: string | null;
  is_p2p: boolean;
  rtt_ms: number | null;
  status: string;
}

interface VntConfig {
  token: string;
  server: string;
  virtual_ip: string;
  device_name: string;
  password: string | null;
  use_encryption: boolean;
  protocol: string;
}

interface RemoteDevice {
  virtual_ip: string;
  hostname: string;
  os: string;
  rtt_ms: number;
  online: boolean;
  services: string[];
}

interface TransferItem {
  id: string;
  file_name: string;
  file_size: number;
  direction: string;
  target_ip: string;
  progress: number;
  status: string;
  error: string | null;
  speed_bytes_per_sec: number;
}

interface CredentialEntry {
  target_ip: string;
  username: string;
  encrypted_password: string;
  credential_type: string;
  display_name: string;
  created_at: string;
}

// Pages
type Page = "dashboard" | "devices" | "files" | "settings" | "about";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

function formatSpeed(bytesPerSec: number): string {
  return formatBytes(bytesPerSec) + "/s";
}

export default function App() {
  const [page, setPage] = useState<Page>("dashboard");
  const [platform, setPlatform] = useState("windows");

  useEffect(() => {
    invoke<string>("get_platform").then(setPlatform).catch(() => {});
  }, []);

  return (
    <div className="app">
      <nav className="sidebar">
        <div className="sidebar-header">
          <div className="logo">VNTRemote</div>
          <div className="logo-sub">基于 VNT 的远程管理</div>
        </div>
        <div className="nav-items">
          <button
            className={`nav-item ${page === "dashboard" ? "active" : ""}`}
            onClick={() => setPage("dashboard")}
          >
            <span className="nav-icon">📊</span>
            <span>仪表盘</span>
          </button>
          <button
            className={`nav-item ${page === "devices" ? "active" : ""}`}
            onClick={() => setPage("devices")}
          >
            <span className="nav-icon">💻</span>
            <span>设备列表</span>
          </button>
          <button
            className={`nav-item ${page === "files" ? "active" : ""}`}
            onClick={() => setPage("files")}
          >
            <span className="nav-icon">📁</span>
            <span>文件传输</span>
          </button>
          <button
            className={`nav-item ${page === "settings" ? "active" : ""}`}
            onClick={() => setPage("settings")}
          >
            <span className="nav-icon">⚙️</span>
            <span>设置</span>
          </button>
          <button
            className={`nav-item ${page === "about" ? "active" : ""}`}
            onClick={() => setPage("about")}
          >
            <span className="nav-icon">ℹ️</span>
            <span>关于</span>
          </button>
        </div>
        <div className="sidebar-footer">
          <span className="platform-badge">{platform}</span>
        </div>
      </nav>
      <main className="main-content">
        {page === "dashboard" && <DashboardPage />}
        {page === "devices" && <DevicesPage platform={platform} />}
        {page === "files" && <FileTransferPage />}
        {page === "settings" && <SettingsPage />}
        {page === "about" && <AboutPage />}
      </main>
    </div>
  );
}

// ==================== Dashboard ====================
function DashboardPage() {
  const [status, setStatus] = useState<VntStatus | null>(null);
  const [config, setConfig] = useState<VntConfig | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    invoke<VntConfig>("get_vnt_config").then(setConfig).catch(() => {});
  }, []);

  const handleStart = async () => {
    if (!config) return;
    setLoading(true);
    try {
      await invoke("start_vnt", { config });
    } catch (e: any) {
      alert("启动失败: " + e);
    }
    setLoading(false);
  };

  const handleStop = async () => {
    try {
      await invoke("stop_vnt");
    } catch (e: any) {
      alert("停止失败: " + e);
    }
  };

  return (
    <div className="page">
      <h1>仪表盘</h1>

      <div className="status-card">
        <div className="status-indicator">
          <div className={`status-dot ${status?.connected ? "online" : "offline"}`} />
          <span>{status?.connected ? "已连接" : "未连接"}</span>
        </div>

        {status?.connected && (
          <div className="status-details">
            <div className="stat">
              <label>虚拟 IP</label>
              <span>{status.virtual_ip}</span>
            </div>
            <div className="stat">
              <label>连接模式</label>
              <span className={`mode-${status.mode}`}>
                {status.mode === "p2p" ? "P2P 直连" : "中继"}
              </span>
            </div>
            <div className="stat">
              <label>在线设备</label>
              <span>{status.peers.length}</span>
            </div>
          </div>
        )}
      </div>

      {config && (
        <div className="config-card">
          <h3>连接配置</h3>
          <div className="config-grid">
            <div className="config-item">
              <label>Token</label>
              <span>{config.token.substring(0, 8)}****</span>
            </div>
            <div className="config-item">
              <label>服务器</label>
              <span>{config.server}</span>
            </div>
            <div className="config-item">
              <label>虚拟 IP</label>
              <span>{config.virtual_ip}</span>
            </div>
            <div className="config-item">
              <label>设备名</label>
              <span>{config.device_name}</span>
            </div>
            <div className="config-item">
              <label>协议</label>
              <span>{config.protocol.toUpperCase()}</span>
            </div>
            <div className="config-item">
              <label>加密</label>
              <span>{config.use_encryption ? "已启用" : "未启用"}</span>
            </div>
          </div>
        </div>
      )}

      <div className="actions">
        <button
          className="btn btn-primary"
          onClick={handleStart}
          disabled={loading || status?.connected}
        >
          {loading ? "连接中..." : "连接 VNT"}
        </button>
        <button
          className="btn btn-danger"
          onClick={handleStop}
          disabled={!status?.connected}
        >
          断开连接
        </button>
      </div>
    </div>
  );
}

// ==================== Devices ====================
function DevicesPage({ platform }: { platform: string }) {
  const [devices, setDevices] = useState<RemoteDevice[]>([]);
  const [scanning, setScanning] = useState(false);
  const [creds, setCreds] = useState<CredentialEntry[]>([]);
  const [showAddCred, setShowAddCred] = useState<string | null>(null);
  const [credForm, setCredForm] = useState({ username: "", password: "", type: "password" });

  const scan = async () => {
    setScanning(true);
    try {
      const result = await invoke<RemoteDevice[]>("scan_devices");
      setDevices(result);
    } catch (e: any) {
      console.error("Scan failed:", e);
    }
    setScanning(false);
  };

  useEffect(() => {
    invoke<CredentialEntry[]>("get_credentials").then(setCreds).catch(() => {});
  }, []);

  const handleRdpConnect = async (ip: string) => {
    try {
      await invoke("launch_rdp", { targetIp: ip });
    } catch {
      // No saved credential, show input
      setShowAddCred(ip);
    }
  };

  const handleSaveCred = async (ip: string) => {
    try {
      await invoke("save_credential", {
        targetIp: ip,
        username: credForm.username,
        password: credForm.password,
        credentialType: credForm.type,
        displayName: ip,
      });
      setShowAddCred(null);
      const updated = await invoke<CredentialEntry[]>("get_credentials");
      setCreds(updated);
    } catch (e: any) {
      alert("保存失败: " + e);
    }
  };

  const handleScreenOff = async (ip: string) => {
    try {
      await invoke("screen_off", { targetIp: ip });
    } catch (e: any) {
      alert("熄屏失败: " + e);
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <h1>设备列表</h1>
        <button className="btn btn-primary" onClick={scan} disabled={scanning}>
          {scanning ? "扫描中..." : "扫描网络"}
        </button>
      </div>

      {devices.length === 0 && (
        <div className="empty-state">
          <p>尚未发现设备</p>
          <p className="hint">请先连接 VNT 网络，然后点击"扫描网络"</p>
        </div>
      )}

      <div className="device-grid">
        {devices.map((dev) => {
          const hasCred = creds.find((c) => c.target_ip === dev.virtual_ip);
          return (
            <div key={dev.virtual_ip} className="device-card">
              <div className="device-header">
                <div className={`os-icon os-${dev.os}`}>
                  {dev.os === "windows" ? "🪟" : dev.os === "macos" ? "🍎" : "🐧"}
                </div>
                <div className="device-info">
                  <span className="device-name">{dev.hostname}</span>
                  <span className="device-ip">{dev.virtual_ip}</span>
                </div>
                <div className={`device-status ${dev.online ? "online" : "offline"}`} />
              </div>

              <div className="device-meta">
                <span>OS: {dev.os}</span>
                {dev.rtt_ms > 0 && <span>延迟: {dev.rtt_ms}ms</span>}
              </div>

              <div className="device-actions">
                {dev.services.includes("rdp") && (
                  <>
                    <button
                      className="btn btn-rdp"
                      onClick={() => handleRdpConnect(dev.virtual_ip)}
                    >
                      🖥️ 远程桌面
                    </button>
                    {hasCred && (
                      <span className="cred-badge" title={`${hasCred.username}`}>
                        🔑
                      </span>
                    )}
                  </>
                )}
                {dev.services.includes("screen_off") && (
                  <button
                    className="btn btn-sm"
                    onClick={() => handleScreenOff(dev.virtual_ip)}
                  >
                    💤 熄屏
                  </button>
                )}
              </div>

              {showAddCred === dev.virtual_ip && (
                <div className="cred-form">
                  <input
                    type="text"
                    placeholder="用户名"
                    value={credForm.username}
                    onChange={(e) => setCredForm({ ...credForm, username: e.target.value })}
                  />
                  <input
                    type="password"
                    placeholder="密码"
                    value={credForm.password}
                    onChange={(e) => setCredForm({ ...credForm, password: e.target.value })}
                  />
                  <select
                    value={credForm.type}
                    onChange={(e) => setCredForm({ ...credForm, type: e.target.value })}
                  >
                    <option value="password">密码</option>
                    <option value="pin">PIN</option>
                  </select>
                  <button className="btn btn-primary" onClick={() => handleSaveCred(dev.virtual_ip)}>
                    保存并连接
                  </button>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ==================== File Transfer ====================
function FileTransferPage() {
  const [transfers, setTransfers] = useState<TransferItem[]>([]);
  const [targetIp, setTargetIp] = useState("");
  const [filePath, setFilePath] = useState("");

  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const items = await invoke<TransferItem[]>("get_transfers");
        setTransfers(items);
      } catch {}
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  const handleSend = async () => {
    if (!targetIp || !filePath) return;
    try {
      await invoke("send_file", { targetIp, localPath: filePath });
    } catch (e: any) {
      alert("发送失败: " + e);
    }
  };

  return (
    <div className="page">
      <h1>文件传输</h1>

      <div className="transfer-form">
        <div className="form-row">
          <label>目标设备 IP</label>
          <input
            type="text"
            placeholder="10.26.0.X"
            value={targetIp}
            onChange={(e) => setTargetIp(e.target.value)}
          />
        </div>
        <div className="form-row">
          <label>本地文件路径</label>
          <input
            type="text"
            placeholder="C:\file.txt 或 /Users/xxx/file.txt"
            value={filePath}
            onChange={(e) => setFilePath(e.target.value)}
          />
        </div>
        <button className="btn btn-primary" onClick={handleSend}>
          发送文件
        </button>
      </div>

      <div className="transfer-list">
        <h3>传输记录</h3>
        {transfers.length === 0 && <p className="hint">暂无传输记录</p>}
        {transfers.map((item) => (
          <div key={item.id} className="transfer-item">
            <div className="transfer-info">
              <span className="transfer-name">{item.file_name}</span>
              <span className="transfer-size">{formatBytes(item.file_size)}</span>
              <span className="transfer-dir">
                {item.direction === "send" ? "→" : "←"} {item.target_ip}
              </span>
            </div>
            <div className="transfer-progress">
              <div className="progress-bar">
                <div
                  className="progress-fill"
                  style={{ width: `${item.progress}%` }}
                />
              </div>
              <span className="progress-text">{item.progress.toFixed(1)}%</span>
            </div>
            <div className="transfer-status">
              <span className={`status-${item.status}`}>{item.status}</span>
              {item.speed_bytes_per_sec > 0 && (
                <span>{formatSpeed(item.speed_bytes_per_sec)}</span>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ==================== Settings ====================
function SettingsPage() {
  const [config, setConfig] = useState<VntConfig>({
    token: "",
    server: "vnt.rustvnt.com:29872",
    virtual_ip: "10.26.0.100",
    device_name: "",
    password: null,
    use_encryption: true,
    protocol: "udp",
  });
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    invoke<VntConfig>("get_vnt_config")
      .then((c) => setConfig(c))
      .catch(() => {});
    // 获取设备名称
    if (!config.device_name) {
      setConfig((c) => ({ ...c, device_name: "VNTRemote-" + Math.random().toString(36).slice(2, 6) }));
    }
  }, []);

  const handleSave = async () => {
    try {
      await invoke("update_vnt_config", { config });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e: any) {
      alert("保存失败: " + e);
    }
  };

  return (
    <div className="page">
      <h1>设置</h1>

      <div className="settings-section">
        <h3>VNT 网络配置</h3>
        <div className="settings-form">
          <div className="form-row">
            <label>Token (组网标识)</label>
            <input
              type="text"
              placeholder="输入唯一 Token，相同 Token 的设备在同一网络"
              value={config.token}
              onChange={(e) => setConfig({ ...config, token: e.target.value })}
            />
          </div>
          <div className="form-row">
            <label>服务器地址</label>
            <input
              type="text"
              value={config.server}
              onChange={(e) => setConfig({ ...config, server: e.target.value })}
            />
            <span className="hint">使用公共服务器: vnt.rustvnt.com:29872，或自建服务器</span>
          </div>
          <div className="form-row">
            <label>虚拟 IP 地址</label>
            <input
              type="text"
              value={config.virtual_ip}
              onChange={(e) => setConfig({ ...config, virtual_ip: e.target.value })}
            />
          </div>
          <div className="form-row">
            <label>设备名称</label>
            <input
              type="text"
              value={config.device_name}
              onChange={(e) => setConfig({ ...config, device_name: e.target.value })}
            />
          </div>
          <div className="form-row">
            <label>网络协议</label>
            <select
              value={config.protocol}
              onChange={(e) => setConfig({ ...config, protocol: e.target.value })}
            >
              <option value="udp">UDP (推荐)</option>
              <option value="tcp">TCP (更稳定)</option>
              <option value="ws">WebSocket</option>
            </select>
          </div>
          <div className="form-row">
            <label>
              <input
                type="checkbox"
                checked={config.use_encryption}
                onChange={(e) => setConfig({ ...config, use_encryption: e.target.checked })}
              />
              启用加密传输
            </label>
          </div>
          <div className="form-row">
            <label>加密密码（可选）</label>
            <input
              type="password"
              placeholder="留空则自动生成"
              value={config.password || ""}
              onChange={(e) => setConfig({ ...config, password: e.target.value || null })}
            />
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>系统设置</h3>
        <div className="settings-form">
          <div className="form-row">
            <label>
              <input type="checkbox" id="autostart" />
              开机自动启动
            </label>
            <span className="hint">系统启动时自动连接 VNT 网络</span>
          </div>
          <div className="form-row">
            <label>
              <input type="checkbox" id="autoconnect" />
              启动时自动连接
            </label>
          </div>
        </div>
      </div>

      <div className="actions">
        <button className="btn btn-primary" onClick={handleSave}>
          {saved ? "✅ 已保存" : "保存配置"}
        </button>
      </div>
    </div>
  );
}

// ==================== About ====================
function AboutPage() {
  const [platform, setPlatform] = useState("");

  useEffect(() => {
    invoke<string>("get_platform").then(setPlatform).catch(() => {});
  }, []);

  return (
    <div className="page">
      <h1>关于 VNTRemote</h1>

      <div className="about-card">
        <div className="about-logo">VNTRemote</div>
        <div className="about-version">v1.0.0</div>
        <p className="about-desc">
          基于 VNT 的远程桌面管理工具，实现无服务器的 P2P 远程办公
        </p>

        <div className="about-features">
          <h3>功能特性</h3>
          <ul>
            <li>🔗 VNT P2P 组网，无需公网服务器</li>
            <li>🖥️ 一键远程桌面 (RDP/VNC)</li>
            <li>💤 远程熄屏节能</li>
            <li>📁 P2P 文件传输</li>
            <li>🔑 凭证加密存储，无密码连接</li>
            <li>🔄 自动更新</li>
            <li>🚀 开机自启</li>
          </ul>
        </div>

        <div className="about-tech">
          <h3>技术栈</h3>
          <div className="tech-tags">
            <span>Rust</span>
            <span>Tauri</span>
            <span>React</span>
            <span>VNT</span>
          </div>
        </div>

        <div className="about-links">
          <button
            className="btn"
            onClick={() => invoke("open_external", { url: "https://github.com/vnt-dev/vnt" })}
          >
            VNT 项目
          </button>
          <button
            className="btn"
            onClick={() => invoke("open_external", { url: "https://rustvnt.com" })}
          >
            VNT 官网
          </button>
        </div>

        <div className="about-platform">
          当前平台: <strong>{platform}</strong>
        </div>
      </div>
    </div>
  );
}
