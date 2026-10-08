# Pier 发布指南（Release）

本文档描述 Pier 的完整发布流程：版本号管理、三平台 CI 构建、macOS 签名与公证、
Windows 代码签名、以及更新器（updater）的启用路线。

> 占位约定：`OWNER/REPO` 指仓库地址（如 `masterfulhands/pier`），替换为实际值。

---

## 0. 前置条件一览

| 平台 | 需要什么 | 费用 | 当前状态 |
|---|---|---|---|
| 全平台 | GitHub 仓库 Actions 已启用 | 免费 | 就绪 |
| macOS | Apple Developer Program 账号 | $99/年 | 待配置 |
| macOS | `Developer ID Application` 证书 | 含在上面的年费里 | 待配置 |
| Windows | OV 或 EV 代码签名证书（如 SSL.com、Certum、Sectigo） | 数十~数百美元/年 | 待配置 |
| 更新器 | 一对 `tauri signer` 密钥 | 免费 | 待启用 |

未配置签名时，产物**可以正常发布**，只是：

- macOS 用户首次打开需右键 → 打开（Gatekeeper 提示"未公证"）；
- Windows SmartScreen 会拦截未签名 exe，用户需点"仍要运行"。

---

## 1. 版本发布流程

### 1.1 发布一个新版本

1. **同步版本号**（三处保持一致）：
   - `package.json` → `"version"`
   - `src-tauri/tauri.conf.json` → `"version"`（CI 的 Release 名称用这里的版本渲染）
   - `src-tauri/Cargo.toml` → `[package] version`（改完在 `src-tauri/` 下跑一次
     `cargo check` 刷新 `Cargo.lock`）
2. **更新 `CHANGELOG.md`**：把本版本的变更从"未发布"整理成新版本小节，日期用发布日。
3. **提交并打 tag**：
   ```bash
   git add -A && git commit -m "release: v0.2.0"
   git tag v0.2.0
   git push origin main --tags
   ```
4. **CI 自动执行**（`.github/workflows/build.yml`）：
   - `check` job：TypeScript 类型检查 + 前端构建 + `cargo test`（ubuntu）；
   - `build` job：四条流水线并行 —— macOS arm64、macOS x86_64、Windows x64、Linux x64；
   - tag 触发时，[tauri-action](https://github.com/tauri-apps/tauri-action) 自动创建
     **Release 草稿**（Draft），并附上全部产物。
5. **人工验收草稿**：到 GitHub Releases 下载草稿产物，按下文 §5 检查清单逐项 smoke test。
6. **正式发布**：草稿页点 "Publish"（或用 `gh release edit v0.2.0 --draft=false`）。
7. **同步到 Gitee**（国内分发，见 §1.2）：CI 自动传小文件，大附件需本机补传。

### 1.2 Gitee 同步与本地补传（国内分发，必做）

Gitee 承担国内下载源与自动更新首选端点，但有两个硬约束（实测结论）：

- **Gitee Go 无法多平台构建**：云端 runner 只有 Linux 容器，没有 macOS/Windows
  构建机（官方编译插件也无 Rust）——产物只能在 GitHub Actions 构建；
- **跨境上传大文件不可行**：>50MB 的附件从 GitHub Actions（US runner）上传 Gitee
  会长时间卡死（83MB AppImage 实测卡 3 小时 0%）。Gitee 附件单文件上限 100MB、
  单仓库附件总容量 1GB（发新版时可清理旧版本大附件）。

因此流程是「CI 传小文件 + 本机补传大文件」：

1. **CI 自动完成**（tag push 后 sync-gitee job）：创建 Gitee Release、上传全部
   <50MB 附件（.sig、deb、rpm、exe、msi、dmg 等），并把 `latest.json` 重写为
   Gitee 版本化 URL 后覆盖上传到常驻 `latest` release（自动更新检查端点）。
2. **本机补传大文件**（如 83MB AppImage，境内→境内直传）：
   ```bash
   GITEE_TOKEN=<gitee私人令牌，勾 projects 权限> \
     ./scripts/sync-gitee-release.sh v0.3.0
   ```
   脚本幂等：只下载/上传 Gitee 缺失的附件，同时维护 `latest` release 的
   latest.json。**这一步是发版的必做项**——AppImage 用户的更新端点依赖它。
3. **验收 Gitee**：
   ```bash
   # 版本化 Release 附件齐全（除 >50MB 需补传的）
   curl -s "https://gitee.com/api/v5/repos/princess-zp/shengShouMaTou/releases/tags/v0.3.0" | jq '.assets[].name'
   # 自动更新端点可用（应返回 JSON，URL 全部指向 gitee.com）
   curl -s "https://gitee.com/princess-zp/shengShouMaTou/releases/download/latest/latest.json"
   ```

### 1.3 手动触发一次构建（不发布）

Actions 页面选择 `build` workflow → `Run workflow`。产物以
`pier-macos-arm64` / `pier-macos-intel` / `pier-windows-x64` / `pier-linux-x64`
为名上传到 workflow artifacts，不创建 Release。

---

## 2. macOS 签名 + 公证

Tauri 2 的 bundler 在检测到 `APPLE_*` 环境变量时会**自动完成签名、公证（notarize）
与 staple**。CI 中 `.github/workflows/build.yml` 已预置这些变量，只需配置 Secrets。

### 2.1 导出证书并配置 Secrets

1. 确认已加入 Apple Developer Program，且在
   [developer.apple.com/account/resources/certs](https://developer.apple.com/account/resources/certs/list)
   创建了 **Developer ID Application** 证书（注意不是 "Apple Development"）。
2. 打开 macOS「钥匙串访问」，找到该证书，右键 → 导出，选 `.p12` 格式，设置导出密码。
3. 生成 base64 并配到仓库 Secrets（Settings → Secrets and variables → Actions）：
   ```bash
   base64 -i developer-id.p12 | pbcopy
   ```
   | Secret 名 | 值 |
   |---|---|
   | `APPLE_CERTIFICATE` | p12 的 base64 内容（含换行也能识别） |
   | `APPLE_CERTIFICATE_PASSWORD` | 导出 p12 时设置的密码 |
   | `APPLE_SIGNING_IDENTITY` | `Developer ID Application: 你的名字 (TEAMID)` |
   | `APPLE_ID` | 公证用 Apple ID（App 专用密码流程） |
   | `APPLE_PASSWORD` | 在 appleid.apple.com 生成的**App 专用密码**（不是登录密码） |
   | `APPLE_TEAM_ID` | 开发者团队 ID（账号详情页 10 位字母数字） |

   > 也可改用 App Store Connect API Key（`APPLE_API_ISSUER` / `APPLE_API_KEY` /
   > `APPLE_API_KEY_PATH`）代替 `APPLE_ID` + `APPLE_PASSWORD`，更适合团队。

### 2.2 tauri.conf.json 配置

```jsonc
"bundle": {
  "macOS": {
    "minimumSystemVersion": "10.15",   // 已配置
    "signingIdentity": "Developer ID Application: ... (TEAMID)" // 待拿到证书后填写，
    // 或者留空，在 CI 中由 APPLE_SIGNING_IDENTITY 环境变量驱动（推荐，密钥不进代码库）
  }
}
```

JSON 不支持注释，实际配置时只写 `"signingIdentity": "..."` 或干脆不写该字段。

### 2.3 验证签名与公证

```bash
codesign -dv --verbose=4 "Pier.app"            # 应显示 Authority=Developer ID Application ...
codesign --verify --deep --strict --verbose=2 "Pier.app"
spctl -a -t run -vv "Pier.app"                 # accepted source=Notarized Developer ID
xcrun stapler validate "Pier.app"
```

公证若失败，用 `xcrun notarytool log <submission-id> --apple-id ... --password ... --team-id ...`
查看 Apple 返回的详细原因。

### 2.4 可选：universal2 合并包

当前 CI 分别出 arm64 / x86_64 两个 dmg。若想出一个通用包，把 workflow 里两个
macOS job 合并为一个，参数改为：

```yaml
- platform: macos-latest
  args: --target universal-apple-darwin
```

（runner 会自动装两个 target；缺点是构建时间变长、包体积约翻倍。）

---

## 3. Windows 代码签名

### 3.1 证书选择

| 类型 | 存储方式 | 特点 |
|---|---|---|
| OV（组织验证） | 普通 .pfx 文件 | 便宜；首次累积 SmartScreen 信誉较慢 |
| EV（扩展验证） | 硬件 U 盾 / 云签名 KMS | 即时 SmartScreen 信誉；CI 上需云签名工具 |

### 3.2 路径 A：本地 signtool（OV 证书）

拿到 `.pfx` 后，在本机导入证书库（双击安装，记录证书**指纹**），然后在
`src-tauri/tauri.conf.json` 的 bundle 区配置：

```json
"windows": {
  "certificateThumbprint": "证书SHA1指纹（不带空格）",
  "digestAlgorithm": "sha256",
  "timestampUrl": "http://timestamp.digicert.com"
}
```

之后 `npm run tauri build` 即自动签名 MSI/NSIS 安装包。
（此配置包含真实指纹后不要提交到公开仓库，建议仅在发布机或 CI 环境变量中提供。）

### 3.3 路径 B：CI 云签名（EV 证书）

EV 证书私钥在云 KMS 里，无法导出 p12，常用两种做法：

- **SSL.com eSigner**：安装 [eSignerCKA](https://www.ssl.com/how-to/esignercka-manual/)
  作为本地签名子代理，在 CI 中 tauri 构建完成后对 `bundle/**/*.exe|*.msi` 执行
  `signtool sign /fd sha256 /tr http://timestamp.ssl.com ...`；
- **Azure Trusted Signing**：使用官方 `azure/trusted-signing-action` 对产物批量签名，
  适合已有 Azure 账号的团队。

两种都是"先 `tauri build`、后对产物签名"，在 `.github/workflows/build.yml` 的
build job 末尾插入签名 step 即可（当前文件已留注释占位）。

### 3.4 注意区分：更新器签名 ≠ 代码签名

`TAURI_SIGNING_PRIVATE_KEY` 是 **tauri-plugin-updater** 用来给更新包签名的密钥对
（minisign 风格），与 Windows 代码签名无关，见下一节。

---

## 4. 更新器（tauri-plugin-updater）启用路线

Tauri 2 官方更新插件，支持 macOS / Windows / Linux 自动检查并静默/提示升级。

1. **生成密钥对**（只需一次，私钥妥善保管、永不入库）：
   ```bash
   npx @tauri-apps/cli signer generate -w ~/.tauri/pier.key
   # 输出公钥，记下来
   ```
2. **安装插件**：
   ```bash
   npm i @tauri-apps/plugin-updater
   cd src-tauri && cargo add tauri-plugin-updater
   ```
   在 `lib.rs` 注册插件（`.plugin(tauri_plugin_updater::Builder::new().build())`），
   并在 capability 文件中加 `"updater:default"` 权限。
3. **配置 `tauri.conf.json`**：
   ```json
   "plugins": {
     "updater": {
       "pubkey": "<第 1 步输出的公钥>",
       "endpoints": [
         "https://github.com/OWNER/REPO/releases/latest/download/latest.json"
       ]
     }
   }
   ```
4. **CI 产出更新清单**：在 `.github/workflows/build.yml` 的 tauri-action 参数加
   `includeUpdaterJson: true`，并注入：
   ```yaml
   env:
     TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
     TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
   ```
   （workflow 中已留注释占位。）tauri-action 会用私钥给 `.app.tar.gz` / NSIS 包签名，
   并生成 `latest.json` 附加到 Release —— 正好匹配第 3 步的 endpoint。
5. 前端用 `check()` / `downloadAndInstall()`（`@tauri-apps/plugin-updater`）实现更新 UI。

---

## 5. 发布前检查清单

每个 Release 草稿发布前至少验证：

- [ ] **产物齐全**：macOS arm64 `.dmg`、macOS Intel `.dmg`、Windows `.msi` 与 `-setup.exe`（NSIS）、Linux `.deb` / `.AppImage`
- [ ] **图标**：安装后 Dock / 任务栏显示拱桥图标，菜单栏托盘为模板剪影
- [ ] **macOS**：双主题、隧道创建/复制/二维码、重连、托盘菜单可用
- [ ] **Windows**：MSI 与 NSIS 两种安装器都能装上并启动
- [ ] **版本一致**：关于页 / `tauri.conf.json` / Release tag 三处版本号一致
- [ ] **CHANGELOG**：已更新且发布说明指向它
- [ ] 签名启用后：macOS 通过 `spctl -a -t run`，Windows 无 SmartScreen 拦截（EV）

## 6. Hotfix 流程

1. 从出问题的 tag 拉 `hotfix/x` 分支修复；
2. 版本号 +1（如 `v0.1.1`），走 §1 完整流程；
3. 不建议删除已发布的 tag 重打（updater endpoint 指向 `latest`，重打会造成已下载用户
   的清单错乱）。
