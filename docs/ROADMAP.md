# ROADMAP

> 版本状态总表；阶段细节见 [03-开发计划.md](03-开发计划.md)，会话流水见 [HANDOFF.md](HANDOFF.md)。

| 版本 | 内容 | 状态 |
|---|---|---|
| v0.1.x | MVP：统一 Alt+T 面板、OpenAI 兼容流式引擎、自动回填、剪贴板监听对照面板 | ✅ 已发版（tag v0.2.7 起 Release） |
| v0.2.x | DeepL + 双引擎对比、译文缓存/请求合并、翻译历史、开机自启、截图翻译、数据目录/便携模式、语言目录、UI 重铸、真半透明、面板拖动/缩放/分界线、快捷键点击录入、模型列表获取 | ✅ 已发版（tag v0.2.7 + Release，2026-10-26） |
| v0.3.x | 引擎开关式重构（无隐式降级链）、阿里云机器翻译引擎（国内直连）、语种×渠道联动、多引擎结果窗口内堆叠 | ✅ 已发版（tag v0.3.2 + Release，2026-10-27） |
| **v0.4.x (M5)** | **术语库**——SQLite glossary 表 + 管理窗口 + 大模型 Prompt 注入 + DeepL Glossary API / 阿里云 Context 适配 + 译后校验。设计 [design/GLOSSARY_DESIGN.md](design/GLOSSARY_DESIGN.md) / 提示词 [prompts/M5_GLOSSARY_PROMPT.md](prompts/M5_GLOSSARY_PROMPT.md) | ✅ 已实现（v0.4.0，2026-10-28；三引擎 example 实测 + 用户实机验收） |
| **v0.5.0 (M6a/b)** | **自动更新 + 性能**——tauri-plugin-updater + GitHub Releases 更新源（latest.json/minisign 验签/nsis passive）、发版脚本化、懒建窗口（冷启动 998ms→830ms）、marked/dompurify 懒加载（常驻 ~390MB→~233MB，<80MB 因 WebView2 架构地板未达）。设计 [design/POLISH_DESIGN.md](design/POLISH_DESIGN.md) | ✅ 已发版（v0.5.0，2026-10-28；0.4.90 替身全链路升级实测通过；私有仓期间 GitHub 渠道 latest.json 404 属预期，转公开后即通） |
| **v1.0 (M6c)** | **发布工程**——git 历史密钥清除、Demo GIF、README 校对、截图脱敏、仓库转公开、GitHub 渠道真实验证 | 🔄 待用户确认（密钥泄漏已审计：test_engine.rs 历史 sk-Key 需重写/作废） |
| 远期 | 按应用记忆方向偏好、双语对话侧栏、CLI、插件化第三方引擎 | Backlog |

## 顺序裁决

- M6 更新渠道定为 **GitHub Releases**（2026-10-28 用户裁决）：latest.json 固定 URL +
  minisign 验签，无需自建服务器；Authenticode/macOS/Linux 出包后置 Backlog。
- 移动端规划（2026-10-28 曾立项 M7）当日撤销：手机端已有可直接使用的应用，不值得
  维护双端；方案文档已删（git 历史 7b70958 可查）。
- M5 术语库优先于 M6 打磨（2026-10-28 用户裁决）：术语一致性是当时翻译质量的最大痛点。
- 划词翻译已从 Backlog 移除（2026-10-25 用户裁决：复制粘贴已覆盖选词动作）。
