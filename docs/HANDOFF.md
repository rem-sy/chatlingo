# HANDOFF（v0.x 周期）

> 格式遵循 [HANDOFF_FORMAT.md](HANDOFF_FORMAT.md)。
> 开工先读：本文档 + [ROADMAP.md](ROADMAP.md) + 任务对应 design 文档与提示词。

**项目一句话**：ChatLingo —— AI 对话双语悬浮翻译器（为了纯英文使用 ChatGPT 而生），
Tauri 2 + Rust + Vue 3，[github.com/rem-sy/chatlingo](https://github.com/rem-sy/chatlingo)（私有），MIT。

## 本机环境

| 项 | 值 |
| --- | --- |
| 仓库路径 | `D:\hobby\源码\Translator`（路径含中文，见下方陷阱 1） |
| 构建目录 | `C:\chatlingo-target`（`src-tauri/.cargo/config.toml` 固定，陷阱 1） |
| 安装位置 | `C:\Users\Rem1sy\AppData\Local\ChatLingo\` |
| 本机数据目录 | `D:\hobby\产物\chatlingo\v0.2.6\data\`（指针文件 `%APPDATA%\com.remsy.chatlingo\data-location`；会话 2 实测指针已从 v0.2.5 迁到 v0.2.6） |
| 产物归档 | `D:\hobby\产物\chatlingo\<版本>\`（setup.exe + chatlingo.exe） |
| 网络代理 | Clash 系统代理 `127.0.0.1:7890`，**时开时关**；git push / GitHub API / DeepL 依赖它；windhub 渠道与阿里云国内直连 |
| git 凭据 | `~/.git-credentials` 明文存有 PAT（username rem-sy）；GitHub API 调用方式见下方命令节 |
| Rust / Node | cargo 1.98（x86_64-pc-windows-msvc）、Node 24 + npm（pnpm 不可用，`beforeBuildCommand` 已用 npm） |
| 平台 | Windows 10/11 x64 单平台；MSVC BuildTools 2022 + VS18 共存（ring 编译失败过 → reqwest 用 native-tls，勿改回 rustls） |

## 常用命令

```bash
# 构建（在仓库根）
npm run tauri build        # 产物 C:\chatlingo-target\release\bundle\nsis\ + chatlingo.exe

# 静默安装 + 启动（pwsh）
Start-Process 'C:\chatlingo-target\release\bundle\nsis\ChatLingo_X.Y.Z_x64-setup.exe' -ArgumentList '/S' -Wait
Start-Process "$env:LOCALAPPDATA\ChatLingo\chatlingo.exe" -RedirectStandardError <log>

# 带日志调试（重要：排查必须重定向 stderr，eprintln 是唯一诊断通道）
# 引擎直测：src-tauri/examples\test_aliyun.rs 模式（cargo run --example），Key 从本机 config.json 读

# GitHub API（Python urllib + TOKEN 从 ~/.git-credentials 解出，参考历史会话命令）
# Release 发布：POST /releases → uploads.github.com 传资产；描述必须 charset=utf-8（PowerShell 发中文会乱码）

# 前端单查
npx vite build            # 只出 dist，快速验前端编译
```

## 当前进度

> **最新会话：会话 2（2026-10-28，M5 术语库 → v0.4.0）**。
> 项目处于 **v0.4.0 已发版**状态（tag + Release 双物料已上传，用户实机验收通过）。
> 下一阶段 **M6 打磨**。
> 会话记录按时间正序排列。

## 会话记录

## 会话 1（2026-10-23 ~ 10-28 · 从立项到 v0.3.2，0 → 可用产品）

- **交付**：
  - 立项与设计（`docs/00-开源参考调研.md`、`01-设计理念.md`、`02-设计方案.md`）
  - v0.1.0 MVP：统一 Alt+T 翻译面板、OpenAI 兼容流式引擎、自动回填、剪贴板监听对照面板
  - v0.2.x：DeepL + 引擎链、双引擎对比、译文缓存/请求合并、翻译历史 SQLite、开机自启、
    截图翻译（视觉模型 OCR）、数据目录自定义 + 便携模式、语言目录勾选配置、UI 配色重铸、
    真半透明毛玻璃、面板拖动/缩放/分界线、快捷键点击录入
  - v0.3.x：**引擎开关式重构**（无隐式降级链，多开即并行对比）、**阿里云机器翻译引擎**
    （国内直连）、**语种×渠道联动**（设置页语言列表=启用渠道并集；运行时跳过不支持
    当前目标语言的引擎）
  - 发版：tag v0.2.7 / v0.3.2 + Release 双物料；README 全面更新
- **关键实现/坑**（换会话必读）：
  1. **WebView2 非 ASCII 路径闪退**：exe 放中文路径报 `os error 5`。构建目录已固定
     `C:\chatlingo-target` 规避；绿色版必须放英文路径
  2. **拖动权限**：`data-tauri-drag-region` 生效需 capabilities 显式
     `core:window:allow-start-dragging`；不要叠加自写 mousedown startDragging（与内建
     脚本双触发导致时灵时不灵）；`maximizable(false)` 防双击最大化白屏
  3. **CSS 加载顺序**：HTML 内联 transparent 会被后加载的 base.css 覆盖 → 窗口样式内
     必须 `html, body { background: transparent !important; }`
  4. **配置字段命名**：serde `rename_all = "camelCase"` 时 `base_url`→`baseUrl` ≠
     `baseURL`；EngineConfig 有 `alias = "baseURL"` 兼容历史。新字段一律 camelCase 且
     前后端（Rust struct / TS interface）同步加
  5. **阿里云 API**：`mt.aliyuncs.com` + `Action=TranslateGeneral` + `Version=2018-10-12`
     + `Scene=general` + `SourceLanguage=auto`（实测支持）；POP HMAC-SHA1 签名；
     **响应是 XML**（已手写 tag 提取）；成功响应含 `<Code>200</Code>` 勿当错误
  6. **语种支持映射双份维护**：Rust `supports_target()`（lib.rs）与前端
     `SUPPORTED_LANGS`（api.ts）必须同步改
  7. **多实例/托盘幽灵**：single-instance 插件已装；用户机器上曾长期跑测试目录旧实例
     混装——诊断前先 `Get-Process chatlingo` 确认唯一实例及其 Path
  8. **剪贴板监听被用户手关**：诊断「复制无反应」先查 config `clipboard.monitor`
  9. **面板位置钳制**：`USER_QUICKBAR_POS` 记忆位置 + show 时 clamp 进屏幕（曾出
     「呼不出」实为沉到屏幕外）
  10. **代理依赖**：push/DeepL 需要 7890 代理在线；不通时本地攒 commit，勿反复重试
  11. **引擎语种**：DeepL 不支持 th/vi/ar/hi；阿里云不支持 id/hi；llm 全支持
- **验证**：三引擎均实测（LLM windhub/glm-4.5-flash、DeepL 免费版、阿里云
  TranslateGeneral 直测通过「重试机制采用带有抖动的指数退避策略。」）；
  UI 经用户多轮实机走查迭代；用户验收 v0.3.2 无遗留缺陷
- **收尾交接**：本次新增 HANDOFF/HANDOFF_FORMAT/ROADMAP/design/GLOSSARY_DESIGN +
  prompts/M5_GLOSSARY_PROMPT；README 与 03-开发计划已同步 v0.3.2

## 会话 2（2026-10-28 · M5 术语库，v0.3.2 → v0.4.0）

- **交付**：
  - `src-tauri/src/store.rs`：glossary 表（与 history.db 同库）+ CRUD/JSON·CSV 导入导出/
    归一化匹配（大小写不敏感 + ASCII 全角→半角）/同 source 冲突取 id 最小——全部以
    Connection 级自由函数实现，`examples/test_glossary.rs` 内存库直测全过（e77d32e）
  - 三引擎注入：LLM system prompt 追加 Glossary 段（`engines/mod.rs::glossary_prompt_block`）、
    阿里云 Context 参数（`glossary_context`，900 字符截断）、DeepL `v2/glossaries`
    懒重建（AppState `deepl_glossary` 缓存 + 脏位 + 内容签名；语言对不支持降级）
    （bbe77fd）
  - 译后校验：deepl/aliyun 译文未含指定译法 → `glossary-warn` 事件，quickbar/panel
    结果卡片追加橙色提示行（不强行替换）
  - glossary 管理窗口（vite input + capabilities + lib.rs create_windows + 托盘「术语库…」+
    设置页入口按钮），风格复用 history 骨架；导入导出走 Blob 下载/文件读取（d07040a）
  - 设置项 `glossary: { enabled, maxTerms }`（Rust/api.ts/UI 三处同步）；版本 v0.4.0 三处
  - examples：`test_glossary_llm.rs` / `test_glossary_aliyun.rs` / `test_glossary_deepl.rs`
    （Key 从本机 config.json / ak.txt 读）
  - 收尾修复：quickbar 单引擎模式译后校验提示行挂错键不显示（409a2c7，用户复测通过）
  - 发版：tag v0.4.0 + GitHub Release 双物料（release-notes-v0.4.0.md，Python urllib）
- **关键实现/坑**：
  12. **DeepL glossary 翻译必须显式 source_lang**：带 `glossary_id` 的 /v2/translate 缺
      source_lang 报 400；TranslateParams 增加 `source_lang`（脚本检测 ZH/JA/KO/默认 EN）
  13. **阿里云 Context 是软约束**：实测「RAG」仍译成「抹布系统」——按设计走译后校验
      提示兜底，不要指望 Context 必然生效
  14. **upsert 按 source 去重**：同 source 永远只有一行（更新而非插入）；「同 source 多行」
      只可能来自旧库异常数据，读取侧仍保留 id 最小去重防御
  15. **译文缓存 key 纳入术语签名**（glossary 命中内容 hash）——否则增删术语后命中旧缓存，
      「增删即生效」验收会挂
  16. **阿里云 Key 不在 config.json**（本机留空）：examples 从 `C:\temp\chatlingo\ak.txt` 读
  17. **TranslateGeneral 对已是目标语的文本原样返回**：测术语注入要用纯外文句子，
      「帮我实现一个 RAG 系统」（中文主体）会被判 zh→zh 直接跳过
- **验证**：`test_glossary`（CRUD/冲突/匹配/截断/归一/往返）全过；`test_glossary_llm`
  PASSED（windhub glm-4.5-flash 含「检索增强生成」）；`test_glossary_deepl` PASSED
  （en→zh glossary 资源创建+删除，译文含「检索增强生成」）；阿里云 Context 软约束符合
  设计预期（兜底提示）；v0.4.0 已静默安装本机、stderr 日志确认 glossary 窗口创建正常；
  UI 待用户实机验收
- **收尾交接**：ROADMAP v0.4.x 置 ✅；03-开发计划 M5 勾选；提示词移入
  `docs/prompts/archive/`；产物归档 `D:\hobby\产物\chatlingo\v0.4.0\`
  （setup.exe + chatlingo.exe）；Release 已发布（v0.4.0 tag + 双物料 + notes）

## 下一步队列

1. **M6 打磨**：自动更新（updater）、安装包签名、macOS/Linux 适配、性能目标、文档站
2. Backlog：按应用记忆方向偏好、双语对话侧栏、CLI、插件化引擎、术语库进阶（多术语表/使用统计）
3. 术语库候选优化：内置示例术语待用户清理后可考虑「首次启动预置默认术语库」选项

## 架构地图

```
窗口（6）：quickbar（Alt+T 统一面板）/ panel（阅读对照）/ settings / history / glossary（术语库）/ snip（截图选区）

Rust（src-tauri/src/）：
  lib.rs      主逻辑：EngineKind{Llm,Deepl,Aliyun}、enabled_engines、supports_target、
              make_stream、run_translation（单引擎流式 / 多引擎并行，结果回调用窗口）、
              剪贴板监听、快捷键、托盘、截图流、窗口管理、全部 #[tauri::command]
  config.rs   AppConfig（camelCase serde）+ 数据目录解析（便携标记 > 指针文件 > %APPDATA%）
  detect.rs   脚本统计语言检测 + 代码主导判断
  engines/    TranslationEngine trait（ChunkStream）
              openai.rs（SSE 流式+重试）/ deepl.rs / aliyun.rs（POP 签名+XML）/ vision.rs（截图 OCR）
  store.rs    SQLite 翻译历史 + 术语库（history.db，与配置同目录）；
              glossary_* 系列自由函数可脱离 AppHandle 直测（examples/test_glossary.rs）

前端（src/）：quickbar/ panel/ settings/ history/ snip/ + shared/api.ts（类型+语言目录+
             SUPPORTED_LANGS）。前端零业务逻辑，全部事件驱动（quickbar-chunk /
             panel-chunk 带 engine 标签 / quickbar-multi-start / panel-start{engines} 等）
```

## 开发约定

- 版本三处同步：`src-tauri/tauri.conf.json` + `package.json` + `src-tauri/Cargo.toml`
- 构建前杀进程（exe 被占用会 build fail）；`npm run tauri build`
- 提交信息 Conventional Commmits（中文可）；产物归档 `D:\hobby\产物\chatlingo\<版本>\`
- 发版：tag + GitHub Release（notes 文件走 Python urllib UTF-8，勿用 PowerShell 发中文）
- 新增窗口：vite.config.ts input + lib.rs create_windows + capabilities windows 数组
- 新增配置字段：Rust struct + api.ts interface + settings UI 三处同步，camelCase
- MIT；参考项目（pot 等）GPL/AGPL 严禁复制代码

## 已知陷阱速查

见会话 1「关键实现/坑」11 条 + 会话 2 新增 12-17 条；新增陷阱随会话记录追加。
