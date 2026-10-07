# 功能共识：Cloudflare 固定域名（Named Tunnel）— v0.2.0

> 状态：待用户最终确认
> 日期：2026-10-07
> 调研依据：Cloudflare 官方文档核对（cfd_tunnel API、remote-management、account-limits）

## 1. 问题与答案

**问题**：Quick tunnel（trycloudflare.com）每次启动域名随机，无法固定（官方设计如此，无 SLA、定位"测试用途"）。

**答案**：接入 Cloudflare **Named Tunnel**（正式产品，免费计划含，每账户 1000 隧道、无带宽限制）——用户粘贴一个 API Token，圣手码头全自动完成隧道创建、域名路由、启动，此后域名永久固定（`https://子域名.你的域名`），断线重连/重启应用/换机器均不变。

**用户前提**：一个托管在 Cloudflare 的域名（NS 指向 Cloudflare 免费）；唯一付费例外是三级以上子域名（`a.b.example.com` 需 Advanced Certificate Manager）。

## 2. 已拍板决策（5 项）

| # | 决策点 | 结论 |
|---|--------|------|
| 1 | 入口形态 | 向导内 Cloudflare 通道两档：临时域名（默认零前提）/ 固定域名（绑定流程内嵌向导）；设置页管理已绑定账号 |
| 2 | 授权方式 | 粘贴 API Token（应用内嵌带预填权限的 token 创建直达链接）；token 存系统钥匙串 |
| 3 | 域名体验 | 全自动：token 验证后列出全部托管域名（下拉选）→ 填子域名 → DNS CNAME + 隧道路由自动创建，同名冲突自动复用 |
| 4 | 隧道粒度 | 一条固定域名隧道 = 一个 Cloudflare tunnel 对象 = 一条 ingress（额度 1000 足够）；共享隧道二期 |
| 5 | 管理位置 | 远程管理（`config_src: cloudflare`）：ingress 存云端，用户可在 Cloudflare Dashboard 自助查看/修改；`cloudflared` 仅需 token 启动 |

## 3. 技术设计

### 3.1 用户流程（3 步）

1. **粘贴 API Token**：应用内"创建 Token"指引（直达 dash.cloudflare.com/profile/api-tokens，权限三件套：Account `Cloudflare Tunnel:Edit` + Zone `DNS:Edit` + Zone `Read`）
2. **选域名 + 填子域名**：token 验证通过后自动列出托管域名
3. **一键创建并启动**：自动执行隧道创建 → ingress 写入 → CNAME 创建 → `cloudflared tunnel run` 启动 → 固定域名上线

### 3.2 API 链路（Rust 新模块 `cloudflare.rs`）

| 步骤 | 端点 |
|---|---|
| 验证 token + 列账户 | `GET /accounts` |
| 列域名 | `GET /zones` |
| 创建隧道（响应含 id + token） | `POST /accounts/{id}/cfd_tunnel` body `{name, config_src:"cloudflare"}` |
| 写 ingress（末尾 catch-all 404） | `PUT /accounts/{id}/cfd_tunnel/{tid}/configurations` |
| 建 CNAME（`<tid>.cfargotunnel.com`，proxied:true；81053/81057 同名冲突→查询复用） | `POST /zones/{zid}/dns_records` |
| 查健康状态（inactive→healthy） | `GET /accounts/{id}/cfd_tunnel/{tid}` |
| 删除隧道（卸载时，提示 DNS 记录一并清理） | `DELETE /accounts/{id}/cfd_tunnel/{tid}` |

### 3.3 引擎与数据模型

- `Backend::CloudflareNamed` 新增；复用同一 cloudflared 二进制
- 启动命令：`cloudflared tunnel --no-autoupdate run --token <t>`（token 经 **TUNNEL_TOKEN 环境变量**传入，不出现在进程参数里）；就绪判定：日志含 "Registered tunnel connection"
- `public_url = https://{subdomain}.{domain}`（创建时确定，永不变）
- 密钥存储：API Token → keychain `cf-api-token`；隧道运行 token → keychain `cf-tunnel-token-{tunnel_id}`
- 隧道类型：**仅 HTTP**（named tunnel 对普通访客的 TCP 需访问端跑 cloudflared access，二期考虑）

### 3.4 前端

- 向导 Cloudflare 通道两档卡片；固定域名档 = 3 步流程（贴 token → 选域名/填子域 → 完成创建启动），带进度与错误指引（token 权限不足/域名无托管等）
- 设置页：Cloudflare 绑定管理（token 状态、更换、解绑——解绑提示是否同时清理云端隧道与 DNS）
- i18n 中英双语全量

### 3.5 边界与风险

- Cloudflare 出站 7844 端口被防火墙拦则连不上（诊断规则补一条）
- 删除隧道不会自动清 DNS 记录（应用内卸载时主动清理 + 提示）
- token 轮换：Cloudflare 建议定期轮换 tunnel token——旧 token 失效后需重新 provision（设置页提供"重新生成隧道"入口）
- 免费 TLS 仅覆盖 `*.example.com` 一级子域名

## 4. 实施拆分（并行三线）

- **R7（Rust）**：cloudflare.rs API 客户端 + Backend::CloudflareNamed Provider + engine 分支 + commands（verify/zones/provision/deprovision）+ 契约扩展
- **F4（前端）**：向导两档 + 3 步绑定流程 UI + 设置页账号管理 + i18n
- 集成联调 + 本地真实 Cloudflare 账号端到端验证（需要用户提供一个托管在 CF 的域名与 token 做验收）

版本：**v0.2.0**
