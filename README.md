# ChatLingo

> AI 对话双语悬浮翻译器 —— 让你用中文和任何 AI agent 无障碍对话。

[![Status](https://img.shields.io/badge/status-design-blue)](docs/03-开发计划.md) ![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey) ![Stack](https://img.shields.io/badge/stack-Tauri%202%20%7C%20Rust%20%7C%20Vue%203-orange)

## 为什么做 ChatLingo

和 ChatGPT、Claude、DeepSeek 等 AI agent 对话时，存在一个高频痛点：

- 用中文提问，模型的英文语料训练更充分，**英文提问往往得到更好的回答**；
- 模型用英文回答时，阅读效率又远低于母语。

于是每个双语用户都在手动做同一件事：把提问粘到翻译器译成英文 → 粘回对话框 → 把回答粘出来译成中文。**ChatLingo 把这个循环压缩到一次快捷键。**

## 核心特性（v0.1 已实现 / 规划中）

- 🪟 **悬浮窗形态**：无边框置顶小窗，呼之即来、贴边隐藏，不抢焦点。
- 🔁 **方向感知路由**：`我的输入 → 英文/日文`，`Agent 输出 → 中文`，自动判方向，无需手动切换。
- 🧹 **格式保真**：LLM 翻译保留 Markdown、代码块、表格结构；代码块默认原样跳过。
- 🔌 **可插拔引擎**：任意 OpenAI 兼容 API（DeepSeek / OpenAI / Ollama 本地模型）优先，DeepL / Google 等传统 API 可选备用。
- 📋 **剪贴板模式**：复制 agent 回复 → 自动译成中文并回填剪贴板 + 悬浮预览。
- 🔒 **本地优先**：API Key 存系统钥匙串，翻译历史存本地 SQLite，无遥测、无云端依赖。

## 文档

| 文档 | 内容 |
| --- | --- |
| [开源参考调研](docs/00-开源参考调研.md) | 同类开源项目分析与差异化定位、许可证警示 |
| [设计理念](docs/01-设计理念.md) | 我们解决什么问题，遵循什么原则 |
| [设计方案](docs/02-设计方案.md) | 交互形式、翻译流水线、技术架构、竞品分析 |
| [开发计划](docs/03-开发计划.md) | 里程碑拆解与任务清单 |

## 技术栈

Tauri 2（Rust 核心）+ Vue 3 / TypeScript 前端。选型理由见[设计方案 · 技术选型](docs/02-设计方案.md#技术选型)。

## 当前状态：v0.1.0 可用 ✅

MVP 已实现并通过本机验证（Windows 10/11 x64）：

- **Quick Bar 输入翻译**：`Alt+T` 在光标处呼出悬浮输入条，输入中文回车 → 流式译文 → 自动写入剪贴板并模拟 `Ctrl+V` 回填到原输入框；`EN/JA` 一键切换目标语言，`Esc` 收起。
- **输出方向翻译**：`Alt+Y` 翻译当前剪贴板内容为中文；开启「剪贴板监听」后，复制外文文本自动弹出右下角对照面板（原文可折叠、译文 Markdown/代码块安全渲染）。
- **防误触**：已是目标语言不翻译、代码围栏占比过高跳过、最小长度阈值、自动忽略本程序写入的内容。
- **托盘常驻**：呼出/翻译剪贴板/设置/退出；设置窗可配置引擎、快捷键、自动回填与监听行为。

### 快速开始

1. 安装 `src-tauri/target/release/bundle/nsis/ChatLingo_0.1.0_x64-setup.exe`（或直接运行 `chatlingo.exe`）。
2. 托盘图标 → 设置：填写任意 OpenAI 兼容引擎（默认 DeepSeek：`https://api.deepseek.com/v1` + API Key + `deepseek-chat`），点「测试连接」。
3. 打开你的 AI 对话页面，`Alt+T` 开始用中文和世界对话。

> ⚠️ **已知问题（Windows）**：WebView2 无法在包含中文等非 ASCII 字符的 exe 路径下创建窗口（报 os error 5）。本仓库已通过 `src-tauri/.cargo/config.toml` 把构建目录固定到 `C:/chatlingo-target` 规避；请勿把 exe 移动到中文路径运行。

## 开发

```bash
npm install
npm run tauri dev    # 开发调试
npm run tauri build  # 出安装包
```

技术栈：Tauri 2（Rust 核心，业务全在 Rust 侧）+ Vue 3 + TypeScript + Vite。三窗口架构：`quickbar` / `panel` / `settings`，事件驱动（Tauri event）流式推送。

## License

MIT
