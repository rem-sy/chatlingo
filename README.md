# ChatLingo

> AI 对话双语悬浮翻译器 —— 你用中文打字，Agent 看到英文；Agent 用英文回复，你读到中文。

> 为了纯英文使用 ChatGPT 而生。

[![Status](https://img.shields.io/badge/status-v0.1.9%20usable-green)](docs/03-开发计划.md) ![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-lightgrey) ![Stack](https://img.shields.io/badge/stack-Tauri%202%20%7C%20Rust%20%7C%20Vue%203-orange) ![License](https://img.shields.io/badge/license-MIT-blue)

## 这是什么

**ChatLingo 把「翻译提问 → 粘贴 → 翻译回复 → 粘贴」这个循环压缩到一次快捷键。** 它是一个常驻系统托盘的桌面小工具：

- 按 <kbd>Alt</kbd>+<kbd>T</kbd>，光标处呼出统一翻译面板：输入中文（或截图识别文字），选好目标语言回车，流式译文出来后**自动粘贴回你原来的输入框**——ChatGPT 收到的就是地道英文；
- 复制 Agent 的英文回复，屏幕角落自动弹出**半透明中文对照面板**，Markdown 排版和代码块原样保留——你读到的就是地道中文。

从「想到一句话」到「这句话以英文出现在 ChatGPT 输入框」，全程不超过 2 次按键。纯英文对话的质量收益，零表达成本拿到。

## 核心特性

| 特性 | 说明 |
| --- | --- |
| **统一入口** | `Alt+T` 一个面板搞定一切：选方向（15 种语言勾选配置）、文本输入、截图识别，目标语言随意切 |
| **悬浮窗形态** | 无边框置顶小窗：翻译面板跟随光标，阅读面板可拖动、半透明毛玻璃、支持最小化/关闭 |
| **截图翻译** | 框选屏幕任意区域（图片、视频、不可复制界面均可），视觉模型识别文字回填输入框，校对后再翻译 |
| **格式保真** | LLM 翻译保留 Markdown 结构，代码块、行内代码、URL 原样跳过不译 |
| **双引擎 + 自动降级** | 任意 OpenAI 兼容 API（DeepSeek / OpenAI / Ollama / 中转站）为主力；DeepL 为兜底，主引擎失败自动切换并在界面提示 |
| **双引擎对比** | 可选同时用大模型和 DeepL 翻译，面板双栏并列展示择优，各自带复制按钮 |
| **智能跳过 + 缓存** | 已是目标语言不翻、代码主导跳过、短文本忽略；相同内容命中本地缓存秒出、并发请求自动合并 |
| **剪贴板监听** | 复制外文文本自动弹译文面板（可关闭、可调阈值） |
| **翻译历史** | 本地 SQLite 存储，可搜索、一键复制、清空 |
| **本地优先** | 配置与 Key 只存本机 `%APPDATA%`，无遥测、无账号、无云端依赖 |
| **托盘常驻 + 单实例** | 开机自启可选；重复启动自动唤起已有实例 |

## 安装

1. 下载 `ChatLingo_0.1.x_x64-setup.exe`
2. 双击安装（装到用户目录，无需管理员权限），从开始菜单启动「ChatLingo」
3. 托盘图标右键 → **设置**，配置引擎（见下文），点「测试连接」

> 便携版：`chatlingo-0.2.0-portable.exe` 可直接运行，但**必须放在纯英文路径**（见 [已知问题](#已知问题)）。在 exe 同目录创建名为 `chatlingo.portable` 的空文件即进入便携模式，数据存于 exe 旁 `data/` 子目录。
>
> 数据位置（配置/翻译历史）默认在 `%APPDATA%\com.remsy.chatlingo\`，设置页「数据目录」可自定义任意位置并一键迁移。

## 使用说明

### 唯一入口：Alt+T 统一翻译面板

按 <kbd>Alt</kbd>+<kbd>T</kbd> 在光标处呼出悬浮翻译面板，一个入口搞定所有翻译：

- **选方向**：`EN / JA / 中文` 胶囊点击切换目标语言（输出方向选「中文」，输入方向选英/日）
- **文本输入**：打字或粘贴，<kbd>Enter</kbd> 翻译，译文流式显示；目标为外语时可选「粘贴回填」（自动模拟 <kbd>Ctrl</kbd>+<kbd>V</kbd> 贴回原输入框）
- **截图输入**：点「截图」按钮 → 框选屏幕任意区域（图片、视频、不可复制的界面均可）→ 识别出的文字**回填输入框，可先校对再翻译**，方向随意
- <kbd>Esc</kbd> 收起，<kbd>Shift</kbd>+<kbd>Enter</kbd> 换行

另外一条零按键通道：**剪贴板自动监听**（默认开启）——复制任意外文文本（≥最小长度、非代码主导），右下角自动弹出对照面板，无需任何快捷键。

面板说明：标题栏可**拖动**移动位置；`—` 最小化、`×` 关闭；每栏有独立`复制`按钮；「原文」可折叠展开对照。

### 引擎配置

**主力（OpenAI 兼容大模型）**——三样东西，模型支持一键从渠道拉取列表选择：

| 字段 | 示例 |
| --- | --- |
| Base URL | `https://api.deepseek.com/v1`（DeepSeek 官方）· `https://api.openai.com/v1` · `http://localhost:11434/v1`（本地 Ollama） |
| API Key | 对应平台的 Key |
| 模型 | `deepseek-chat` · `gpt-4o-mini` · `qwen2.5:7b` 等（点「获取模型列表」下拉选） |

**兜底（DeepL，可选）**：填入 DeepL API Key 即启用降级（免费版每月 50 万字符，Key 以 `:fx` 结尾，[获取方法](https://www.deepl.com/pro#developer)）。主力失败时自动用 DeepL 翻译，界面显示橙色提示。

**视觉模型（截图 OCR）**：需支持图片输入（如 gemma-4、grok 系列），同样支持从渠道拉取列表选择。

**语言**：设置页从 15 种语言目录里勾选可用项，配置默认目标与输出方向目标。

> 建议：含 Markdown/代码的内容只有大模型能保真翻译，DeepL 兜底主要保证「永远翻得出来」。

### 快捷键

| 快捷键 | 作用 | 可自定义 |
| --- | --- | --- |
| <kbd>Alt</kbd>+<kbd>T</kbd> | 呼出/收起统一翻译面板（文本输入/截图/选方向） | ✓ 设置页 |

### 托盘菜单

呼出翻译面板 · 翻译历史 · 设置 · 退出（退出请走托盘，避免托盘图标残留）

## 已知问题

- **注意：exe 不能放在含中文等非 ASCII 字符的路径下运行**：WebView2 的限制，创建窗口会报 `os error 5` 闪退。安装版无此问题（固定装在英文路径）；本仓库已通过 `src-tauri/.cargo/config.toml` 将构建目录固定到 `C:/chatlingo-target` 规避。
- Windows 托盘偶现幽灵图标：进程被任务管理器强杀后的残影，鼠标悬停即消失，不影响功能。
- 模拟 <kbd>Ctrl</kbd>+<kbd>V</kbd> 回填在个别屏蔽模拟输入的应用（如部分游戏、管理员权限窗口）中无效，此时退化为「仅复制」，手动粘贴即可。

## 开发

```bash
npm install
npm run tauri dev    # 开发调试
npm run tauri build  # 出安装包（C:\chatlingo-target\release\...）
```

**技术栈**：Tauri 2（Rust 核心，业务全部在 Rust 侧）+ Vue 3 + TypeScript + Vite。

**架构**：三窗口（`quickbar` / `panel` / `settings`）+ 事件驱动。Rust 侧模块：`capture`（快捷键/剪贴板监听）→ `detect`（脚本统计语言检测）→ `engines`（`TranslationEngine` trait，OpenAI 兼容流式 + DeepL 兜底）→ `actions`（写剪贴板/模拟粘贴）。前端零业务逻辑。

```
docs/
├── 00-开源参考调研.md   # pot/CopyTranslator/nextai-translator 等同类项目分析与差异化定位
├── 01-设计理念.md       # 方向感知、无感优先、格式保真、本地优先
├── 02-设计方案.md       # 交互形式、翻译流水线、技术架构、竞品对比
└── 03-开发计划.md       # M0-M4 里程碑与任务清单
```

## Roadmap

- [x] M1 输入方向（Quick Bar + 流式翻译 + 自动回填）
- [x] M2 输出方向（剪贴板监听 + 对照面板）
- [x] M3a 引擎生态·上（DeepL 兜底 + 降级链 + 连接测试）
- [x] M3b 引擎生态·下（双引擎并行对比、译文缓存与请求合并）
- [x] M4 打磨·上（翻译历史 SQLite、开机自启、截图翻译）
- [ ] M4 打磨·下（自动更新、macOS/Linux 适配）
- [ ] 远期：按应用记忆方向偏好、双语对话侧栏

## License

MIT © [rem-sy](https://github.com/rem-sy)
