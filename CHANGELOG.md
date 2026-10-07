# 更新日志

本项目所有显著变更将记录于此文件。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本 2.0.0](https://semver.org/lang/zh-CN/)。

## [0.1.1] - 2026-10-07

### Changed
- 应用更名「圣手码头」：.app/Dock/托盘/窗口标题改为中文显示名，包内可执行文件改为 `ShengShouMaTou`（规避中文可执行名工具链风险，与圣手捕影命名分层一致）
- Bundle identifier 变更为 `com.cherish.shengshoumatou`，启动时自动迁移旧 `com.masterfulhands.pier` 数据目录（隧道/服务器配置与引擎二进制；钥匙串凭据不受影响）
- 品牌集中管理：Rust `brand.rs` 与前端 `brand.ts` 单一事实来源，中英文界面显示名分离（zh: 圣手码头 / en: Pier）
- 仓库迁移至 `cherish-zp/sheng-shou-ma-tou`（GitHub）与 `princess-zp/sheng-shou-ma-tou`（Gitee）

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
