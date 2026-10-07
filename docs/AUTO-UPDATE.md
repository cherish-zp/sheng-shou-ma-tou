# 圣手码头自动更新方案（v0.3.0）

> 本文是该应用自动更新功能的完整方案，**通用可复用**：任何 Tauri 2 项目按此文档即可搭建同款。
> 架构取向：官方 `tauri-plugin-updater` 内核 + dbx 式编排（双源端点、常驻 latest、启动/定时/手动检查）。

## 0. 总览

```
┌─ 发布（CI 自动）────────────────────────────────────────┐
│ tag v* 推送 → tauri-action 四平台构建 + minisign 签名     │
│   → GitHub Release（安装包 + .sig + latest.json）         │
│   → sync-gitee job：附件同步 Gitee Release（中文名）       │
│   → latest.json 重写 URL 为 Gitee 附件 → 上传 Gitee 常驻  │
│     「latest」release（固定 URL，国内快）                  │
└──────────────────────────────────────────────────────────┘
┌─ 应用内更新（用户侧）────────────────────────────────────┐
│ 启动 8s 后静默检查 + 每 60 分钟 + 设置页手动               │
│   → 发现新版本 → 底部横幅（版本号/说明/立即更新/忽略）      │
│   → 下载（进度条）→ 「重启并更新」                          │
│   macOS/Linux：官方原地替换 .app/AppImage 后自动重启        │
│   Windows：NSIS 静默安装后自重启                            │
└──────────────────────────────────────────────────────────┘
```

## 1. 一次性准备（每个项目做一次）

### 1.1 生成签名密钥

```bash
npx @tauri-apps/cli signer generate -w ~/my-keys/app.key -p ""
```

- `*.key.pub` 的内容（去掉注释行）写入 `tauri.conf.json` → `plugins.updater.pubkey`
- **私钥文件妥善备份**（丢失 = 已发布用户永远收不到更新）；base64 编码后存
  GitHub Secrets `TAURI_SIGNING_PRIVATE_KEY`；密码（如有）存
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

### 1.2 配置 `tauri.conf.json`

```jsonc
{
  "bundle": { "createUpdaterArtifacts": true },   // 产出 app.tar.gz/NSIS/AppImage + .sig
  "plugins": {
    "updater": {
      "pubkey": "<1.1 的公钥>",
      "endpoints": [                               // 双源，依次回退
        "https://gitee.com/<org>/<repo>/releases/download/latest/latest.json",
        "https://github.com/<org>/<repo>/releases/latest/download/latest.json"
      ]
    }
  }
}
```

### 1.3 依赖与权限

- Rust：`tauri-plugin-updater`、`tauri-plugin-process`（relaunch 用）
- JS：`@tauri-apps/plugin-updater`、`@tauri-apps/plugin-process`
- capabilities：`updater:default`、`process:allow-restart`
- Rust lib：`.plugin(tauri_plugin_updater::Builder::new().build())` + `.plugin(tauri_plugin_process::init())`

### 1.4 CI（tauri-action）

- env 注入 `TAURI_SIGNING_PRIVATE_KEY`（base64）+ `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- `tauri-action@v0` 自动：构建 → 签名（.sig）→ 生成 `latest.json` → 上传 Release
  （`includeUpdaterJson` 默认开启）

## 2. 双源与 Gitee 常驻 latest（国内可达的关键）

tauri-action 生成的 `latest.json` 中 URL 指向 GitHub 附件——国内不可达。
CI 的 sync-gitee job 在附件同步完成后追加：

1. 读取 `dist/latest.json`
2. **URL 重写**：`https://github.com/<org>/<repo>/releases/download/<tag>/…`
   → `https://gitee.com/<org>/<repo>/releases/download/latest/…`
   （jq：`.platforms |= with_entries(.value.url |= sub(...))`）
3. 上传重写后的 latest.json 到 Gitee **常驻 `latest` release**（不存在则创建；
   先删同名旧附件再上传）
4. 同时把各平台安装包补传到 latest release（latest.json 的下载目标）

这样 updater 首选端点（Gitee）返回的 latest.json 里全部是 Gitee 国内直链。

## 3. 应用内更新（前端）

`useAppUpdater` 状态机：
`idle → checking → available → downloading(progress) → ready → relaunch`，另有
`up-to-date` / `error`。

- 触发：启动延迟 8s 静默检查、每 60 分钟定时、设置页手动
- 「忽略此版本」存 localStorage（`update.ignoredVersion`）
- 下载：`update.downloadAndInstall(cb)`，Started/Progress/Finished 驱动进度条
- 完成：`relaunch()`（Windows NSIS 自重启，无需手动调）
- UI：底部横幅（可用/下载中/待重启/错误四态）+ 设置页「关于与更新」卡片

关键实现文件（本项目）：
`src/composables/use-app-updater.ts`、`src/components/update/update-banner.tsx`、
`src/components/update/update-settings-section.tsx`

## 4. 发版清单

1. 版本号三处同步（tauri.conf.json / Cargo.toml / package.json）+ Cargo.lock
2. CHANGELOG 条目
3. commit + push + annotated tag `vX.Y.Z` + push tag
4. CI 自动完成四平台构建/签名/发布 + Gitee 同步 + latest 维护
5. 验证：GitHub Release 附件含 `.sig` 与 `latest.json`；Gitee latest release
   的 latest.json URL 均为 Gitee 地址；老版本应用内「检查更新」能看到新版本

## 5. 踩坑记录

- **不要以空串形式保留 APPLE_* secrets**：`${{ secrets.X }}` 在 secret 不存在时
  仍注入空串，bundler 检测到变量即走签名导入，直接构建失败（1030/SecKeychain）
- **cargo 增量编译不感知 dist 变化**：打包前 `cargo clean -p <app>` +
  `rm -rf dist && npm run build`，构建后验证产物（体积/mtime）
- **tauri 默认压缩嵌入 assets**：`strings <二进制> | grep <前端串>` 永远为 0，
  不要据此判断包新旧；用 md5/体积/实际运行验证
- **私钥即命脉**：丢失后只能换新密钥对 + 发「必须手动下载」的大版本

## 6. 可选增强（未实施）

- Windows 便携版（zip 单 exe）：移植 dbx `update_portable.rs`
  （minisign 验签 + manifest SHA-256 + PowerShell 备份-替换-回滚）
- 下载磁盘缓存与断点恢复；15s 停滞自动换源；系统代理透传
- 设置页下载源切换（GitHub / Gitee）
