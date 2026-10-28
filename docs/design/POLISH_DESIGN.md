# M6 打磨设计（POLISH_DESIGN）

> 状态：设计完成待开发。范围裁决依据 [ROADMAP.md](../ROADMAP.md)；
> 实现细节以本文档为准。主线：**自动更新（GitHub 渠道）→ 性能 → v1.0 发布工程**。

## 1. 范围裁决（2026-10-28）

| 事项 | 决策 | 理由 |
| --- | --- | --- |
| **自动更新** | ✅ M6a 主线（v0.5.0） | 更新源 = **GitHub Releases**（私有仓期间仅自用，转公开后天然分发渠道）；tauri-plugin-updater + minisign 验签 |
| Windows Authenticode 代码签名 | ⏸ 后置 Backlog | 正式证书几百元/年且需企业身份；updater 自带 minisign 签名已防篡改，SmartScreen 警告自用可接受 |
| 性能（冷启动<1s / 内存<80MB） | ✅ M6b | 实测驱动，懒建窗口 + 编译参数 |
| macOS / Linux 出包 | ⏸ 后置 Backlog | 本机无 macOS（无法出 mac 包）；Linux 无日常使用场景。M6 仅做代码层兼容清理，不出包 |
| 文档站 | ⏸ 可选 | README 已较全；v1.0 前视余力做 GitHub Pages 单页，不做硬性目标 |
| 仓库转公开 + Demo GIF | ✅ M6c（v1.0 前） | 截图已有，补录 Demo GIF |

## 2. 自动更新（M6a）

### 2.1 架构：tauri-plugin-updater + GitHub Releases

```
应用启动（延迟 8s 静默检查）/ 托盘「检查更新…」
   → GET https://github.com/rem-sy/chatlingo/releases/latest/download/latest.json
   → 版本比对（语义化，当前 0.4.0 < 远端则提示）
   → 用户确认 → 下载 setup.exe（同一 Release 资产）
   → minisign 公钥验签（编译进应用的 pubkeys）
   → 静默安装（NSIS /S）→ 重启
```

- **更新源**：每个 Release 挂一份 `latest.json`（`releases/latest/download/` 固定 URL
  永远指向最新版），无需额外服务器，GitHub 渠道一体化。
- **验签**：`tauri signer generate` 生成 minisign 密钥对。公钥进
  `tauri.conf.json > plugins.updater.pubkeys`；**私钥密码离线保管**
  （`D:\hobby\产物\chatlingo\signing\`，不入库不入归档目录明文——写 HANDOFF 指针）。
  构建时环境变量 `TAURI_SIGNING_PRIVATE_KEY` + `..._PASSWORD`。
- **国内网络现实**：GitHub 直连不稳（Clash 时开时关）。检查失败**静默**（仅 debug 日志）；
  托盘入口手动检查失败时弹提示并给「打开 Release 页」兜底按钮。updater 下载走系统
  代理（reqwest 读环境变量；桌面应用一般继承用户代理设置，实测为准）。

### 2.2 latest.json 格式（发版脚本自动生成）

```json
{
  "version": "0.5.0",
  "notes": "…（Release notes 摘要）",
  "pub_date": "2026-10-30T12:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "（tauri signer 对 setup.exe 的 .sig 内容）",
      "url": "https://github.com/rem-sy/chatlingo/releases/download/v0.5.0/ChatLingo_0.5.0_x64-setup.exe"
    }
  }
}
```

### 2.3 前端/交互

- 托盘菜单加「检查更新…」；设置页「通用」区块加「当前版本 + 检查更新」按钮。
- 发现新版本：系统通知（tauri-plugin-notification）或设置页内提示条 →「下载并安装」。
- 下载进度显示（updater 支持 progress 回调）。
- 下载完成 → NSIS 静默安装自动重启（updater Windows 默认行为：installMode passive）。

### 2.4 发版流程改造（脚本化）

现手工流程 → `C:\temp\chatlingo\release_vXXX.py` 模板沉淀为仓库脚本
`scripts/release.py`（Python urllib/curl）：

1. 版本三处 bump 校验一致
2. `npm run tauri build`（带签名环境变量；构建前杀进程）
3. 生成 latest.json（读版本/notes/签名文件）
4. 创建 Release + 上传 setup.exe / chatlingo.exe / latest.json
   （**大文件走 curl**，urllib 408 教训见 HANDOFF 陷阱 19）
5. 归档产物到 `D:\hobby\产物\chatlingo\<版本>\`

## 3. 性能（M6b）

- **测量基准先行**：启动计时（setup 完成到托盘出现，用 stderr 日志时间戳）；
  内存取工作集（任务管理器/PowerShell `Get-Process`）。记录 v0.4.0 基线，逐项优化复测。
- 优化点（按性价比排序）：
  1. **懒建窗口**：启动即建 6 窗口（settings/history/glossary 毛玻璃 WebView 各自吃内存），
     改为首唤起时创建（lib.rs create_windows 拆分 + open_* 命令兜底 get-or-create）
  2. Cargo release profile：`lto = "thin"`、`codegen-units = 1`、`strip = true`
  3. 前端：vite 手动 chunk（shared api/panel 的 marked+dompurify 较大，懒加载）
  4. 剪贴板监听轮询间隔 900ms（可接受，不动）
- 验收：冷启动 < 1s、常驻内存 < 80MB（本机实测为准）。

## 4. 发布工程（M6c → v1.0）

- Demo GIF：录 Alt+T 翻译 + 多引擎对比 + 术语提示 一条龙（可用现有截图自动化脚本思路，
  或用户手工录屏后 ffmpeg 压制）
- README 最终校对；License/MIT 文件确认
- 仓库转公开（私有期间 PAT/密钥清理审计：git 历史检查无密钥泄漏——config.json 从未入库，
  检查 screenshots 无敏感信息：截图含 windhub Base URL，转公开前评估是否打码）
- v1.0 tag + Release

## 5. 里程碑与验收

| 阶段 | 内容 | 版本 | 验收 |
| --- | --- | --- | --- |
| M6a | 自动更新（updater + GitHub 源 + 发版脚本） | v0.5.0 | 模拟旧版本安装后能检测→下载→验签→静默升级成功；检查失败静默/兜底可用 |
| M6b | 性能实测优化 | v0.5.x | 冷启动 <1s、内存 <80MB（本机） |
| M6c | 发布工程 | v1.0 | Demo GIF、转公开审计、v1.0 Release |

## 6. 风险

- updater 在无代理环境下载失败 → 兜底「打开 Release 页」必须同版可用
- 私钥丢失 → 无法再发可更新版本（密钥备份写进 HANDOFF，至少两处离线位置）
- 懒建窗口改动涉及窗口生命周期（托盘唤起/单实例回调），回归面大 → 每窗口单独验证
