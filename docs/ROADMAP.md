# ROADMAP

> 版本状态总表；阶段细节见 [03-开发计划.md](03-开发计划.md)，会话流水见 [HANDOFF.md](HANDOFF.md)。

| 版本 | 内容 | 状态 |
|---|---|---|
| v0.1.x | MVP：统一 Alt+T 面板、OpenAI 兼容流式引擎、自动回填、剪贴板监听对照面板 | ✅ 已发版（tag v0.2.7 起 Release） |
| v0.2.x | DeepL + 双引擎对比、译文缓存/请求合并、翻译历史、开机自启、截图翻译、数据目录/便携模式、语言目录、UI 重铸、真半透明、面板拖动/缩放/分界线、快捷键点击录入、模型列表获取 | ✅ 已发版（tag v0.2.7 + Release，2026-10-26） |
| v0.3.x | 引擎开关式重构（无隐式降级链）、阿里云机器翻译引擎（国内直连）、语种×渠道联动、多引擎结果窗口内堆叠 | ✅ 已发版（tag v0.3.2 + Release，2026-10-27） |
| **v0.4.x (M5)** | **术语库**——SQLite glossary 表 + 管理窗口 + 大模型 Prompt 注入 + DeepL Glossary API / 阿里云 Context 适配 + 译后校验。设计 [design/GLOSSARY_DESIGN.md](design/GLOSSARY_DESIGN.md) / 提示词 [prompts/M5_GLOSSARY_PROMPT.md](prompts/M5_GLOSSARY_PROMPT.md) | ✅ 已实现（v0.4.0，2026-10-28；三引擎 example 实测 + 用户实机验收） |
| v1.0 (M6) | 自动更新（updater）、安装包签名、macOS/Linux、性能（冷启动<1s/内存<80MB）、文档站、仓库转公开 | 未开工 |
| **v0.5.x (M7)** | **移动端（Android 先行）**——Tauri 2 mobile 复用 Rust 核心（三引擎/术语库/历史），TLS 特性化（desktop native-tls / mobile rustls），移动 UI 四页签，系统分享接入，签名 APK 直发。设计 [design/MOBILE_DESIGN.md](design/MOBILE_DESIGN.md) / 提示词 [prompts/M7_MOBILE_PROMPT.md](prompts/M7_MOBILE_PROMPT.md) | 🔄 下一阶段（调研完成 2026-10-28） |
| M8 (iOS) | iOS 端（需 macOS 构建环境 + Apple Developer），Share Extension，TestFlight | 远期（条件具备才启动） |
| 远期 | 按应用记忆方向偏好、双语对话侧栏、CLI、插件化第三方引擎 | Backlog |

## 顺序裁决

- **M7 移动端优先于 M6 打磨**（2026-10-28 用户裁决）：移动场景需求明确，
  Tauri 2 mobile 已稳定且 Rust 核心可直接复用，机会成本低于自动更新/跨平台打磨；
  M6 自动更新等顺延（Android 端自带「下载 APK 覆盖安装」更新路径，移动优先反而互补）。
- M5 术语库优先于 M6 打磨（2026-10-28 用户裁决）：术语一致性是当时翻译质量的最大痛点。
- 划词翻译已从 Backlog 移除（2026-10-25 用户裁决：复制粘贴已覆盖选词动作）。
