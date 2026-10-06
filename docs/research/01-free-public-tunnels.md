# 调研报告 1：免费公共隧道服务（2025-2026 现状）

> 调研方法：官方文档/定价页/ToS 原文抓取 + GitHub Release 元数据实测 + bore.pub 端到端连通实测（2026-10-06）

## 一、总对比表

| 服务 | 需账号 | 免费额度 | 协议 | 稳定性口碑 | 许可证/ToS 允许第三方集成 | 客户端跨平台/大小 |
|---|---|---|---|---|---|---|
| **Cloudflare quick tunnel**（trycloudflare.com） | 否（named tunnel 才要） | 完全免费、不限时长 | HTTP/HTTPS 为主；任意 TCP/UDP 需访客侧也跑 cloudflared | 高（大厂边缘网络），但官方声明"无运行时间保证"，曾因滥用被部分 DNS（如 OpenDNS）封锁 | 二进制 Apache-2.0 可分发；服务条款明确定位"testing and development"，未禁止第三方调用但不宜承诺 SLA | macOS pkg 19–21MB、Windows exe 52.8MB/msi 18.6MB、Linux 全架构 |
| **ngrok 免费层** | 是（authtoken） | 3 个在线端点、1 个固定域名、1GB/月、2 万 HTTP 请求/月、TCP 100 连接/分、HTTP 4000 请求/分、有插页警告页 | HTTP/HTTPS/TCP/TLS（无 UDP） | 高（商业服务） | agent 二进制闭源；ToS 允许"以自己账号向第三方分发 agent 配套自己的应用"，用终端用户自己账号分发需书面同意；官方推荐路径是 MIT 的 ngrok-go SDK | macOS zip 11–12.5MB、Windows zip 12.3MB（解压约 25MB+） |
| **bore（bore.pub）** | 否 | 完全免费、无带宽声明 | 仅 TCP（无 UDP、无 TLS，公网是 http://bore.pub:随机高端口） | 中：无 SLA 的个人维护服务端，但本次实测端到端可用（分配 15967 端口并成功回源）；仓库 2026-02 仍在维护，11.5k stars | MIT，客户端/服务端均可自由分发与自建 | 单一 Rust 静态二进制 2.4MB（压缩包 0.9MB），macOS/Windows/Linux(含 musl) 全覆盖 |
| **zrok**（NetFoundry SaaS） | 是（邮箱注册） | 5GB/天、25 环境、50 个 share、保留域名；未绑卡的免费账号有插页页 | 公共 share 为 HTTP(S)；私有 share 支持 HTTP/TCP/UDP/VPN | 较高（OpenZiti 商业团队运营），v2.0.7 活跃迭代（2026-10 发布） | Apache-2.0，官方明确支持自建且"自建无限制"，无分发障碍 | Go 单二进制：darwin amd64/arm64、windows amd64、linux 全架构，压缩包约 30MB |
| **localtunnel** | 否 | 免费、无时长限制 | 仅 HTTP/HTTPS | 低：项目维护弱、无 IP 级子域名认证存在子域名抢注风险（issue #676）、有强制插页页 | MIT 开源可分发；作为商业产品后端的口碑差 | Node.js npm 包，桌面应用需捆绑 Node 或移植客户端，集成成本高 |
| **localhost.run** | 否（付费才要） | 免费随机 URL、短时会话，无自定义域名 | HTTP/HTTPS（SSH 隧道承载） | 中高：可靠、简单，免费层有请求节流 | 基于 SSH，无客户端需分发；商业集成实质导向其 $9/月 订阅 | 无需客户端（纯 ssh -R），macOS/Windows(内置 OpenSSH) 均可 |
| **serveo** | 否 | 免费匿名隧道 | HTTP/TCP（SSH 承载） | 低：历史多次长时间宕机，社区口碑"只适合临时演示" | 无明确商用条款，信誉风险高 | 无客户端（纯 ssh -R） |
| **Pinggy** | 否 | 免费隧道 60 分钟/会话（到期重连换 URL）、单连接 | HTTP/TCP/UDP/TLS（SSH over 443 承载） | 中：口碑不错（HN 好评），但 60 分钟硬限制 | 商业模式即按席位订阅；把免费 SSH 端点嵌入商业产品风险中等 | 官方单二进制约 10MB 或直接用系统 ssh |
| **Tailscale Funnel** | 是（SSO 登录） | 免费 Personal 层（6 用户/无限设备）含 Funnel，但仅限非商业用途 | 仅 TLS 类（HTTPS、TLS passthrough、TLS 终结 TCP），无 UDP；端口仅 443/8443/10000；域名固定 *.ts.net；带宽"不可配置的限制"；仍为 beta | 高（Tailscale 基础设施） | 免费层"仅适合非商业使用"；Funnel 需整只 Tailscale 客户端（tailscaled + GUI，100MB 级）并完成登录/ACL 配置 | macOS/Windows 全平台，但属"整个产品"而非可嵌入组件 |

补充关键细节：
- Cloudflare 官方 quick tunnel 文档（2026 版）明确列出：无需账号；"Quick Tunnels are for testing and development"；无运行时间保证；每隧道 200 个并发请求（超出返回 429）；不支持 SSE；每次启动换随机域名；另新增 `--allowed-mail` 邮箱 OTP 保护模式。免费账号 + named tunnel 可绑定用户自有域名（需域名托管在 Cloudflare）获得稳定 URL，任意 TCP/UDP 在两种模式下都要求访客侧运行 `cloudflared access tcp`，普通浏览器访客只能访问 HTTP(S)。
- ngrok ToS（2026-03-02 版）：许可仅限"下载并复制 ngrok Agent"；若产品方持有 ngrok 账号、向使用该产品的第三方分发 agent 是被允许的（Customer Licensee 模式）；若要让终端用户用自己的账号，分发 agent 需 ngrok 事先书面同意。禁止转售服务与竞品用途。MitM 滥用历史（Sophos 报告）使其域名信誉一般。
- Tailscale 免费 Personal 层官方 FAQ："This is a free plan and is only suitable for non-commercial use"。

## 二、MVP 推荐

**架构前提**：把后端抽象为 Provider 接口，MVP 内置 2 个无账号后端，账号型后端作为可选登录项，并预留"自建服务端地址"配置。

**1. Cloudflare quick tunnel（cloudflared）— 首选默认后端（HTTP/Web 场景）**
- 唯一不需要账号、又背靠全球基础设施的高可靠性选项；对核心场景（本地 HTTP/Web 服务暴露）零摩擦。
- Apache-2.0 允许随应用分发二进制；多平台安装包现成。
- 风险控制：官方定位是"testing and development"，UI 上应表述为"临时分享/演示"，不要承诺稳定性；200 并发限制对个人分享场景足够；trycloudflare.com 因滥用被部分网络封锁是已知现象，需作为错误场景处理。
- 升级路径清晰：同一二进制支持 named tunnel，后续可加"登录 Cloudflare 账号绑定自有域名"的高级模式。

**2. bore — 第二默认后端（任意 TCP / 非 HTTP 场景兜底）**
- 无账号、MIT、二进制仅 2.4MB，是所有候选里集成成本最低的；本次实测端到端可用。
- 弥补 cloudflared 的短板：可转发任意 TCP（数据库、SSH、游戏服等），而 Cloudflare 做不到对普通访客的任意 TCP。
- 弱点是 bore.pub 为无 SLA 的个人服务端，UI 需做失败提示与重试；自建升级路径极简单（单二进制 `bore server --min-port`），适合后续提供"自建/团队私有节点"选项。注意其公网 URL 无 TLS 且是随机高端口。

**3. zrok — 推荐的账号型进阶后端（可选集成，二期）**
- 免费层额度最慷慨（5GB/天、50 share、支持保留域名），同时支持 HTTP/TCP/UDP/VPN，是"协议覆盖 + 额度"最优解。
- Apache-2.0 且官方明确"自建无限制"，是长期去供应商化/商用升级的最佳路径（配 OpenZiti 还能做端到端加密）。
- 缺点：需邮箱注册、免费未绑卡有插页页、二进制约 30MB（压缩）。建议做成"登录后获得稳定 URL"的进阶模式而非 MVP 默认。

**不建议作为 MVP 后端**：ngrok（账号强制 + 1GB/月 + 插页页 + agent 分发需书面同意，条款摩擦最大）；Tailscale Funnel（要求装整个客户端 + SSO 登录 + 仅 TLS/三端口 + 免费层禁商用）；localtunnel（维护弱、子域名抢注风险、Node 依赖）；serveo（历史宕机记录差）。localhost.run 和 Pinggy 可作为"SSH 兜底通道"候选，但 Pinggy 的 60 分钟限制使其只适合演示场景。

## 三、主要信息来源

- Cloudflare Quick Tunnels 官方文档：https://developers.cloudflare.com/tunnel/get-started/quick-tunnels/
- cloudflared 仓库（Apache-2.0）与 Release 大小实测：https://github.com/cloudflare/cloudflared
- Cloudflare 任意 TCP 需访客侧 cloudflared access：https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/use-cases/ssh/arbitrary-tcp/
- Cloudflare 应用服务专项条款：https://www.cloudflare.com/service-specific-terms-application-services/
- ngrok 定价页：https://ngrok.com/pricing ；ngrok ToS：https://ngrok.com/tos ；ngrok-go SDK（MIT）：https://github.com/ngrok/ngrok-go
- bore 仓库（MIT）与 Release：https://github.com/ekzhang/bore
- zrok 定价：https://zrok.io/pricing/ ；zrok 仓库：https://github.com/openziti/zrok
- Tailscale Funnel 文档：https://tailscale.com/docs/features/tailscale-funnel ；Tailscale 定价：https://tailscale.com/pricing
- localhost.run 官网：https://localhost.run
- localtunnel 子域名风险 issue：https://github.com/localtunnel/localtunnel（issue #676）
