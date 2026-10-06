# 调研报告 3：自建服务端选型 + 竞品 GUI 盘点 + SSH 一键部署方案（2025-2026）

> 数据采集时间：2026-10-06，Star/活跃度数据来自 GitHub API 实时抓取

## 一、候选服务端对比

### 1.1 对比总表

| 维度 | **frp** | **rathole** | **bore** | **zrok** | **nps** | **wstunnel** |
|---|---|---|---|---|---|---|
| Star | **109.8k** | 14.3k | 11.5k | 4.8k | 34.2k（原仓库）/ 3.4k（接力 fork） | 7.1k |
| 语言 | Go | Rust | Rust | Go | Go | Rust |
| 最新正式版 | v0.71.0（2026-08-14） | v0.5.0（**2023-10-01**，之后仅 dev build） | v0.5.0（2022，功能冻结） | v2.0.7（2026-10-03） | 0.26.10 后停更 | v11.0.0（2026-09-19） |
| 最近推送 | 2026-09-15（**活跃**） | 2026-08-23（低频） | 2026-02（基本停更） | 2026-10-03（**活跃**） | 原 2024-05 停摆 | 2026-09-27（**活跃**） |
| 许可证 | **Apache-2.0** | Apache-2.0 | MIT | Apache-2.0 | **GPL-3.0**（商用打包需注意） | BSD-3-Clause |
| TCP/UDP | ✅ | ✅ | ⚠️ 仅 TCP | ✅ | ✅ | ✅（反向） |
| HTTP/HTTPS + 域名路由 | ✅（vhost + subdomain） | ❌ | ❌ | ✅ | ✅（含 URL 路由） | ❌ |
| 静态文件 serve | ✅（static_file 插件 + Basic Auth） | ❌ | ❌ | ✅（drive 模式） | ✅ | ❌ |
| 鉴权 | token（默认）/ OIDC，TLS 默认开 | token **强制**（服务级） | 可选 secret（仅握手 HMAC，流量不加密） | 账号体系 + token，端到端加密 | 密钥 + 面板账号 | 无内置鉴权（靠 TLS/mTLS） |
| 自带 Dashboard | ✅ 服务端 Dashboard + 客户端 Admin UI + Prometheus | ❌（HTTP API 开发中） | ❌ | ✅ Web Console | ✅ Web 面板（较全） | ❌ |
| 单二进制跨平台 | ✅ 全平台 | ✅（二进制可小至 ~500KiB） | ✅ | ✅ | ✅ | ✅ 静态二进制 |
| 资源占用 | 中等（Go，几十 MB 级） | **最低** | 极低 | 客户端 ~10MB，**但服务端需整套 OpenZiti** | 中等 | 低 |
| 服务端配置复杂度 | 低（frps.toml 几行即可） | 低 | 极低（一条命令） | **高**：需 OpenZiti controller + router + DB + frontend + 泛域名 DNS/TLS | 中（需面板配置） | 低 |
| 社区口碑/风险 | 生态最大、教程最多；国内环境易被识别阻断（社区反馈） | "更新慢、issue 没人回"是常见吐槽；常被用作被墙 frp 的替代 | 定位极简，作者已不积极 | 功能强但自托管门槛高 | 原作者弃坑历史 + 2021 年曾有安全漏洞风波；GPL | 定位是抗审查伪装隧道，非端口映射管理 |

### 1.2 推荐

**默认内置：frp。** 理由：

1. **事实标准**：109.8k star，中文文档/教程生态无可匹敌——目标用户（有 VPS 的中文开发者/NAS 玩家）大概率已听说过甚至用过 frp，"一键部署 frps"是零解释成本的卖点。
2. **功能完整覆盖产品需求**：TCP/UDP/HTTP/HTTPS、vhost 域名路由、静态文件 serve（自带 Basic Auth）、token 鉴权、服务端 Dashboard、Prometheus 指标——GUI 可以直接把 Dashboard 数据嵌进流量图。
3. **服务端配置几乎零门槛**：frps 最小可用只需 `bindPort` + `auth.token`，最适合"SSH 一键部署"自动生成。
4. **维护活跃 + Apache-2.0**：商用打包无许可证风险（nps 的 GPL-3.0、原仓库停更是硬伤）。
5. **竞品验证**：所有主流 GUI（frpc-desktop、frpmgr、MoonProxy、frp-panel）全部围绕 frp 生态，客户端配置文件可直接互导。

**是否支持多个服务端：是，但分阶段。** 架构上把服务端抽象为 Provider trait（部署/生成配置/解析状态），v1 只做 frp；v1.5 增加可选的 **rathole**（针对内存极小的 VPS、以及"frp 被运营商识别阻断"场景的差异化卖点）。zrok（自托管需整套 OpenZiti 栈）、bore（仅 TCP 且停更）、nps（GPL + 弃坑史）、wstunnel（不同细分赛道）均不建议内置。

## 二、竞品桌面 GUI 现状与痛点

### 2.1 盘点表

| 工具 | 技术栈/平台 | Star | 维护状态 | 说明 |
|---|---|---|---|---|
| luckjiawei/frpc-desktop | Electron/TS，Win+mac+Linux | **6.9k** | 活跃（2026-09 推送） | 最热门开源 frpc GUI，纯客户端 |
| koho/frpmgr | Go + Win32，**仅 Windows** | 2.1k | 活跃（2026-09 推送） | Windows 原生，口碑尚可但无 mac |
| MoonProxyHQ/moonproxy-desktop | **Tauri 2 + Rust + Vue3**，Win+mac | 60 | 新项目（2026-07 起推广） | 同赛道同技术栈直敌，纯客户端；mac 为 ad-hoc 签名未公证 |
| VaalaCat/frp-panel | Web 面板（Master/Server 架构） | 1.8k | 活跃 | 需先自部署面板，对个人用户过重 |
| kanadeblisst00/FrpcManager | Windows 专用 | 小 | 一般 | frpc 注册为系统服务 |
| LakeYang/frp-GUI | WinForms | 小 | **已死**（内置 frpc v0.21.0） | 老一代 GUI 的典型结局 |
| 花生壳（贝锐） | 闭源商业客户端 | — | 商业运营 | 免费 1M 带宽 + 流量/隧道数限制、付费套餐贵，社区持续寻找替代 |
| SakuraFrp Launcher（natfrp） | Electron（闭源），Win+mac+Linux | — | 商业运营 | 口碑较好但绑定其免费服务（实名+限速） |
| yisier/nps（含面板） | Web 面板 | 3.4k | 活跃 fork | 面板偏运维向，非桌面原生体验 |

### 2.2 用户抱怨点清单（主要来自 frpc-desktop 的 133 个 issues + 社区）

1. **性能**：Electron 卡顿——"每次切换 tab 相当卡"（mac #137）、"未启用 frpc 时明显卡顿"（Win #127/#128）。**issue #149 里社区自己提议用 Tauri 重写**——直接验证了我们的技术栈选择。
2. **错误呈现差**："链接失败后无任何日志显示"（#122）、"运行一段就断开"（#136）——失败原因没有人类可读的解释。
3. **日志编码问题**：Windows 下子进程日志 GBK/UTF-8 乱码（#144）、taskkill 编码导致崩溃（#142）。
4. **兼容性碎**：Win7/Win Server 2012 启动报错（#121/#126）、"所选 frp 架构与系统不符"（#125）。
5. **配置管理弱**：web 面板改的配置重启后被重置（#135）、请求导入已有 toml/ini（#138）、请求识别本地已装的 frpc（#147）。
6. **macOS 体验差**：需要 sudo 提权弹窗（#129）、开源 GUI 的 mac 包普遍未公证（MoonProxy 需手动 `xattr` 去"已损坏"）。
7. **版本匹配无人管**：frpc/frps 版本不匹配是最高频故障，现有 GUI 均不处理。

### 2.3 我们的差异化机会点

- **机会点 0（最大空白）：没有任何一款 GUI 覆盖"服务端"**。现有全部 GUI 都假设用户"自备一台已装好 frps 的服务器"，从 SSH 连接 → 装服务端 → 生成 token → 回填客户端 → 防火墙放行的全链路无人实现。
- Tauri 2 轻量化（对标 Electron 卡顿）+ 精美 UI。
- "连接状态由真实证据支撑"：部署时预检 + 运行时诊断。
- 一键诊断：失败时自动给出"安全组未放行 7000 / token 不匹配 / frpc-frps 版本差异"级别的中文结论。
- 内置二进制 + 引擎自更新（SHA256 校验原子替换），解决版本匹配与架构误选。
- 配置互通：导入/导出 frpc.toml/ini，识别接管本地已有 frpc。
- macOS 正规签名公证、托盘常驻、开机自启。

## 三、"SSH 一键部署"推荐实现方式

参考先例：Tailscale install.sh（发行版探测）、MvsCode/frps-onekey（frps 一键脚本，约 2k star；其短板：仍用 SysV init 而非 systemd、完全没有防火墙处理逻辑——只粗暴 `setenforce 0`）、1Panel/宝塔安装脚本（systemd 注册 + 随机密码生成）。

桌面端通过 Rust SSH 库（russh）执行，推荐流程：

1. **环境探测**（只读命令）：`whoami`（root/免密 sudo 检测）；`uname -m` 映射架构（x86_64→amd64，aarch64→arm64）；读 `/etc/os-release` 的 ID/VERSION_ID；systemd 检测 `[ -d /run/systemd/system ]`——非 systemd 环境给出明确不支持提示而非静默失败。
2. **依赖检查**：`curl`/`wget`/`tar` 缺失时用 `apt-get/dnf/yum` 安装，务必 `DEBIAN_FRONTEND=noninteractive`。
3. **端口占用预检**：`ss -tlnp` 检查 bindPort(7000)、vhostHTTP(S)Port(80/443)、dashboardPort(7500)，冲突时在 GUI 提示改端口。**云安全组无法脚本放行**——检测云厂商 metadata 端点（169.254.169.254 / 100.100.100.200）后，在 GUI 里给出"请到 AWS/阿里云控制台放行 TCP 7000,7500,80,443"的卡片 + 一键复制。
4. **获取二进制（双通道）**：优先 VPS 直接 `curl` GitHub Release（国内 VPS 提供 ghproxy 镜像 fallback）；失败则桌面端本地下载后 **SFTP 上传**。完成后 `sha256sum` 校验 + `chmod 755`。安装目录建议 `/opt/<product>/frps`（避开 `/usr/local/frps`，降低与手动安装冲突）。
5. **生成配置**：自动写 `frps.toml`——随机 32 位 token（存入本机 keychain 并回填客户端）、`webServer` 随机密码、可选 `allowPorts` 限制、`subdomainHost`（若用户填了域名）。TOML 生成要处理转义。
6. **systemd unit**：写入 `/etc/systemd/system/<product>-frps.service`（**带产品前缀，避免冲突/便于卸载**），要点：`After=network-online.target`、`Restart=on-failure` + `RestartSec=5`、`LimitNOFILE=1048576`、绝对路径 ExecStart；然后 `daemon-reload` + `enable --now`。
7. **防火墙放行**：按序探测——`ufw allow <port>/tcp`（Debian/Ubuntu）、`firewall-cmd --permanent --add-port=... && firewall-cmd --reload`（CentOS/RHEL）、iptables 兜底；SELinux 存在时用 `semanage port` 或明确提示，**不要**直接 `setenforce 0`。
8. **部署后验证**：VPS 侧 `curl -sI localhost:7500` 确认起来，拉取 `journalctl -u <product>-frps -n 50` 回显到 GUI 日志面板；随后桌面端直接发起 frpc 连接做端到端验证。
9. **升级/卸载**：`frps --version` 探测；升级 = 备份旧二进制 → stop → 替换 → start（配置不动）；卸载 = `disable --now` → 删 unit/二进制 → 清理防火墙规则 → 配置打包下载回本地留档。
10. **幂等与接管**：部署前探测已有 `frps*.service` 或 `/etc/frp`，存在则进入"接管已有 frps"模式（读出 token 或让用户重置），支持重复部署不产生僵尸服务。

## 四、信息来源

**服务端项目**：fatedier/frp（https://github.com/fatedier/frp）· rapiz1/rathole（https://github.com/rapiz1/rathole）· ekzhang/bore（https://github.com/ekzhang/bore）· openziti/zrok（https://github.com/openziti/zrok）· ehang-io/nps / yisier/nps · erebe/wstunnel

**rathole 口碑**：冲浪笔记 rathole 指南（https://www.chonglangbiji.com/clients/rathole）· Nodeseek 部署帖（https://www.nodeseek.com/post-260881-1）· Appinn 讨论（https://meta.appinn.net/t/topic/75337）

**竞品 GUI**：luckjiawei/frpc-desktop（https://github.com/luckjiawei/frpc-desktop）· koho/frpmgr（https://github.com/koho/frpmgr）· MoonProxyHQ/moonproxy-desktop（https://github.com/MoonProxyHQ/moonproxy-desktop）· VaalaCat/frp-panel（https://github.com/VaalaCat/frp-panel）· LakeYang/frp-GUI（https://github.com/LakeYang/frp-GUI）· SakuraFrp 启动器文档（https://doc.natfrp.com/launcher/usage.html）· 花生壳官网（https://www.oray.com）

**SSH 部署先例**：MvsCode/frps-onekey（https://github.com/MvsCode/frps-onekey）· Tailscale install.sh（https://tailscale.com/install.sh）· 1Panel（https://github.com/1Panel-dev/1Panel）· dylanbai8/frpspro · 思有云 frps 脚本（https://www.ioiox.com）

---

**核心结论**：默认内置 **frp**（Apache-2.0、v0.71 活跃、功能全、生态认知零成本），架构预留 rathole 作为轻量/抗封锁第二 Provider；市场上所有 frp GUI 都是"纯客户端"且以 Electron 卡顿+报错黑盒为通病，"**SSH 一键部署服务端全链路 + Tauri 2 原生体验 + 一键诊断**"是清晰的差异化空位。
