//! 应用品牌单一事实来源（2026-10 由 Pier 更名「圣手码头」）。
//!
//! 用户可见名称一律引用这里，避免散落硬编码。
//! 命名分层与参考项目一致：
//!   - 用户可见层（.app/Dock/托盘 tooltip）：中文「圣手码头」
//!   - 包内可执行文件名：拼音驼峰 `ShengShouMaTou`（tauri.conf.json 的
//!     `mainBinaryName`，规避中文可执行名的工具链兼容风险）
//!   - Cargo 包名保持 `pier`（内部名，改名无收益且牵动全链路）

/// 中文显示名 = productName（.app / Dock / 窗口标题 / 托盘 tooltip）。
/// CI 产物附件名的中文字符由 tauri-action 上传时剥除，流水线在
/// 发布后用 GitHub API PATCH 附件名修复（见 build.yml 的
/// patch-release-names job）；Windows/Linux 构建通过 --config 覆盖
/// 为 ASCII productName（rpm/NSIS 工具链对非 ASCII 文件名不可靠）。
pub const DISPLAY_NAME_ZH: &str = "圣手码头";

/// 英文界面显示名（i18n en 场景的 app.name；Rust 侧预留引用）。
#[allow(dead_code)]
pub const DISPLAY_NAME_EN: &str = "Pier";

/// 包内可执行文件名（拉丁，与 tauri.conf.json mainBinaryName 对应）。
#[allow(dead_code)]
pub const EXECUTABLE_NAME: &str = "ShengShouMaTou";

/// 旧 bundle identifier（数据目录迁移来源）。
pub const LEGACY_IDENTIFIER: &str = "com.masterfulhands.pier";
