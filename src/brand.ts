// 应用品牌单一事实来源（2026-10 由 Pier 更名「圣手码头」）。
// UI 可见名称一律经 i18n 的 app.name 渲染（zh: 圣手码头 / en: Pier），
// 这里保留常量供非 i18n 场景（如 document.title 兜底）引用。

/** 中文显示名（.app / Dock / 托盘，与 tauri.conf.json productName 一致）。 */
export const DISPLAY_NAME_ZH = "圣手码头";

/** 英文界面显示名（沿用既有英文品牌）。 */
export const DISPLAY_NAME_EN = "Pier";

/** 包内可执行文件名（tauri.conf.json mainBinaryName）。 */
export const EXECUTABLE_NAME = "ShengShouMaTou";
