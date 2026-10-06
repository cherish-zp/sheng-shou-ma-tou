# Pier

> 一座从内网架到公网的桥。开源、精美的桌面端内网穿透工具。

**Pier** 把你电脑上的本地端口一键映射到公网——无需注册账号、无需命令行，装上就能用。

<!-- 发布时把 cherish-zp/MasterfulHandsPier 替换为实际仓库地址（下同） -->
[![Build](https://github.com/cherish-zp/MasterfulHandsPier/actions/workflows/build.yml/badge.svg)](https://github.com/cherish-zp/MasterfulHandsPier/actions/workflows/build.yml) ![platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue) ![license](https://img.shields.io/badge/license-MIT-green)

## 下载

前往 [GitHub Releases](https://github.com/cherish-zp/MasterfulHandsPier/releases) 获取最新版本：

| 平台 | 产物 |
|---|---|
| macOS (Apple Silicon) | `Pier_aarch64.dmg` |
| macOS (Intel) | `Pier_x64.dmg` |
| Windows | `Pier_x64-setup.exe`（NSIS）或 `Pier_x64_en-US.msi` |
| Linux | `.deb` / `.AppImage` |

> 当前版本未做代码签名：macOS 首次打开请右键 → 打开；Windows 若遇 SmartScreen 提示请点"仍要运行"。
> 签名 / 公证流程已就绪，见 [docs/RELEASE.md](docs/RELEASE.md)，待配置开发者账号后启用。

## 特性

- **零账号开箱即用** — 两种免服务器引擎内置：
  - **Cloudflare quick tunnel**：本地 HTTP/Web 服务 → 临时公网网址（`*.trycloudflare.com`），适合演示与 webhook 调试
  - **bore**：任意 TCP 服务（SSH、远程桌面、数据库…）→ `bore.pub:端口`
- **多隧道管理** — 状态一目了然，公网地址一键复制 / 扫码分享，实时日志与错误摘要
- **断线自动重连** — 指数退避重试，状态实时同步到界面与系统托盘
- **为 VPS 用户准备的更多能力**（路线图）— SSH 一键部署 frp 服务端，自定义域名 + 泛解析，把隧道架在自己的服务器上
- **精美原生体验** — Tauri 2 构建，安装包小、内存占用低；暗色/亮色双主题、中英双语、系统托盘、开机自启

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2（Rust） |
| 前端 | React 19 + TypeScript + Tailwind CSS v4 + shadcn/ui |
| 隧道引擎 | [cloudflared](https://github.com/cloudflare/cloudflared)（Apache-2.0）、[bore](https://github.com/ekzhang/bore)（MIT） |

## 开发

```bash
# 前置要求：Node 20+、Rust 1.77+（macOS 需 Xcode CLT，Windows 需 MSVC）
npm install
npm run tauri dev    # 开发模式
npm run tauri build  # 打包
```

项目文档见 [docs/](docs/)：

- [docs/CONSENSUS.md](docs/CONSENSUS.md) — 产品共识：定位、19 项核心决策、架构与里程碑
- [docs/research/](docs/research/) — 免费公共隧道 / P2P 打洞 / 服务端选型三份调研报告

## 路线图

- [x] M1：免服务器双引擎（cloudflared + bore）、隧道管理、托盘、双主题、i18n
- [x] M2：VPS 一键部署 frp 服务端（SSH）、frpc 配置导入、一键诊断、自定义域名
- [x] M3：访问鉴权（密码 / IP 白名单）、流量统计图表、品牌应用图标与三平台 CI
- [ ] 签名公证 — 流程就绪（[docs/RELEASE.md](docs/RELEASE.md)），待开发者账号
- [ ] 之后：P2P 直连（iroh）、智能 dev server 发现、CLI（`pier 5000`）、Linux 版

## License

MIT
