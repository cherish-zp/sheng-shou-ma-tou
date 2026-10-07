# 更新日志

本项目所有显著变更将记录于此文件。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本 2.0.0](https://semver.org/lang/zh-CN/)。

## [0.3.0] - 2026-10-07

### Added
- **应用内自动更新**（tauri-plugin-updater + dbx 式编排）：
  - 双源检查端点：Gitee 常驻 latest release（国内快）优先，GitHub 漂移直链兜底
  - 启动静默检查 + 每 60 分钟定时 + 设置页「检查更新」手动触发
  - 发现新版本 → 底部横幅（版本/说明/立即更新/忽略此版本）→ 下载进度条 → 「重启并更新」
  - 三平台原地更新：macOS 替换 .app / Windows NSIS 静默 / Linux 替换 AppImage
  - minisign 签名验证（公钥内嵌，防更新包篡改）
- CI：更新包签名 + Gitee 常驻 latest release 维护（latest.json 的 URL 自动重写为
  Gitee 国内直链）+ 独立 sync-gitee 手动重跑工作流
- `docs/AUTO-UPDATE.md`：通用自动更新方案文档（任何 Tauri 2 项目可复用）

### Changed
- 应用图标更新至新品牌视觉
- 隧道启停开关选中态改为绿色（运行态视觉）
- 菜单宽度与文案修正（清理项去重/中文化）

### Fixed
- Named tunnel 强制 http2（QUIC 阻断网络下永久卡「等待分配公网地址」）
- 秘密存储迁移至本地 SQLite（未签名应用每次重建触发钥匙串授权弹窗的问题彻底解决）
- 钥匙串 → SQLite 迁移改为直读钥匙串；更新路由请求补 config 包裹（CF 1030）
- 侧边栏导航高亮互斥、双实例/双托盘图标

## [0.2.0] - 2026-10-07

### Added
- **Cloudflare 固定域名（Named Tunnel）**：粘贴 API Token 三步绑定，子域名永久固定；
  支持编辑时修改域名/子域（自动换绑路由与 DNS）、一键清理云端资源（隧道 + DNS）、
  API Token 小眼睛查看；每条隧道独立 Token
- 帮助中心（侧栏 ？）：固定域名图文指南、Token 权限清单、常见问题

### Changed
- 秘密存储由 macOS 钥匙串迁移至本地 SQLite（`secrets.db`）——未签名应用每次
  重建都会触发钥匙串授权弹窗，已彻底告别；启动时自动迁移旧数据
- 隧道启停开关选中态改为绿色（运行态视觉）

### Fixed
- Named tunnel 强制 `--protocol http2`（QUIC 被拦网络下永久卡"等待分配公网地址"）
- 更新远端 ingress 的 PUT body 补 `config` 包裹（Cloudflare 1030 错误）
- 钥匙串 → SQLite 迁移改为直读钥匙串（此前搬了 0 条，启动读不到 Token）
- 侧边栏导航高亮互斥（Tooltip asChild 吞掉函数 className 的经典坑）
- 双实例/双托盘图标（single-instance 插件注册）

## [0.1.1] - 2026-10-07

### Changed
- 应用更名「圣手码头」：.app/Dock/托盘/窗口标题改为中文显示名，包内可执行文件改为 `ShengShouMaTou`（规避中文可执行名工具链风险，与圣手捕影命名分层一致）
- Bundle identifier 变更为 `com.cherish.shengshoumatou`，启动时自动迁移旧 `com.masterfulhands.pier` 数据目录（隧道/服务器配置与引擎二进制；钥匙串凭据不受影响）
- 品牌集中管理：Rust `brand.rs` 与前端 `brand.ts` 单一事实来源，中英文界面显示名分离（zh: 圣手码头 / en: Pier）
- 仓库迁移至 `cherish-zp/sheng-shou-ma-tou`（GitHub）与 `princess-zp/sheng-shou-ma-tou`（Gitee）
- macOS 构建链路保留中文 productName（.app/dmg 中文名正确），CI 发布后自动
  PATCH 修复上传环节剥字的附件名；Windows/Linux 构建经 `--config` 覆盖为
  ASCII productName（rpm 规范化/NSIS 对非 ASCII 文件名不可靠）

### Fixed
- 注册 single-instance 插件：杜绝双开（双托盘图标），第二实例启动即退出并置前已有窗口
- `AtomicU32::fetch_update` → `try_update`（消除 Rust 1.95 deprecated 警告）

## [0.1.0] - 2026-10-06

首个公开版本：一座从内网架到公网的桥。

### 新增

**免服务器隧道引擎（M1）**

- **Cloudflare quick tunnel**：本地 HTTP/Web 服务一键映射到临时公网网址（`*.trycloudflare.com`），适合演示与 webhook 调试
- **bore**：任意 TCP 服务（SSH、远程桌面、数据库……）映射到 `bore.pub:端口`
- 零账号、零命令行，开箱即用

**隧道管理（M1）**

- 多隧道并行管理，状态一目了然
- 公网地址一键复制、二维码扫码分享
- 实时日志流与错误摘要
- 断线自动重连：指数退避重试，状态实时同步到界面与系统托盘

**VPS 自建能力（M2）**

- SSH 一键部署 frp 服务端：填入服务器信息即可在自有 VPS 上架桥
- frpc 配置导入：粘贴现有 frpc 配置即可接管既有隧道
- 自定义域名 + 泛解析支持
- 一键诊断：网络、引擎、服务器连通性集中体检

**访问控制与统计（M3）**

- 访问鉴权：隧道密码保护与 IP 白名单
- 流量统计图表：上下行流量可视化

**桌面体验**

- Tauri 2 构建：安装包小、内存占用低
- 暗色 / 亮色双主题，跟随系统
- 中英双语界面（i18n）
- 系统托盘：状态菜单 + 模板图标（自动适配深浅色菜单栏）
- 开机自启

**发布工程（M3）**

- 品牌应用图标全套：macOS `.icns`、Windows `.ico`（16–256 多尺寸）、Microsoft Store 占位图、菜单栏模板图标
- GitHub Actions 三平台 CI（`.github/workflows/build.yml`）：tag 触发自动构建 macOS（Apple Silicon / Intel）、Windows、Linux 产物并创建 Release 草稿；手动触发上传构建产物
- 签名 / 公证流程文档（[docs/RELEASE.md](docs/RELEASE.md)），待配置 Apple Developer 账号与 Windows 证书后启用

[0.1.0]: https://github.com/OWNER/REPO/releases/tag/v0.1.0
