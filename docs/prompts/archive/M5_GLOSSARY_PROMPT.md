# M5 术语库 · 开工提示词

> 新会话开工时把本文件全文粘贴给开发 agent。

---

你是 ChatLingo 项目的开发 agent。本次任务：**实现 M5 术语库功能，发版 v0.4.0**。

## 第一步：读文档（按序）

1. `docs/HANDOFF.md` —— 本机环境、命令、11 条已知陷阱（必读「关键实现/坑」）
2. `docs/HANDOFF_FORMAT.md` —— 会话结束前按此规范更新 HANDOFF
3. `docs/design/GLOSSARY_DESIGN.md` —— 本次任务的实现级设计（以此为准）
4. `docs/ROADMAP.md` + `docs/03-开发计划.md` M5 节 —— 范围与验收
5. 代码入口：`src-tauri/src/lib.rs`（EngineKind / run_translation / commands）、
   `src-tauri/src/store.rs`（SQLite 模式参考）、`src/history/`（管理窗口 UI 参考）

## 任务范围（按 GLOSSARY_DESIGN §7 顺序）

1. glossary 表 + CRUD Tauri commands
2. `glossary_match()` + 大模型 Prompt 注入
3. 术语库管理窗口（托盘「术语库…」入口，风格对齐 history 窗口）
4. 阿里云 Context 注入 + 译后校验提示
5. DeepL Glossary API（语言对受限，最后做，可降级为仅译后校验）
6. 设置页术语库区块（总开关/最大注入条数/入口按钮）
7. 版本 bump v0.4.0（三处：tauri.conf.json / package.json / Cargo.toml）

## 硬性约束

- 构建前先杀进程：`Get-Process chatlingo | Stop-Process -Force`（exe 占用会 build fail）
- 构建目录 `C:\chatlingo-target`（勿动）；安装本机用 setup.exe `/S` 静默
- **验证方式**：新引擎逻辑用 `src-tauri/examples/` 下 example 直测（参考
  test_aliyun.rs 模式，Key 从本机 config.json 读）；UI 让用户实机验收
- 排查必须带 stderr 重定向启动，eprintln 是唯一诊断通道
- 新配置字段 camelCase，Rust struct / api.ts / settings UI 三处同步
- git push 可能因代理（127.0.0.1:7890）不通失败：本地攒 commit，问用户代理状态
- 提交信息 Conventional Commmits；产物归档 `D:\hobby\产物\chatlingo\v0.4.0\`
- 完成后：更新 HANDOFF.md（新会话节）+ ROADMAP.md 状态 + 本提示词若有余量移入
  `docs/prompts/archive/`；用户确认后发 Release（notes 用 Python urllib UTF-8）

## 验收标准（GLOSSARY_DESIGN §6）

- 「RAG → 检索增强生成」注入后三引擎译法一致（DeepL 语言对不支持时出提示行）
- 术语增删即生效；100+ 术语有截断策略；导出→清空→导入数据一致

## 开工第一件事

读完上述文档后，从 GLOSSARY_DESIGN §7 第 1 步开始：store.rs 加 glossary 表与
CRUD，写 example 直测通过后继续第 2 步。开工前先 `git log --oneline -5` 确认
工作区干净、`Get-Process chatlingo` 确认无残留实例。
