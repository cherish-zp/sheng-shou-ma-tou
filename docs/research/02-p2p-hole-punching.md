# 调研报告 2：P2P 打洞（NAT traversal）技术路线可行性评估（2025-2026）

## 一、可行性结论

### 核心判断：取决于"访问者是谁"

**打洞在协议层面要求通信双方都参与发包**（各自在自家 NAT 上留出映射）。这决定了产品形态的分野：

**场景 A：把本地 3000 端口暴露给"任意公网访问者"（访问者只是打开浏览器/用 curl 的陌生人）——纯 P2P 不成立。**
- 任意访问者不会、也没法配合发打洞包，ICE/UDP 打洞无从发起。这个场景下"P2P"只有两条窄路：
  1. **Natter/natmap 式"全锥 NAT 端口暴露"**：用 STUN 探测出 NAT 公网映射 IP:Port 并 keepalive 保活，任何人直接访问该映射地址（Natter 的 README 原话："Expose your TCP/UDP port behind full-cone NAT to the Internet"，访问者"无需安装任何客户端"）。**但它仅在全锥形 NAT（NAT1）下成立**——移动宽带和手机蜂窝网常见的对称/端口递增型 CGNAT 直接出局，且暴露的是随机端口、无域名、依赖映射长期稳定，只能算"锦上添花的 bonus"。
  2. **浏览器 WebRTC 访问者**：访问者的浏览器可以参与 ICE 打洞，理论上可实现"陌生人 P2P"，但前提是先通过中继把引导页送给访问者（先中继后升级），工程复杂度陡增，不建议作为首期路线。
- 结论：**"任意访问者"场景下，中继是必需品而非兜底**。产品实质是 ngrok/bore 类"中继型穿透"，P2P 至多是加速通道。

**场景 B：访问者安装了客户端（两台装了客户端的机器互访）——"P2P 优先 + 失败自动回退中继"完全现实，且是业界已验证的成熟范式。**
- frp xtcp（打洞失败回退 stcp 中转）、Tailscale（先 DERP 后升级直连）、iroh（QUIC 打洞 + relay 兜底）、libp2p（DCUtR + Circuit Relay v2）全是同一范式。
- 实测直连成功率：UDP 打洞典型 **80-85%**（家用 NAT 上 STUN 打洞可达 93.67%，TUM 研究）；TCP 同时打开打洞约 **64-82%**；libp2p DCUtR 大规模实测 **70% ± 7.1%**（ProbeLab）。即约 15-30% 的连接需走中继。

**场景 C（国内特殊性）**：国内运营商 CGNAT 普遍——移动宽带几乎默认 CGNAT 且基本要不到公网 IPv4，移动/长城宽带常见对称型 NAT（社区经验值：NAT4 对 NAT4 的 P2P 成功率约 20%）；电信/联通家宽相对友好。好消息是很多国内 CGNAT 属于"端口递增型"而非严格随机对称型，社区已有 n4（端口预测 + 区间扫描 PoC）和 frp 0.49+ 双 NAT4 打洞成功案例。另外**国内 IPv6 普及率高，双栈（IPv6 直连优先、IPv4 打洞兜底）能显著拉高直连率**。

### 产品建议
- 主打"给别人看本地 demo / 对外提供服务" → **中继为主**，P2P 只对装了客户端的熟客生效。
- 主打"两台自己的机器（NAS/办公机/开发机）之间映射端口" → **P2P 优先 + 自动回退中继**，直连率预期 70-85%。
- 最优工程形态：把两者做成同一条管道的两种模式（iroh 的设计），访问者不装客户端时自动落在中继 URL 上。

## 二、开源积木调研结果

### 1. frp xtcp（参考范式）
- 原理：frps 只做"帮忙交换双方公网地址"的信令，打洞成功后流量不走 frps。官方文档明确"可用性和稳定性无法保证"。
- 已支持 `keepTunnelOpen`（保持隧道常开、定期重打洞）、多策略重试；**自带 fallback**：`fallbackTo`（指向 stcp-visitor）+ `fallbackTimeoutMs`。
- 局限：xtcp 的 visitor 端必须跑 frpc → 属于场景 B，对任意访问者无效。

### 2. libp2p DCUtR（Rust 实现成熟，但偏重）
- 标准组合是 **Circuit Relay v2（中继兜底）+ AutoNAT v2（可达性探测）+ DCUtR（打洞升级直连）**。
- 成熟度"够生产"，但 ~70% 成功率意味着 relay 必不可少；整个 libp2p 栈 API 复杂、依赖面大。

### 3. iroh（Rust 原生，最契合 Tauri 技术栈，推荐）
- n0 公司出品：QUIC 打洞 + relay 兜底一体化，核心库生产环境验证，正走在 1.0 路线上。
- relay 节点同时承担信令（互换地址）+ 兜底转发 + 打洞辅助；n0 提供免费公共 relay 起步。
- "ticket"（节点 ID + 直连地址 + relay 地址的编码串）天然替代自建信令：两台机器交换 ticket 即可打洞。
- 端口映射只需桥接 `本地 TCP listener <-> iroh stream`（社区有 iroh-ssh 等同类模式）。

### 4. 其他项目速览
- **rathole**（Rust，MIT）：无 P2P 打洞模式，纯中转反向代理——但是最轻的自建中继兜底选项之一。
- **Natter / natmap**：全锥 NAT 端口暴露（STUN + keepalive），无访问端客户端，可作为"NAT1 用户的 bonus 直连模式"借鉴。
- **netbird**：ICE/STUN 打洞 + Coturn TURN + 自研 WebSocket Relay 三层兜底；打洞失败走 relay 时速率明显下降（社区实测约 7Mbps 量级）。
- **Tailscale**：连接先走 DERP 保证秒通，后台持续尝试升级直连；提示我们**中继节点要在国内布局**。
- **WebRTC datachannel 桥接 TCP**：仅当需要"浏览器当访问者"时才值得引入。

### 5. 信令服务器（最轻方案）——完全可行
- **Cloudflare Workers + Durable Objects**：一个 DO = 一个房间，原生 WebSocket + Hibernation，SQLite DO 已进免费层，正是 WebRTC/P2P 信令的标准模式，免费额度对信令绰绰有余。
- **Deno Deploy**：原生支持 WebSocket，免费层 100 万请求/月 + 100GB 出站。
- **Vercel 不可用**：Serverless Functions 无法持有长连接。
- 若选 iroh，甚至只需分发 ticket，信令服务器可选。

### 6. 兜底中继——免费公共中继只能用于起步，不能用于生产
- **bore**（Rust，约 400 行，MIT）：自建中继成本极低；但公共 bore.pub 仅 TCP、随机端口、有隧道时长/防滥用限制，作者明示生产请自建。
- **zrok SaaS 免费层**：25 环境/50 后端/约 10GB 流量每天，注册即用，适合内测。
- 结论：**fallback 中继必须自建**（一台国内 VPS 跑 rathole/frp/iroh-relay 均可）。

## 三、推荐积木组合（Rust/Tauri）

| 层 | 首选 | 备选 |
|---|---|---|
| 打洞 + 连接 | **iroh**（QUIC 打洞 + relay + ticket，纯 Rust，生产验证） | libp2p（DCUtR + Circuit Relay v2 + AutoNAT） |
| 端口映射桥 | 本地 TCP listener ↔ iroh stream 自写桥接 | 直接嵌入 frp/rathole 二进制做进程管理 |
| 信令 | ticket 交换（QR/短码）+ 可选 CF Workers DO 上的极简 WS | Deno Deploy WS；frp 式"经 relay 节点互换地址" |
| 兜底中继 | 自建 **iroh-relay**（或 rathole/frp）部署在国内 VPS | 起步期蹭 n0 公共 relay / zrok 免费层 |
| 加速通道（bonus） | Natter 式全锥 NAT 端口暴露（NAT1 用户免客户端直连） | IPv6 直连优先（国内性价比极高） |
| 浏览器访问者（远期） | 中继引导页 + WebRTC datachannel | — |

## 四、实现复杂度评估

- **方案一（推荐）：iroh 集成**。TCP 桥接 1-2 周；信令（ticket 分发 + 极简 WS）0.5-1 周；自建 relay 部署 2-3 天；Tauri UI/配置/升级等桌面工程 4-8 周。**MVP 合计约 2-3 人月**，打磨到产品级 4-6 人月。
- **方案二：libp2p 栈自建**：多 1-2 人月，合计 3.5-5 人月。
- **方案三：完全自研打洞**：6-12 人月，不建议。
- **方案四（最快验证市场）**：UI 包一层 frp 或 rathole，2-4 周出 demo，验证需求后再换内核。

## 五、风险清单

1. **NAT 环境恶化风险**：移动/长城等对称型 CGNAT 用户占比不低，直连率在国内可能低于 70%。
2. **运营商 UDP QoS**：国内家宽存在 UDP 限速案例，TCP fallback 要做。
3. **中继带宽成本**：免费公共中继不可用于生产；自建中继意味着带宽费用回到自己身上。
4. **"任意访问者"预期错配**：产品文案必须区分"装客户端的访问者（可 P2P）"与"任意访客（走中继）"。
5. **安全与滥用**：必须有端到端加密 + 访问鉴权（iroh 自带 TLS）；工具可能被滥用，服务端需风控。
6. **合规**：国内中继节点需 ICP 备案；跨境链路质量与政策风险。
7. **连接稳定性**：NAT 映射会超时，需 keepalive 与断线重打洞；移动宽带 IP 归属地漂移会打断长连接。
8. **serverless 供应商锁定**：信令要设计成可一键迁到自建单 binary（几行 Rust axum WS 即可）。
9. **依赖风险**：iroh 仍处 1.0 前（API 会变）。

## 六、主要信息来源

- 打洞成功率：TUM（https://www.net.in.tum.de）、Aalto（https://aaltodoc.aalto.fi）、ProbeLab（https://probelab.io）、ACM NAT 特征研究（https://dl.acm.org）
- 国内 NAT 环境：知乎 NAS 远程访问指南、V2EX 讨论、lyc8503 博客（https://blog.lyc8503.net）、n4 端口递增型打洞 PoC（https://github.com/MikeWang000000/n4）、bulianglin NAT 类型打洞实测（https://bulianglin.com/archives/p2p.html）
- frp xtcp：官方文档（https://gofrp.org/zh-cn/docs/features/xtcp/）
- libp2p：docs.rs/libp2p、rust-libp2p 文档（https://libp2p.github.io）
- iroh：GitHub（https://github.com/n0-computer/iroh）、iroh-relay（https://lib.rs）、官网（https://www.iroh.computer）
- netbird/tailscale：https://docs.netbird.io/about-netbird/understanding-nat-and-connectivity 、Tailscale How NAT traversal works（https://tailscale.com/blog/how-nat-traversal-works）
- 端口暴露类：Natter（https://github.com/MikeWang000000/Natter）、natmap（https://github.com/heiher/natmap）、rathole（https://github.com/rapiz1/rathole）
- WebRTC 桥接：pion webrtc（https://github.com/pion/webrtc）、str0m（https://docs.rs）
- 信令 serverless：konsumer/signal-worker（https://github.com/konsumer/signal-worker）、Deno Deploy 定价（https://deno.com）
- 兜底中继：bore（https://github.com/ekzhang/bore）、zrok 定价（https://zrok.io）

**一句话总结**：纯 P2P 打洞在"两个装了客户端的节点"场景下可行且成熟（70-85% 直连率 + 业界标配的中继兜底），在"暴露端口给任意公网访问者"场景下不成立（访问者无法参与打洞，中继是刚需）；对 Rust/Tauri 技术栈，最划算的组合是 **iroh（打洞+兜底一体化）+ 极简 WS 信令（CF Workers DO 可免费用）+ 自建国内中继**，P2P 部分约 2-3 人月。
