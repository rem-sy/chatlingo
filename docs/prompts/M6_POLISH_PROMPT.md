# M6 打磨开工提示词（自动更新 → 性能 → v1.0）

> 前置阅读：`docs/HANDOFF.md`（环境/命令/21 条陷阱）→ `docs/design/POLISH_DESIGN.md`
> （**实现级设计，以此为准**）→ `docs/ROADMAP.md` M6 行。
> 代码入口：`src-tauri/src/lib.rs`（托盘/窗口创建）、`src-tauri/tauri.conf.json`、
> `src/settings/App.vue`（设置区块）。

## 任务范围（按 POLISH_DESIGN §5 顺序）

### M6a 自动更新（v0.5.0）
1. `tauri signer generate` 生成密钥对；私钥+密码离线保管两处
   （`D:\hobby\产物\chatlingo\signing\` + 用户自备位置），HANDOFF 记指针不记内容
2. 集成 `tauri-plugin-updater` + `tauri-plugin-notification` + `tauri-plugin-process`
   （重启用）：endpoint = `https://github.com/rem-sy/chatlingo/releases/latest/download/latest.json`，
   pubkeys 进 tauri.conf；windows installMode 用 nsis passive
3. 托盘「检查更新…」+ 设置页版本区块（当前版本/检查/进度/失败兜底「打开 Release 页」）
4. 启动延迟 8s 静默检查（失败只 eprintln）
5. 发版脚本 `scripts/release.py`（POLISH_DESIGN §2.4 五步流程；
   大文件上传一律 curl，urllib 会 408——陷阱 19）
6. 发 v0.5.0；用 v0.4.0 安装版实测「检测→下载→验签→静默安装→重启→版本变 0.5.0」全链路

### M6b 性能（v0.5.x）
1. 基线测量：v0.4.0 冷启动时间（stderr 时间戳）+ 常驻内存，记录进 HANDOFF
2. 懒建窗口：settings/history/glossary 改首唤起创建（托盘/单实例回调/入口按钮全部
   get-or-create 化）；quickbar/panel 保持启动即建（核心交互窗口）
3. Cargo release profile（lto/codegen-units/strip）+ vite 手动 chunk
4. 复测达标（<1s / <80MB）→ v0.5.x 发版

### M6c 发布工程（v1.0）
Demo GIF、README 校对、密钥泄漏审计（git 历史 + 截图脱敏评估）、仓库转公开、v1.0 tag

## 硬性约束

- 桌面单平台（Windows）；macOS/Linux 只做 cfg 兼容清理不出包
- 构建前杀进程；带签名构建需设 TAURI_SIGNING_PRIVATE_KEY(+PASSWORD) 环境变量
- 懒建窗口改动回归面大：托盘入口/单实例二次启动/关闭隐藏逻辑逐窗口验证
- 版本三处同步；提交 Conventional Commits（`feat(updater):`/`perf:` 前缀）
- git push 依赖代理；Release 大资产用 curl

## 验收标准

- M6a：v0.4.0 → v0.5.0 自动升级全链路成功；断网/无代理时检查静默、手动入口有兜底
- M6b：冷启动 <1s、常驻 <80MB（本机实测，基线数据留档）
- M6c：v1.0 Release + 仓库公开无泄漏

## 开工第一件事

读文档后：先 `cargo tauri signer generate -w D:\hobby\产物\chatlingo\signing\chatlingo.key`
（密码自设并记录保管位置），公钥指纹记入 HANDOFF；再动 updater 集成。
私钥一旦丢失该更新渠道永久作废——先备份，后开发。

## 完成后

HANDOFF 新会话节 + ROADMAP/03-开发计划状态 + 本提示词移 `docs/prompts/archive/`；
用户确认后发版（脚本化流程跑通即 M6a 验收）。
