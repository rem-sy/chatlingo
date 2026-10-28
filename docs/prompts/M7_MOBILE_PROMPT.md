# M7 移动端开工提示词（Android MVP → v0.5.0）

> 前置阅读顺序：`docs/HANDOFF.md`（环境/命令/18 条陷阱）→ `docs/HANDOFF_FORMAT.md` →
> `docs/design/MOBILE_DESIGN.md`（**本次任务的实现级设计，以此为准**）→
> `docs/ROADMAP.md` + `docs/03-开发计划.md` M7 节。
> 代码入口：`src-tauri/src/lib.rs`（需 cfg 拆分）、`engines/`（TLS 特性化）、
> `src/shared/api.ts`（复用）、`src/`（新增 mobile UI 壳）。

## 任务范围（按 MOBILE_DESIGN §3 M7a 顺序）

1. **环境搭建**：Android Studio + SDK + NDK + JDK 17 + `rustup target add
   aarch64-linux-android armv7-linux-androideabi`；`npm run tauri android init`；
   Hello 版跑通真机/模拟器（首次环境搭建极易踩 NDK/版本坑，预留完整一个会话也值得）
2. **TLS 特性化**：reqwest desktop=native-tls（不变）/ mobile=rustls；改完先在 Windows
   上回归桌面构建 + `cargo run --example test_glossary_llm` 确认桌面零回归
3. **lib.rs 平台拆分**：托盘/全局快捷键/剪贴板监听/截图/窗口管理/数据目录指针
   全部 `#[cfg(desktop)]`；`invoke_handler` 分平台注册；新增 `mobile.rs`
4. **tauri.android.conf.json** + capabilities（移动端无 core:window:allow-start-dragging）
5. **mobile UI 四页签**：翻译（流式+多引擎卡片）/ 历史 / 术语库 / 设置；
   复用 shared/api.ts 与既有命令，前端新目录 `src/mobile/`，vite input 加 mobile.html
6. **真机实测**：三引擎翻译、术语注入（RAG→检索增强生成 验收句）、历史/术语读写、
   导入导出（与桌面产物互通）
7. **签名出包**：keystore 本地生成保管（不入库），`tauri android build --apk`；
   v0.5.0 三处版本 bump；APK 归档 `D:\hobby\产物\chatlingo\v0.5.0\` + Release 挂 APK

## 硬性约束

- **桌面零回归**：所有移动端改造必须 cfg 隔离，桌面构建产物与行为不变；
  每次大改后 `npm run tauri build` + 静默安装冒烟
- 构建前杀进程（陷阱同桌面）；Android 构建目录仍在 `C:\chatlingo-target`
  （`.cargo/config.toml` 全平台生效，确认 NDK 链接器配置不冲突，必要时 per-target 覆盖）
- 新配置字段 camelCase 三处同步（Rust / api.ts / UI）
- git push 依赖代理（127.0.0.1:7890），不通则本地攒 commit
- 提交信息 Conventional Commits；M7a 期间建议 `feat(mobile):` 前缀

## 验收标准（M7a）

- Android 真机安装 APK：四页签可用，三引擎翻译正常（含流式渲染）
- 术语「RAG → 检索增强生成」注入后，LLM 译文与桌面端一致；
  阿里云/DeepL 译后校验提示行同样生效
- 桌面端导出的术语 JSON → 手机导入 → 翻译行为一致
- Windows 桌面 v0.5.0 构建回归通过，v0.4.x 用户无感升级

## 开工第一件事

读完全部文档后：先查环境（`java -version`、`ANDROID_HOME`、`rustup target list --installed`），
缺什么装什么；环境齐了再 `tauri android init`。**不要在环境未验证前动 lib.rs。**

## 完成后

更新 HANDOFF.md（新会话节）+ ROADMAP/03-开发计划状态 + 本提示词移入
`docs/prompts/archive/`；用户确认后发 v0.5.0 Release（notes 走 Python urllib UTF-8，
大文件上传用 curl——urllib 会 408，见会话 2/3 记录）。
