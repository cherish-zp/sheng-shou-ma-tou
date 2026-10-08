# Tauri 2 应用自动更新方案（通用可复用）

> 本文档是**实战验证过的完整方案**——圣手码头 v0.3.0 发布过程踩坑 10+ 轮的全部经验固化。
> 任何 Tauri 2 项目按本文档从零搭建，可以避开我们踩过的每一个坑。
> 架构：官方 `tauri-plugin-updater` 内核 + dbx 式编排（双源端点、常驻 latest、静默/定时/手动检查）。

---

## 0. 总览

```
┌─ 发布（CI 自动，tag 触发）───────────────────────────────┐
│ tauri-action 四平台构建 + minisign 签名（.sig）            │
│   → GitHub Release（安装包 + .sig + latest.json）          │
│   → sync-gitee job：附件同步 Gitee（中文附件名）            │
│   → latest.json 重写 URL 为 Gitee 直链 → 上传 Gitee 常驻   │
│     「latest」release（固定 URL，国内可达的检查端点）        │
└──────────────────────────────────────────────────────────┘
┌─ 应用内更新（用户侧）────────────────────────────────────┐
│ 启动 8s 静默检查 + 每 60 分钟 + 设置页手动                  │
│   → 新版本 → 底部横幅（版本/说明/立即更新/忽略此版本）       │
│   → 下载（进度条）→ 「重启并更新」                          │
│   macOS/Linux：官方原地替换 .app/AppImage + 自动重启        │
│   Windows：NSIS 静默安装（passive）后自重启                 │
└──────────────────────────────────────────────────────────┘
```

## 1. 一次性准备

### 1.1 生成签名密钥

```bash
npx @tauri-apps/cli signer generate -w ~/my-keys/app.key -p ""
```

| 产物 | 用途 |
|---|---|
| `app.key`（**内容即单行 base64**，`dW50…` 开头） | base64 后存 GitHub Secrets `TAURI_SIGNING_PRIVATE_KEY`；文件本身永久备份 |
| `app.key.pub` | 去掉注释行后的字符串写入 `tauri.conf.json` → `plugins.updater.pubkey` |

> ⚠️ **私钥即命脉**：丢失 = 已发布用户永远收不到更新。U 盘/密码管理器双重备份。
> ⚠️ 建议私钥**带尾换行**存储（tauri signer 生成时自带）——CI 归一化逻辑依赖标准文件形态。

### 1.2 `tauri.conf.json`

```jsonc
{
  "version": "X.Y.Z",                          // 与 Cargo.toml/package.json 三处同步
  "bundle": { "createUpdaterArtifacts": true }, // 产出 app.tar.gz/NSIS/AppImage + .sig
  "plugins": {
    "updater": {
      "pubkey": "<1.1 的公钥>",
      "endpoints": [                            // 双源，依次回退
        "https://gitee.com/<org>/<repo>/releases/download/latest/latest.json",
        "https://github.com/<org>/<repo>/releases/latest/download/latest.json"
      ]
    }
  }
}
```

### 1.3 依赖与权限

- Rust：`tauri-plugin-updater`、`tauri-plugin-process`（relaunch）
- JS：`@tauri-apps/plugin-updater`、`@tauri-apps/plugin-process`
- capabilities：`updater:default`、`process:allow-restart`
- lib.rs：`.plugin(tauri_plugin_updater::Builder::new().build())` + `.plugin(tauri_plugin_process::init())`

### 1.4 CI 签名（tauri-action）

- Secrets：`TAURI_SIGNING_PRIVATE_KEY`（= `app.key` 文件内容的 **base64**）
- ⚠️ CI 内必须 **base64 -d 解码为文件再传路径**——直接把 base64 字符串当私钥内容会
  全平台签名失败（见踩坑 #1）；也不要以空串形式保留 APPLE_* 类未用 secrets（见踩坑 #2）

## 2. 双源与 Gitee 常驻 latest（国内可达的关键）

tauri-action 生成的 `latest.json` 中 URL 指向 GitHub 附件——**国内不可达**。
CI 的 sync-gitee job（或本机跑 `scripts/sync-gitee-release.sh`）负责维护
Gitee **常驻 `latest` release**（tag 名 `latest`，不存在则创建），里面只放一个
`latest.json`：

1. **先下载** GitHub Release 全部附件到 `dist/`（重写逻辑依赖本地文件——
   曾把维护块排在下载前导致 `dist/latest.json` 不存在、整块静默空转）；
2. **URL 重写**：每个平台 URL 的文件名剥出来（先 unquote），GitHub 的
   `ShengShouMaTou` 前缀映射为 Gitee 的「圣手码头」前缀，再 percent-encode，
   拼到 `https://gitee.com/<org>/<repo>/releases/download/<TAG>/` ——
   **指向版本化 Release 而非 latest release**，避免把安装包镜像两份；
3. 先删 latest release 上的同名旧附件再上传（幂等，每次发版覆盖）；
4. 版本化 Release 的安装包由附件同步步骤补齐——**>50MB 的大文件 CI 传不动，
   必须本机补传**（见 §5 #7），否则对应平台 URL 404。

> Gitee API token：`gitee.com/profile/personal_access_tokens` 生成，勾 **projects** 权限，
> 存 GitHub Secrets `GITEE_TOKEN`。同步失败可本地跑 `scripts/sync-gitee-release.sh` 兜底
> （curl + python3，无 gh/jq 依赖）。

## 3. 应用内更新（前端）

`useAppUpdater` 状态机：
`idle → checking → available → downloading(progress) → ready → relaunch`，另有 `up-to-date` / `error`。

- 触发：启动延迟 8s 静默检查、每 60 分钟定时、设置页手动
- 「忽略此版本」存 localStorage；token 永不落盘（keychain/SQLite）
- 下载进度：`downloadAndInstall(cb)` 的 Started/Progress/Finished 事件驱动
- UI：底部横幅四态（可用/下载中/待重启/错误）+ 设置页「关于与更新」卡片

## 4. 发版清单（Checklist）

1. 版本号三处同步（tauri.conf.json / Cargo.toml / package.json）+ `cargo update -w`（Cargo.lock）
2. CHANGELOG 条目
3. commit + push + **annotated tag**（message 即发版说明）+ push tag
4. CI 自动完成构建/签名/发布/Gitee 小附件同步/latest 维护
5. **本机补传大附件**（必做）：`GITEE_TOKEN=... ./scripts/sync-gitee-release.sh v0.3.0`
6. 验证：GitHub Release 附件含 `.sig` 与 `latest.json`；Gitee 版本化 Release
   附件齐全；`releases/download/latest/latest.json` 返回 JSON 且 URL 均为
   Gitee 版本化地址、逐个可下载；老版本应用内「检查更新」可见新版本

---

## 5. 踩坑清单（全部实战踩过，逐条付出过代价）

### #1 签名密钥：base64 层数与格式（两层坑）

- **tauri signer 生成的 `.key` 文件内容本身就是 base64**（`dW50…` 开头）——
  用户把它再 base64 一次 → **双层**。CI 解码一次后仍是 base64 → 校验失败。
- **修复**：解码逻辑用**多轮自适应**（最多 3 层），解码到出现
  `untrusted comment` 首行为止；三种输入（原文/单层/双层）全兼容。
- **禁止**：固定"解码一次"的单层假设。

### #2 未使用的签名 secrets：空串注入陷阱

- secret 不存在时 `${{ secrets.X }}` 仍注入**空字符串** env——
  bundler 检测到 `APPLE_CERTIFICATE` 变量存在（哪怕空）即走证书导入 →
  `SecKeychainItemImport: parameters not valid` 全平台失败。
- **修复**：未启用的签名 env **彻底删除**（不能注释留空串）；启用时才加回。

### #3 CI 内 base64 解码：BSD 与 GNU 的差异

- **macOS runner 的系统 `base64` 是 BSD 版**：`-d` 无效（要用 `-D`），报
  `invalid argument`。Linux 的 GNU base64 才认 `-d`。
- **修复**：解码/校验用 **python3**（三个 runner 都自带，行为一致）；
  或按 runner 分支写 `-d`/`-D`。同理编码：GNU `-w 0` vs macOS `-b 0`。

### #4 `/tmp` 路径跨进程不可见（Windows）

- `shell: bash`（Git Bash）写的 `/tmp/x` 是 Git Bash 虚拟路径；
  **Windows 原生进程（python3/node/tauri-action）解析为当前盘符 `\tmp\`** →
  `FileNotFoundError`。
- **修复**：跨进程传递的文件统一放 **`${{ runner.temp }}`**（GitHub 官方
  跨进程一致目录），并在 bash/python/node 间用同一 env 变量传路径。

### #5 bash 语法步骤缺 `shell: bash`（Windows 全挂）

- Windows runner 默认 shell 是 **PowerShell**：bash 语法（`<` 重定向、
  heredoc、`$(...)`、`\` 续行）全部 `ParserError`。
- **修复**：所有含 bash 语法的 `run:` 步骤显式 **`shell: bash`**；
  纯简单命令可留默认。

### #6 GitHub 剥非 ASCII 附件名（平台限制）

- 上传时附件名含非 ASCII（如 `圣手码头_0.3.0.dmg`）会被 GitHub **剥成 `_0.3.0.dmg`**；
  事后用 API PATCH 改回中文名**也无效**（存储层行为）。
- **修复**：附件名统一 **ASCII**（如 `ShengShouMaTou_0.3.0_aarch64.dmg`）；
  应用内显示名保留中文；中文附件名放 Gitee（Gitee 支持，同步脚本上传时重命名）。
- 事后补救：Release **编辑页**的附件名输入框可手动改（ASCII 名可稳定保存）。

### #7 跨境大文件上传 Gitee 不可行，且 Gitee 不能自己构建

- US runner → Gitee 传 81MB：curl 速度显示 1.6MB/s"正常"，但**进度 0% 且内部
  计时 1h22m+**（链路拖慢），单次 30 分钟超时被掐 → 重试 → 再超时。
- **修复**：sync 脚本对 **>50MB 附件跳过**并输出 `::notice` 提示本地兜底
  （`scripts/sync-gitee-release.sh` 本地网络上传快）；**本机补传是发版必做项**。
- 小文件（≤50MB）正常上传 + 6 次重试 + 失败计数。
- **别指望 Gitee 自己构建**：Gitee Go 云端 runner 只有 Linux 容器（1C2G–8C16G），
  没有 macOS/Windows 构建机，官方编译插件也没有 Rust——Tauri 的 dmg/msi 无法
  在 Gitee 云端产出。25k star 的 dbx 也是这么选的：Gitee 只做**纯代码镜像**
  （git push 分支+tag，零发行版），国内分发走 CNB + 自建对象存储。
- **平台配额**：Gitee 附件**单文件 100MB**、**单仓库附件总容量 1GB**（社区版）
  ——版本发多了注意清理旧版本大附件；Gitee Pages 已下线，不能用它托管
  latest.json（用常驻 latest release 挂附件是可行替代）。

### #8 QUIC 被拦网络（应用运行时 + 都要注意）

- 部分国内网络拦截 UDP 443：cloudflared **auto 协议不降级**，永久卡
  "等待连接"（quick tunnel 表现为边缘 530）。
- **修复**：所有 cloudflared 调用**默认 `--protocol http2`**（TCP 443 全网可达）。

### #9 minisign 密钥文件的尾换行

- tauri 生成的 `.key`（base64 形式）解码出的 minisign 原文**以换行结尾**；
  重新编码时若丢尾换行，解码字节与 tauri 生成的 `.key` 差 2 字节 →
  **与内嵌公钥不配对**（实测 md5 对比抓到）。
- **修复**：raw 分支编码前 `printf '%s\n'`（补标准尾换行）。

### #10 诊断日志的可靠性

- `gh run view --log-failed` 对真实失败可能**返回空**（实测）。
- **修复**：诊断 Issue 用 **`gh api repos/{repo}/actions/jobs/{id}/logs`**
  REST 端点拉日志尾部（注意：job 运行中或刚结束时可能 404，需在失败后
  稍候拉取）；同时保留 --log-failed 后备。

### #11 关窗 ≠ 退出（验证时的头号错觉）

- macOS 关闭窗口只是关窗口，应用仍在后台运行——用户"打开"看到的
  可能是**内存里的旧界面**，新装的代码永远不生效。
- **验证前必须**：`pgrep -fl "<dev二进制名>|<app名>"` 双名排查（dev 二进制名
  可能与产品名不同）+ `osascript quit` 或 pkill 全清，再启动新版。

### #12 tauri build 的前端嵌入缓存

- `generate_context!` 宏在编译期嵌入 `dist/`，**cargo 增量编译不感知
  dist 内容变化**（touch 也不可靠）。
- **修复**：正式打包前 `cargo clean -p <app>` + `rm -rf dist && npm run build`
  + dist 特征串验证 → 构建后验证二进制（体积/md5 差异，**不要用 strings
  grep 前端内容**——assets 压缩嵌入，永远搜不到）。

---

### #13 矩阵并行构建合并 latest.json 丢平台键（macOS 用户全挂）

- tauri-action 并行矩阵构建各自上传 `latest.json`（下载已有 → 合并 → 重传），
  实测合并结果**只剩 linux/windows 键，darwin-aarch64 / darwin-x86_64 整个
  缺失**（疑似 CJK 产物名导致其按名匹配签名文件失败后静默跳过 macOS 条目）。
  macOS 用户检查更新报：`None of the fallback platforms
  ["darwin-aarch64-app", "darwin-aarch64"] were found in the response
  platforms object`——**连"已是最新"都不显示**，因为平台匹配发生在版本比较之前。
- **修复**：不信任 tauri-action 的合并。tag 发版后由独立 job
  （assemble-updater-json）从 Release 的 `.sig` 附件**确定性重组** latest.json：
  版本取 tag、签名取对应 `.sig` 文件内容、URL 取 `browser_download_url`；
  平台键按附件名**后缀**匹配（同一附件可挂多键：updater 会依次探测
  `darwin-aarch64-app` → `darwin-aarch64`）；缺任一必需平台直接 fail。
- **别在网页端上传 latest.json**：GitHub 网页上传按扩展名白名单拦截
  （"We don't support that file type"），`.json` 不在列——网页只能删附件，
  替换内容只能走 API/CI。
- workflow_dispatch 支持 `tag` 输入：跳过构建、仅重跑 patch/清单组装/Gitee
  同步——发版后修补**不需要重打 tag 重构建**。注意：needs 链上被跳过的 job
  会**默认连带跳过**下游（GitHub 隐式 success() 检查），需在下游 if 里用
  `!cancelled() && needs.X.result != 'failure'` 显式接管。

## 6. 可选增强（未实施，按需）

- Windows 便携版（zip 单 exe）：移植 dbx `update_portable.rs`
  （minisign 验签 + manifest SHA-256 + PowerShell 备份-替换-回滚）
- 下载磁盘缓存与断点恢复；15s 停滞自动换源；系统代理透传
- 设置页下载源切换（GitHub / Gitee）；增量更新

## 7. 快速接入清单（别的项目照抄）

1. §1.1 生成密钥 → §1.2 配置 → §1.3 依赖 → §1.4 CI 签名
2. §2 双源与 Gitee latest 维护（照抄 build.yml 的 sync-gitee job）
3. §3 前端 composable + 横幅 + 设置页（照抄本项目
   `src/composables/use-app-updater.ts`、`src/components/update/*`）
4. §5 踩坑清单过一遍（每条都是真实付出过代价的）
5. 本地签名构建验证（`.sig` 生成）→ 发版 checklist → 发布

> 本项目参考实现：`sheng-shou-ma-tou` 仓库
> `.github/workflows/build.yml`、`src/composables/use-app-updater.ts`、
> `src/components/update/*`、`scripts/sync-gitee-release.sh`
