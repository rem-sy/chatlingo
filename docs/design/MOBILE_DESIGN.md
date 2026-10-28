# M7 移动端设计（MOBILE_DESIGN）

> 状态：调研完成，待开发。范围裁决依据 [ROADMAP.md](../ROADMAP.md)；
> 平台细节以本文档为准。目标：把 ChatLingo 的翻译核心带上手机，Android 先行。

## 1. 调研结论（2026-10-28）

### 1.1 技术选型：继续 Tauri 2，不换框架

| 方案 | 结论 | 理由 |
| --- | --- | --- |
| **Tauri 2 mobile（选定）** | ✅ | 2.x 稳定线（2026 年已到 2.10.x）正式支持 Android/iOS；`src-tauri` 的 Rust 核心（engines/store/glossary/config/detect）**原样复用**，前端同一套 Vue+TS；桌面端零改动 |
| Flutter / RN | ❌ | 三引擎、术语匹配、SQLite 全部要用 Dart/TS 重写一遍，双端逻辑永久分叉 |
| Capacitor 套壳纯 Web | ❌ | 翻译逻辑在 Rust，拿不到；跨域/密钥安全也难处理 |
| 原生 Kotlin/Swift | ❌ | 成本最高，双端两套 |

**核心事实**：Tauri 2 移动端 = Rust 核心（`tauri::mobile_entry_point`）+ 系统 WebView
（Android WebView / iOS WKWebView）渲染前端，`invoke`/事件机制与桌面完全一致。
密钥仍只存本机（Android Keystore 可后续加），本地优先原则不变。

### 1.2 桌面特性 × 移动端可行性矩阵

| 桌面特性 | 移动端结论 | 说明 |
| --- | --- | --- |
| 三引擎翻译（LLM/DeepL/阿里云） | ✅ 直接复用 | 纯 reqwest HTTP；注意 TLS 栈（见 2.2） |
| 术语库（SQLite + 注入 + 译后校验） | ✅ 直接复用 | rusqlite bundled 可编译到双移动端 |
| 翻译历史 | ✅ 直接复用 | 同上 |
| 语言检测 | ✅ 直接复用 | 纯算法 |
| 引擎开关/多引擎对比 | ✅ 复用 | UI 重做，逻辑不变 |
| **托盘/全局快捷键/开机自启** | ❌ 桌面独占 | `#[cfg(desktop)]` 隔离，移动端不编译 |
| **悬浮面板（quickbar/panel）** | ⚠️ Android 后期 | 需原生 Kotlin `SYSTEM_ALERT_WINDOW` 悬浮权限，Tauri 无官方支持 → 列入 M7c，MVP 不做 |
| **剪贴板后台监听** | ❌ 平台限制 | Android 10+ 后台读剪贴板仅限输入法；iOS 前台粘贴板提示。**替代方案 = 系统分享（Share）**：其他 App 选中文本 → 分享给 ChatLingo → 翻译 → 结果回剪贴板 |
| **截图翻译（xcap）** | ⚠️ 变形态 | 移动端走系统分享图片/相册选图 → 视觉模型 OCR（复用 vision.rs），M7c |
| 自动更新 | ⚠️ | Android 期 APK 直装手动更新；iOS 走 TestFlight/App Store |

**移动端替代交互核心：Share 进、Clipboard 出。** 桌面是「快捷键呼出 + 自动回填」，
移动是「任意 App 分享文本进来 → 翻译 → 一键复制/回分享」。社区已有
`tauri-plugin-share-intent` 类插件（Android/iOS 双端），必要时自写 Kotlin 桥。

### 1.3 平台先后裁决

| 平台 | 阶段 | 前置条件 |
| --- | --- | --- |
| **Android** | **M7 主线（v0.5.x）** | 本机 Windows 即可开发：Android Studio + SDK + NDK + JDK 17 + rustup target aarch64-linux-android |
| iOS | M8/远期 | **构建必须在 macOS + Xcode**（本机无 Mac）；上架需 Apple Developer $99/年。代码层面 M7 保持 `ios/` 可初始化但不出包 |

## 2. 技术架构

### 2.1 工程形态

```
Translator/
├── src/                  # 前端（现有）
│   ├── mobile/           # 新增：移动端 UI 壳（单窗口多页签：翻译/历史/术语/设置）
│   └── shared/           # 复用 api.ts（类型/语言目录）
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs        # 拆分：桌面专属逻辑 cfg(desktop)；命令层按平台注册
│   │   ├── engines/      # 不动（TLS 特性化，见 2.2）
│   │   ├── store.rs      # 不动
│   │   └── mobile.rs     # 新增：移动端命令（share 接收、无托盘版窗口管理）
│   ├── gen/android/      # tauri android init 生成（入库，签名配置除外）
│   └── tauri.conf.json   # 拆分平台覆盖：tauri.conf.json + tauri.android.conf.json + tauri.desktop.conf.json
└── docs/
```

- **配置文件平台拆分**：桌面 conf 保留 5 窗口 + 托盘；Android conf 只有主窗口，
  `withGlobalTauri`、CSP 按需收紧。
- **命令层分平台注册**：`invoke_handler` 桌面注册全套；移动端只注册
  `translate / glossary_* / history_* / config / share` 系列（托盘/截图/剪贴板监听系列不注册）。

### 2.2 TLS 栈特性化（关键坑位）

- 现状：reqwest + **native-tls**（HANDOFF 陷阱：桌面曾因 ring/MSVC 问题弃用 rustls，勿回切）。
- 移动端：native-tls → Android 需要 openssl 交叉编译（痛苦）；**mobile 走 rustls**
  （ring 对 aarch64-linux-android / aarch64-apple-ios 支持成熟，NDK 下编译顺畅）。
- 落地：`Cargo.toml` 把 engines 的 reqwest 改为
  `default-features = false` + 特性 `desktop-tls = native-tls` / `mobile-tls = rustls`，
  桌面默认 desktop-tls 不变；`#[cfg(mobile)]` 下选 rustls。**桌面行为零改变。**

### 2.3 数据与配置

- 移动端数据目录：`app.path().app_data_dir()`（Android = /data/data/…），config.rs 的
  桌面目录解析（指针文件/便携标记）用 `#[cfg(desktop)]` 隔离，移动端直接 app_data_dir。
- 与桌面端数据互通：走既有的 术语/历史 JSON 导出导入（glossary_import/export 已有），
  不做自动云同步（本地优先原则，云同步列 Backlog）。

### 2.4 移动 UI（新壳，复用逻辑）

单 Activity 单 WebView，底部 Tab 四页，风格延续深色毛玻璃系：

1. **翻译**：语言选择（源自动/目标）、输入框、结果卡（多引擎堆叠，复用卡片样式）、复制/分享按钮
2. **历史**：复用 history 列表逻辑（移动端布局适配）
3. **术语库**：列表 + 增删改 + 启停 + 导入导出（复用 glossary 命令）
4. **设置**：引擎配置（Base URL/Key/模型/开关）、目标语言、数据目录信息

事件流复用：`quickbar-chunk` 流式分片渲染逻辑在移动端等价实现（去 engine 标签细节）。

### 2.5 系统集成（M7b）

- **Share 接收**：Android `ACTION_SEND` text → ChatLingo 出现在分享列表 → 打开翻译页自动填入；
  插件优先（社区 share-intent 插件），不稳则自写 Kotlin 桥（tauri mobile plugin 模板）。
- **结果回写**：翻译完成 → 复制到剪贴板（前台）→ 用户回原 App 粘贴。
- **APK 发布**：GitHub Release 挂 APK（签名 keystore 本地保管，`gen/android` 不入库签名文件）；
  Play Store 上架列 Backlog（$25 一次性）。

## 3. 里程碑拆解

### M7a Android MVP（v0.5.0）
- [ ] 环境搭建：Android Studio + SDK/NDK + JDK17 + rustup android targets；`tauri android init` 跑通真机/模拟器 Hello
- [ ] TLS 特性化改造（2.2），桌面构建回归验证（三引擎直测）
- [ ] lib.rs 桌面/移动拆分（cfg 隔离托盘/快捷键/剪贴板监听/截图/数据目录）
- [ ] tauri.android.conf.json + 移动 capabilities
- [ ] mobile UI 四页签 + 命令注册（翻译流式/历史/术语/设置）
- [ ] 引擎三连直测（example 跑 android target 或真机日志）
- [ ] 签名 APK 出包 → 真机安装实测 → Release 挂 APK（v0.5.0）

### M7b 系统集成（v0.5.x）
- [ ] Share 接收（文本分享进来自动翻译）
- [ ] 翻译结果一键复制/再分享
- [ ] 桌面端术语/历史导出 → 手机导入的互通验证
- [ ] 视觉模型 OCR（分享图片进来识别）——视 M7a 周期可挪 M7c

### M7c 打磨（v0.5.x 后期）
- [ ] Android 悬浮球/悬浮窗（原生 Kotlin，系统悬浮权限）——把桌面 quickbar 的体验带到手机
- [ ] 快捷磁贴（Quick Settings Tile）
- [ ] 图标自适应（adaptive icon，沿用 ∞ 系列）
- [ ] Play Store 上架评估

### M8 iOS（远期，条件：获得 Mac）
- [ ] iOS 初始化 + TestFlight；Share Extension；上架评估

## 4. 风险与决策记录

- **风险 1：NDK/ring 编译折腾**（版本矩阵敏感）→ 严格按 tauri 官方 prerequisites 版本装；
  首次环境搭建预留独立会话。
- **风险 2：WebView2≠Android WebView 行为差异**（流式 SSE 在 Android WebView 正常，但
  CSS 毛玻璃 backdrop-filter Android 支持差）→ mobile UI 降级为实底半透明，不依赖 blur。
- **风险 3：单人双端维护成本** → 桌面仍是主产品，移动 MVP 只做「翻译+术语+历史」三件套，
  悬浮窗等深度集成后置。
- **决策 1**：iOS 不因无法本机构建而阻塞 Android 主线；代码保持可初始化。
- **决策 2**：不做云同步/账号体系（本地优先），数据互通走文件导入导出。
- **决策 3**：移动端不复用桌面窗口体系，UI 新壳但命令/引擎/存储全复用。
