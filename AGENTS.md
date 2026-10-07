# 圣手码头（Pier）开发规范

> 本文件记录开发/发布过程中的强制性规则与踩坑教训。改动构建、打包、发布流程前必读。

## 1. 构建产物与 Spotlight（教训：系统出现两个应用图标）

- **`tauri build` 每次都会在 `src-tauri/target/*/release/bundle/macos/` 重新生成 `.app`**，
  macOS Spotlight 会索引它 → 系统搜索出现重复应用。
- **强制规则**：
  1. `src-tauri/target/.metadata_never_index` 空文件必须存在（目录级禁止索引）；
  2. 系统唯一安装副本只认 **`/Applications/圣手码头.app`**；
  3. 发现重复：删除 target 内的 `.app`，勿动 /Applications。

## 2. tauri build 的前端嵌入缓存坑（教训：安装包跑的是旧界面）

- `generate_context!` 宏在编译期嵌入 `dist/`，但 **cargo 增量编译不感知 dist 内容变化**
  （touch 源文件也不可靠），导致新前端代码构建出的二进制仍嵌旧页面。
- **强制规则**：给用户安装的打包必须按此顺序，且每步验证：
  1. `rm -rf dist && npm run build`
  2. `grep -c "<本次新功能的特征字符串>" dist/assets/*.js` → 必须 ≥1
  3. `cargo clean -p pier --manifest-path src-tauri/Cargo.toml`（最可靠）
  4. `npm run tauri build -- --target aarch64-apple-darwin`
  5. `strings <二进制> | grep -c "<特征串>"` → ≥1 才允许安装
     （若 grep 不到：前端内容可能被 tauri 压缩嵌入，改用二进制体积差异或实际运行验证）

## 3. 秘密存储（v0.2.0 起：本地 SQLite，禁用钥匙串）

- 所有 Token/密码统一存 `secrets.db`（app 数据目录，`secrets_store` 模块）。
- **禁止再引入 macOS 钥匙串**：未签名应用每次重建都被钥匙串 ACL 当作新应用，
  读一次弹一次密码框（v0.2.0 前的真实事故）。
- 钥匙串历史条目已迁移；如见残留弹窗，先查是否有旧实例进程在跑
  （进程名可能是 `pier`，`pkill -f ShengShouMaTou` 杀不到它）。

## 4. Named Tunnel（固定域名）

- cloudflared 统一 `--protocol http2`：QUIC（UDP 443）在多数国内网络被拦，
  auto 模式不会降级（实测卡死）。所有 cloudflared 调用点都不得改回默认。
- Cloudflare API `PUT .../configurations` 的 body 必须嵌 `{"config": {...}}`，
  裸 ingress 数组报 1030。
- 固定域名可修改：编辑面板改子域名 → `cf_update_hostname`（更新 ingress
  hostname + CNAME 换绑 + 旧记录清理），隧道对象与运行 token 不变。

## 5. 发版流程

见 `docs/RELEASE.md`。要点：版本号三处同步（tauri.conf.json / Cargo.toml /
package.json）→ annotated tag（message 即发版说明）→ push 触发四平台 CI →
GitHub Release 自动发布 → sync-gitee 同步附件（需 GITEE_TOKEN，未配置时本地跑
`scripts/sync-gitee-release.sh`）。

## 6. 其他硬规则

- 验证脚本化：`cargo test` + `cargo clippy --all-targets`（零警告）+
  `npm run build` + i18n key 双语一致性比对，四项全绿才允许提交发布。
- 提交遵循 Conventional Commits、中文描述。
- 双实例排查：确认系统里只有一个应用进程时，
  `ps aux | grep -iE "[p]ier|[S]hengShou"`（注意 dev 二进制名叫 `pier`）。
