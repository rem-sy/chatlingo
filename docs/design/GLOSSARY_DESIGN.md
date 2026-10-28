# M5 术语库设计（GLOSSARY_DESIGN）

> 状态：**已实现**（v0.4.0，2026-10-28 发版）。范围与验收同 [03-开发计划.md](../03-开发计划.md) M5 节；
> 本文档为实现级细节与后续术语库维护的参考。

## 1. 目标与非目标

**目标**：用户维护「原文 → 固定译法」术语表，翻译时自动生效，保证专有名词/产品名/
技术词汇在**所有引擎、所有会话**中译法一致。

**非目标**：不做按项目多术语表（Backlog）；不做自动术语挖掘（只管理用户手填+导入）；
不强行改写引擎译文语法。

## 2. 数据模型（store.rs 扩展，history.db 同库）

```sql
CREATE TABLE IF NOT EXISTS glossary (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,        -- 术语原文（如 RAG）
    target TEXT NOT NULL,        -- 固定译法（如 检索增强生成）
    note TEXT DEFAULT '',        -- 备注（可选）
    enabled INTEGER DEFAULT 1,   -- 启停
    created_at INTEGER NOT NULL
);
```

- **匹配规则**：原文子串匹配、大小写不敏感、全角半角归一（仅 ASCII 范围）。
  不做分词、不做词形变化（RAG 和 R.A.G. 视为不同术语，用户自己维护）。
- **冲突检测**：同 source 多条不同 target → 管理窗口标黄提示，翻译时取
  `enabled=1 AND id 最小` 的一条并记日志。
- **导入导出**：JSON（`[{source,target,note,enabled}]`）与 CSV 两格式；
  导入按 source 去重（已存在则更新 target/note）。

## 3. 注入策略（按引擎）

### 3.1 大模型（llm）

`run_translation` 组装 Prompt 前调用 `glossary_match(text) -> Vec<(source, target)>`：

- 命中 ≥1 条 → system prompt 追加一段：
  `术语表（必须严格遵守，原文中的这些词必须按给定译法翻译）：
  - RAG → 检索增强生成
  - ...`
- **截断策略**：按 source 匹配长度降序取前 30 条注入；超出的不注入（记日志）。
- 注入文案放 `engines/mod.rs` 与 `system_prompt` 同处，输入/输出方向 prompt 均适用。

### 3.2 DeepL

官方 Glossary API（free 版支持，`v2/glossaries` 建资源 + 翻译请求带
`glossary_id`）：

- 术语表变更时（增删改/启停）**懒重建**：标记脏位，下次翻译且 glossary 命中时
  先 `POST /v2/glossaries`（source_lang 用检测到的源语言，目标语言为请求目标；
  不匹配语言对的 glossary 不能用 → 只在 source=en/target=zh 等已支持对上生效）。
- **限制**：DeepL glossary 语言对有限；不支持的组合**跳过注入**，走译后校验兜底。
- 旧 glossary 资源删除（`DELETE /v2/glossaries/{id}`）避免堆积（free 版上限 1000 个）。

### 3.3 阿里云

通用版 `TranslateGeneral` 支持 `Context` 参数（官方文档字段，可选填）：

- 命中术语拼接为 Context：`术语对照：RAG=检索增强生成；...`（长度上限按文档，
  超出截断）。
- Context 是软约束，实测效果若不稳 → 叠加译后校验。

### 3.4 译后校验（通用兜底，仅传统引擎）

译文返回后对每条命中术语检查 target 是否出现；未出现 → 面板结果卡片追加
橙色提示行「术语未生效：RAG 应为 检索增强生成」。**不强行替换**（避免破坏语法），
提示文本走既有 `panel-chunk` 后的附加事件或 note 通道。

## 4. 管理窗口（新窗口 glossary）

- 入口：托盘菜单「术语库…」+ 设置页入口按钮。
- UI：搜索框（source/target 模糊）+ 表格（原文/译法/备注/启停开关）+ 底部
  增改删按钮 + 导入/导出。风格与 history 窗口一致（复用其 CSS 骨架）。
- Tauri commands：`glossary_list(query)` / `glossary_upsert(entry)` /
  `glossary_delete(id)` / `glossary_import(json|csv)` / `glossary_export()`。
- 增删改即生效（无脏缓存——Rust 侧 glossary 查询每次读 SQLite，量级小无性能问题；
  DeepL 远端资源走 3.2 懒重建）。

## 5. 设置项

设置页「术语库」区块：总开关（默认开）+ 最大注入条数（默认 30）+ 打开管理窗口按钮。
存 AppConfig：`glossary: { enabled: bool, maxTerms: usize }`（camelCase，
三处同步：Rust struct / api.ts / settings UI）。

## 6. 验收（同 03-开发计划 M5）

- 术语「RAG → 检索增强生成」后，输入「帮我实现一个 RAG 系统」，三引擎译文均含
  「检索增强生成」且无其他译法（DeepL 需 glossary 语言对支持，否则提示行出现）
- 增删术语立即生效，无需重启
- 100+ 术语时注入有截断，翻译无可感知延迟劣化
- 导出 JSON → 清空 → 导入 → 数据一致

## 7. 实现顺序建议

1. store.rs glossary 表 + CRUD commands（可用 example 直测）
2. `glossary_match` + llm prompt 注入（单引擎路径先通）
3. glossary 管理窗口（托盘入口）
4. 阿里云 Context + 译后校验提示
5. DeepL Glossary API（最复杂且语言对受限，最后做；若周期紧可降级为仅译后校验）
6. 设置项 + 版本 v0.4.0 + 构建/实测/归档/提交推送/Release
