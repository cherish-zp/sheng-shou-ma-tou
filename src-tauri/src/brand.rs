//! 应用品牌单一事实来源（2026-10 由 Pier 更名「圣手码头」）。
//!
//! 用户可见名称一律引用这里，避免散落硬编码。
//! 命名分层与参考项目一致：
//!   - 用户可见层（.app/Dock/托盘 tooltip）：中文「圣手码头」
//!   - 包内可执行文件名：拼音驼峰 `ShengShouMaTou`（tauri.conf.json 的
//!     `mainBinaryName`，规避中文可执行名的工具链兼容风险）
//!   - Cargo 包名保持 `pier`（内部名，改名无收益且牵动全链路）

/// 中文显示名（Dock / .app / 托盘 tooltip / 窗口标题）。
pub const DISPLAY_NAME_ZH: &str = "圣手码头";

/// 英文界面显示名（i18n 英文场景沿用既有英文品牌）。
pub const DISPLAY_NAME_EN: &str = "Pier";

/// 包内可执行文件名（拉丁）。
pub const EXECUTABLE_NAME: &str = "ShengShouMaTou";

/// 旧 bundle identifier（数据目录迁移来源）。
pub const LEGACY_IDENTIFIER: &str = "com.masterfulhands.pier";
