# Pier — 内网穿透桌面工具 · 共识文档

> 状态：待用户最终确认
> 日期：2026-10-06
> 调研依据：见 `docs/research/` 下三份调研报告

---

## 1. 一句话定位

**Pier**：一款开源、多平台（macOS / Windows）、UI 精美的内网穿透桌面工具——把本地端口一键映射到公网，无需账号即可使用；有自己的 VPS 时可一键部署专属服务端。

面向两类用户：
- **开发者**：webhook 回调调试（微信/支付/飞书）、给客户演示本地 Web 项目、联调第三方服务
- **普通用户**：在外面访问家里的 NAS、办公电脑、监控等 TCP 服务

## 2. 设计决策全记录（19 项）

| # | 决策点 | 结论 |
|---|--------|------|
| 1 | 产品定位 | 开发者本地调试 + 普通用户远程访问家庭设备，双定位 |
| 2 | 连接形态 | 三形态都做；MVP 做中继型（免服务器公共服务 + VPS 一键部署），P2P 直连二期 |
| 3 | 桌面技术栈 | Tauri 2（Rust 壳 + Web 前端） |
| 4 | 平台范围 | macOS + Windows 先行，Linux 后续 |
| 5 | 商业策略 | 开源免费，收费模式以后再考虑 |
| 6 | 隧道协议 | TCP + HTTP/HTTPS（带域名路由）；UDP 二期 |
| 7 | 访问者体验 | 生成公网 URL，免安装直接访问 |
| 8 | MVP 功能 | 多隧道管理 + 托盘常驻 + 开机自启 + 断线重连 + 日志/流量统计 + 二维码分享；访问鉴权为第一迭代 |
| 9 | UI 风格 | 双主题跟随系统，默认暗色；设计 token 保证双色精修 |
| 10 | 域名策略 | VPS 模式支持自定义域名 + 泛解析（`*.mydomain.com`）；免服务器模式用服务方分配域名 |
| 11 | 主界面形态 | 单窗口管理台（隧道列表为主视图）+ 系统托盘快捷操作 |
| 12 | 添加隧道 | MVP 手动添加（选协议/填端口/一键开启）；智能服务发现（检测 Vite/Next/Flask 等 dev server）二期 |
| 13 | P2P 排期 | 二期。技术路线已定：iroh（QUIC 打洞 + 中继兜底一体化），MVP 先建好其依赖的鉴权/隧道管理基建 |
| 14 | 产品名 | **Pier**（码头/桥墩，寓意"从内网架到公网的桥"；CLI 命令 `pier 5000` 顺口） |
| 15 | 前端框架 | React + TypeScript + shadcn/ui + Tailwind |
| 16 | 界面语言 | 中英双语 i18n，默认跟随系统语言 |
| 17 | 免服务器后端 | 双后端内置：**Cloudflare quick tunnel**（HTTP/Web 场景）+ **bore**（任意 TCP 场景），均零账号；zrok 二期作为"登录获稳定 URL"进阶项 |
| 18 | VPS 服务端 | **frp 默认内置**（Apache-2.0、v0.71 活跃、功能全、生态认知零成本）；Provider 架构预留 rathole 作二期第二引擎 |
| 19 | 迁移与诊断 | 均进 MVP：frpc.toml/ini 一键导入 + 一键诊断（人类可读的失败结论） |

## 3. 产品架构

### 3.1 三种连接形态

```
┌─ 形态 A（MVP）：免服务器 · 公共中继
│   本地端口 ──cloudflared──▶ *.trycloudflare.com（HTTP，零账号）
│   本地端口 ──bore─────────▶ bore.pub:随机端口（任意 TCP，零账号）
│
├─ 形态 B（MVP）：自备 VPS · 一键部署
│   桌面端 ──SSH(russh)──▶ VPS 自动安装 frps（systemd + 防火墙 + 云安全组提示）
│   本地端口 ──frpc──▶ 用户自有域名（自定义域名 + 泛解析，HTTP/TCP）
│
└─ 形态 C（二期）：P2P 直连
    两台装有 Pier 的设备 iroh 打洞直连（预期 70-85% 直连率），
    失败自动回退中继；ticket 扫码配对
```

### 3.2 技术架构（Tauri 2）

**Rust 核心模块**：
- `tunnel-engine`：统一 Provider trait（start / stop / status / stats / logs / deploy）
  - `CloudflareProvider`：驱动 cloudflared quick tunnel（HTTP）
  - `BoreProvider`：驱动 bore client（任意 TCP）
  - `FrpProvider`：驱动 frpc + SSH 部署 frps（二期追加 rathole Provider）
- `ssh-deployer`（russh）：VPS 一键部署全链路——环境探测（systemd/架构/发行版）→ 端口占用预检 → 二进制双通道获取（VPS 直连 GitHub，失败则本地下载 SFTP 上传，SHA256 校验）→ 生成 frps.toml（随机 token 存本机 keychain）→ systemd unit（带产品前缀，幂等可接管已有 frps）→ ufw/firewalld 防火墙放行 → 云厂商安全组检测（metadata 端点识别，GUI 出"去控制台放行 TCP 7000/7500/80/443"提示卡片）→ 部署后端到端验证
- `binary-manager`：内置二进制（cloudflared / bore / frpc）版本管理与引擎自更新（SHA256 原子替换，解决 frpc/frps 版本匹配这个最高频故障）
- `diagnostics`：一键诊断规则引擎（安全组未放行 / token 不匹配 / 版本不匹配 / 端口被占用 / NAT 类型受限 → 人类可读结论）
- `config-store`：隧道配置持久化 + frpc.toml/ini 导入导出 + keychain 存密
- 系统集成：托盘常驻、开机自启、断线自动重连、Windows 子进程编码处理（规避竞品的 GBK 乱码坑）

**前端**：React + TypeScript + shadcn/ui + Tailwind，双主题 token，i18n 骨架（zh-CN / en）

**页面清单**：首屏隧道列表（状态灯 / 公网地址一键复制 / 二维码 / 实时流量）、添加隧道向导（选形态 → 选协议 → 填端口 → 完成）、VPS 服务器管理页（SSH 连接配置、一键部署进度、服务端状态）、日志页（彩色实时日志 + 诊断结论卡片）、设置页（主题 / 语言 / 自启 / 引擎版本管理）

### 3.3 内置二进制与体积预算

cloudflared（mac ~20MB / Win ~19msi）+ bore（2.4MB）+ frpc（~12MB），安装包目标控制在 50-60MB 内（远小于 Electron 方案）。

## 4. MVP 功能范围

**包含**：
1. 隧道管理：多隧道增删改启停，TCP 与 HTTP 两种类型
2. 免服务器模式：Cloudflare quick tunnel + bore 双后端，零账号开箱即用
3. VPS 模式：SSH 一键部署 frps 全链路 + 自定义域名泛解析
4. 系统集成：托盘（状态一览 / 一键开关 / 复制地址）、开机自启、断线重连
5. 可观测：实时日志 + 流量统计 + 一键诊断
6. 分享：公网地址一键复制 / 二维码
7. 迁移：frpc.toml/ini 导入
8. 双主题 + 中英 i18n + Tauri 自动更新

**第一迭代追加**：访问鉴权（隧道密码 / IP 白名单）
**明确不做进 MVP**：UDP、P2P 直连、zrok 登录、智能服务发现、CLI、rathole Provider、Linux 版

## 5. 差异化卖点（对竞品）

调研盘点了 frpc-desktop（6.9k★）、frpmgr、MoonProxy、frp-panel、花生壳、SakuraFrp 等全部同类后的结论：

1. **市场空白**：没有任何 GUI 覆盖"服务端侧"——全部假设用户已备好装了 frps 的服务器。Pier 的 SSH 一键部署全链路是独有卖点
2. **零账号开箱**：花生壳/SakuraFrp 要实名注册限速，ngrok 强制账号；Pier 装上就能用
3. **报错能看懂**：竞品最大差评是"报错黑盒"；Pier 的一键诊断直接输出"安全组未放行 7000 / token 不匹配 / 版本不匹配"级别的结论
4. **轻快原生**：Tauri 2 对标 Electron 竞品的卡顿（frpc-desktop 社区自己在 issue 里提议改用 Tauri）
5. **版本匹配自动管**：内置二进制 + 自更新，消灭"frpc/frps 版本不符"这个最高频故障
6. **macOS 正规签名公证**（竞品普遍未公证，用户要手动 `xattr` 去"已损坏"）
7. **开源免费**，无流量/隧道数限制

## 6. 里程碑（估算）

| 里程碑 | 内容 | 周期 |
|--------|------|------|
| M1 alpha | 项目骨架 + 免服务器双后端 + 隧道管理 UI + 托盘 | 2-4 周 |
| M2 beta | VPS 一键部署 + frpc 导入 + 一键诊断 + i18n/双主题打磨 | +2-3 周 |
| M3 1.0 | 访问鉴权 + 流量图表 + 签名公证 + 发布渠道 | +2 周 |
| 二期 | zrok / rathole Provider、智能服务发现、CLI、P2P（iroh）、Linux | 按需 |

## 7. 关键风险与对策

| 风险 | 对策 |
|------|------|
| Cloudflare quick tunnel 官方定位"测试/开发"、无 SLA、trycloudflare 域名被部分网络封锁 | UI 如实标注"临时分享/演示"场景；错误场景优雅处理；VPS 模式作为稳定升级路径引导 |
| bore.pub 为个人维护、无 SLA | 失败自动重试 + UI 提示；bore 服务端单二进制，二期可提供自建/私有节点选项 |
| frp 在国内部分运营商环境被识别阻断 | 二期 rathole Provider（社区已有此迁移先例） |
| macOS 签名公证需 Apple 开发者账号（$99/年） | 开源项目可先 ad-hoc + 首启引导文档，正式发布前购买账号公证 |
| 国内合规：自建服务端备案、端口转发工具监管灰区 | 用户自建自担（我们只提供工具）；文档明确提示；不做官方公共中继规避主体责任 |
| P2P 国内 CGNAT 直连率可能低于预期（移动宽带最差约 20%） | 二期实现时采用 iroh + IPv6 双栈 + 中继兜底，产品文案不承诺"纯 P2P" |

## 8. 二期路线图（P2P 详细）

- **技术栈**：iroh（QUIC 打洞 + relay 兜底 + ticket 配对，纯 Rust 生产验证）+ 本地 TCP listener ↔ iroh stream 桥接
- **适用场景**：访问端也装 Pier（自己的笔记本/手机访问家里 NAS）——直连省流量、低延迟
- **不适用**：任意访客/webhook（无法参与打洞，继续走中继 URL）
- **MVP 预留**：鉴权体系、隧道管理基建、Provider 抽象均已就位，P2P 作为第三种 Provider 插入
