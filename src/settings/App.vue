<template>
  <div class="settings">
    <h1>ChatLingo 设置</h1>

    <section>
      <h2>翻译引擎（OpenAI 兼容）</h2>
      <div class="form-row">
        <label>启用</label>
        <input type="checkbox" v-model="cfg.engine.enabled" />
        <span class="hint">关闭后不参与翻译；开启多个引擎时将同时翻译并在面板对比</span>
      </div>
      <div class="form-row">
        <label>Base URL</label>
        <input v-model="cfg.engine.baseUrl" type="text" placeholder="https://api.deepseek.com/v1" />
      </div>
      <div class="form-row">
        <label>API Key</label>
        <input v-model="cfg.engine.apiKey" type="password" placeholder="sk-..." />
      </div>
      <div class="form-row">
        <label>模型</label>
        <input v-model="cfg.engine.model" type="text" list="model-list" placeholder="deepseek-chat 或点击右侧获取" />
        <button :disabled="loadingModels" @click="fetchModels">{{ loadingModels ? '获取中…' : '获取模型列表' }}</button>
      </div>
      <div class="form-row" v-if="modelMsg">
        <label></label>
        <span class="test-result" :class="models.length ? 'ok' : 'bad'">{{ modelMsg }}</span>
      </div>
      <datalist id="model-list">
        <option v-for="m in models" :key="m" :value="m" />
      </datalist>
      <div class="form-row">
        <label></label>
        <button :disabled="testing" @click="test">{{ testing ? '测试中…' : '测试连接' }}</button>
        <span class="test-result" :class="testOk === true ? 'ok' : testOk === false ? 'bad' : ''">{{
          testMsg
        }}</span>
      </div>
    </section>

    <section>
      <h2>DeepL</h2>
      <div class="form-row">
        <label>启用</label>
        <input type="checkbox" v-model="cfg.deepl.enabled" />
        <span class="hint">每月免费 50 万字符，Key 以 :fx 结尾（[获取](https://www.deepl.com/pro#developer)）；国内需代理</span>
      </div>
      <div class="form-row">
        <label>DeepL API Key</label>
        <input v-model="cfg.deepl.apiKey" type="password" placeholder="留空则不启用" />
      </div>
    </section>

    <section>
      <h2>阿里云机器翻译</h2>
      <div class="form-row">
        <label>启用</label>
        <input type="checkbox" v-model="cfg.aliyun.enabled" />
        <span class="hint">国内直连；个人每月 100 万字符免费</span>
      </div>
      <div class="form-row">
        <label>SecretId</label>
        <input v-model="cfg.aliyun.accessKeyId" type="password" placeholder="访问管理 cam/capi 里创建" />
      </div>
      <div class="form-row">
        <label>SecretKey</label>
        <input v-model="cfg.aliyun.accessKeySecret" type="password" placeholder="访问管理里创建的密钥" />
      </div>
    </section>

    <section>
      <h2>多引擎</h2>
      <div class="form-row">
        <label></label>
        <span class="hint">启用多个引擎时：所有启用的引擎会同时翻译，阅读面板并列展示结果对比，择优复制</span>
      </div>
    </section>

    <section>
      <h2>语言</h2>
      <div class="form-row">
        <label>可用语言</label>
        <div class="lang-grid">
          <label v-for="code in unionLangs" :key="code" class="lang-check">
            <input
              type="checkbox"
              :value="code"
              v-model="selectedLangs"
            />
            {{ langDef(code).label }} {{ langDef(code).name }}
          </label>
        </div>
      </div>
      <div class="form-row">
        <label>默认目标</label>
        <select v-model="cfg.quickbar.defaultTarget">
          <option v-for="c in unionSelected" :key="c" :value="c">{{ langDef(c).label }} {{ langDef(c).name }}</option>
        </select>
        <span class="hint">Alt+T 面板初始选中的目标语言</span>
      </div>
      <div class="form-row">
        <label>输出方向目标</label>
        <select v-model="cfg.outputTarget">
          <option v-for="code in unionLangs" :key="code" :value="code">{{ langDef(code).label }} {{ langDef(code).name }}</option>
        </select>
        <span class="hint">剪贴板监听/输出方向的翻译目标</span>
      </div>
    </section>

    <section>
      <h2>翻译面板</h2>
      <div class="form-row">
        <label>呼出快捷键</label>
        <input :value="recording ? '请按下组合键…' : cfg.quickbar.hotkey" type="text" readonly :class="{ recording }" @focus="$event.target.blur()" />
        <button @click="toggleRecord">{{ recording ? '取消' : '点击录入' }}</button>
        <span class="hint">{{ hotkeyErr || '点击「点击录入」后直接按组合键（如 Alt+T）' }}</span>
      </div>
      <div class="form-row">
        <label>译文自动回填</label>
        <input type="checkbox" v-model="cfg.quickbar.autoPaste" />
        <span class="hint">翻译完成后自动写入剪贴板并模拟 Ctrl+V 粘贴回原输入框（仅外语目标时）</span>
      </div>
    </section>

    <section>
      <h2>剪贴板与截图</h2>
      <div class="form-row">
        <label>剪贴板自动监听</label>
        <input type="checkbox" v-model="cfg.clipboard.monitor" />
        <span class="hint">复制外文文本时自动弹出译文面板</span>
      </div>
      <div class="form-row">
        <label>最小触发长度</label>
        <input v-model.number="cfg.clipboard.minLength" type="text" />
      </div>
      <div class="form-row">
        <label>视觉模型（OCR）</label>
        <input v-model="cfg.clipboard.visionModel" type="text" list="model-list" placeholder="留空则用主力模型；需支持图片输入" />
        <button :disabled="loadingModels" @click="fetchModels">{{ loadingModels ? '获取中…' : '获取' }}</button>
      </div>
    </section>

    <section>
      <h2>术语库</h2>
      <div class="form-row">
        <label>启用术语库</label>
        <input type="checkbox" v-model="cfg.glossary.enabled" />
        <span class="hint">翻译时自动把命中的术语按固定译法注入所有引擎，保证专有名词译法一致</span>
      </div>
      <div class="form-row">
        <label>最大注入条数</label>
        <input v-model.number="cfg.glossary.maxTerms" type="text" />
        <span class="hint">命中过多时按术语长度优先截断（默认 30）</span>
      </div>
      <div class="form-row">
        <label></label>
        <button @click="openGlossary">打开术语库管理…</button>
        <span class="hint">托盘菜单「术语库…」也可打开</span>
      </div>
    </section>

    <section>
      <h2>通用</h2>
      <div class="form-row">
        <label>开机自启</label>
        <input type="checkbox" v-model="autostart" @change="toggleAutostart" />
        <span class="hint">{{ autostartErr || '开机后自动驻留托盘' }}</span>
      </div>
      <div class="form-row">
        <label>数据目录</label>
        <input v-model="dataDir" type="text" placeholder="留空 = 跟随用户配置文件 (%APPDATA%)" :disabled="dataInfo?.portable" />
        <button :disabled="dataInfo?.portable" @click="browseDir">浏览…</button>
        <button :disabled="dataInfo?.portable || applyingDir" @click="applyDataDir()">{{ applyingDir ? '迁移中…' : '应用' }}</button>
        <button v-if="dataInfo?.custom" @click="applyDataDir(true)">恢复默认</button>
      </div>
      <div class="form-row">
        <label></label>
        <span class="hint">
          当前：{{ dataInfo?.dir }}（{{ dataInfo?.portable ? '便携模式' : dataInfo?.custom ? '自定义' : '跟随用户配置文件' }}）。
          浏览选择目录后点「应用」或直接「保存」即自动迁移。便携模式：在 exe 同目录创建名为 chatlingo.portable 的空文件即可，数据将存于 exe 旁 data/ 子目录
        </span>
      </div>
      <div class="form-row" v-if="dataDirMsg">
        <label></label>
        <span class="test-result" :class="dataDirOk ? 'ok' : 'bad'">{{ dataDirMsg }}</span>
      </div>
      <div class="form-row">
        <label>当前版本</label>
        <span>v{{ appVersion }}</span>
        <button :disabled="updateBusy" @click="manualCheckUpdate">{{ updateBusy ? updateStatus : '检查更新' }}</button>
        <span class="hint">更新源：GitHub Releases</span>
      </div>
      <div class="form-row" v-if="updateStatus">
        <label></label>
        <span class="test-result" :class="updateOk ? 'ok' : 'bad'">{{ updateStatus }}</span>
      </div>
      <div class="form-row" v-if="pendingUpdate">
        <label></label>
        <button class="primary" :disabled="updateBusy" @click="installUpdate">{{ updateBusy ? updateStatus : `下载并安装 v${pendingUpdate.version}` }}</button>
        <span class="hint">{{ (pendingUpdate.notes || '').slice(0, 120) }}</span>
      </div>
      <div class="form-row" v-if="updateFailed">
        <label></label>
        <button @click="openReleasePage">打开 Release 页</button>
        <span class="hint">检查或下载失败（网络/代理）时可手动下载安装</span>
      </div>
    </section>

    <footer>
      <span class="saved" v-if="saved">已保存 ✓</span>
      <span class="err" v-if="saveErr">{{ saveErr }}</span>
      <button class="primary" @click="save">保存</button>
    </footer>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, markRaw } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke, type AppConfig, LANG_CATALOG, langDef, SUPPORTED_LANGS } from '../shared/api'
import { check as updaterCheck, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { getVersion } from '@tauri-apps/api/app'
import '../shared/base.css'
import './settings.css'

const cfg = ref<AppConfig>(defaultCfg())
const selectedLangs = ref<string[]>(['en', 'ja'])
const saved = ref(false)
const saveErr = ref('')
const testing = ref(false)
const autostart = ref(false)
const autostartErr = ref('')
const testOk = ref<boolean | null>(null)
const testMsg = ref('')
const models = ref<string[]>([])
const loadingModels = ref(false)
const modelMsg = ref('')

async function fetchModels() {
  loadingModels.value = true
  modelMsg.value = ''
  cfg.value.quickbar.targetLangs = unionSelected.value
  try {
    await invoke('save_config', { config: cfg.value })
    const res = await invoke<{ models: string[] }>('list_models')
    models.value = res.models
    modelMsg.value = res.models.length
      ? `获取到 ${res.models.length} 个模型，点击模型输入框选择`
      : '渠道未返回任何模型'
  } catch (e) {
    models.value = []
    modelMsg.value = `获取失败：${e}`
  } finally {
    loadingModels.value = false
  }
}

// 目录顺序展示勾选结果
const unionSelected = computed(() =>
  orderedLangs.value.filter((c) => unionLangs.value.includes(c))
)
const orderedLangs = computed(() =>
  LANG_CATALOG.map((l) => l.code).filter((c) => selectedLangs.value.includes(c))
)

// 启用渠道支持语言的并集：语言网格只展示这些
const unionLangs = computed(() => {
  const set = new Set<string>()
  if (cfg.value.engine.enabled) SUPPORTED_LANGS.llm.forEach((c) => set.add(c))
  if (cfg.value.deepl.enabled) SUPPORTED_LANGS.deepl.forEach((c) => set.add(c))
  if (cfg.value.aliyun.enabled) SUPPORTED_LANGS.aliyun.forEach((c) => set.add(c))
  return LANG_CATALOG.map((l) => l.code).filter((c) => set.has(c))
})

function defaultCfg(): AppConfig {
  return {
    engine: { enabled: true, baseUrl: 'https://api.deepseek.com/v1', apiKey: '', model: 'deepseek-chat' },
    deepl: { enabled: true, apiKey: '' },
    aliyun: { enabled: true, accessKeyId: '', accessKeySecret: '' },
    quickbar: { hotkey: 'alt+t', targetLangs: ['en', 'ja'], defaultTarget: 'en', autoCopy: true, autoPaste: true },
    clipboard: { monitor: true, minLength: 20, visionModel: '' },
    glossary: { enabled: true, maxTerms: 30 },
    outputTarget: 'zh-CN',
  }
}

onMounted(async () => {
  // 录入中窗口失焦 → 自动恢复热键，防止忘记停止导致快捷键失效
  await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (!focused) stopRecording()
  })
  try {
    const c = await invoke<AppConfig>('get_config')
    if (!c.deepl) c.deepl = { enabled: true, apiKey: '' }
    if (!c.aliyun) c.aliyun = { enabled: true, accessKeyId: '', accessKeySecret: '' }
    cfg.value = c
    if (!c.glossary) c.glossary = { enabled: true, maxTerms: 30 }
    const union = new Set<string>()
    if (c.engine?.enabled) SUPPORTED_LANGS.llm.forEach((x) => union.add(x))
    if (c.deepl?.enabled) SUPPORTED_LANGS.deepl.forEach((x) => union.add(x))
    if (c.aliyun?.enabled) SUPPORTED_LANGS.aliyun.forEach((x) => union.add(x))
    selectedLangs.value = c.quickbar.targetLangs.filter((x) => union.has(x))
    if (!selectedLangs.value.length) selectedLangs.value = [...union].slice(0, 1)
  } catch (e) {
    saveErr.value = String(e)
  }
  try {
    autostart.value = await invoke<boolean>('get_autostart')
  } catch {
    autostartErr.value = '状态读取失败'
  }
  await refreshDataInfo()
  try {
    appVersion.value = await getVersion()
  } catch {
    appVersion.value = '?'
  }
  // 托盘「检查更新…」→ 设置页执行手动检查
  await listen('updater-check', () => {
    if (!updateBusy.value) manualCheckUpdate()
  })
  // 启动 8s 静默检查发现新版本 → 提示条
  await listen<{ version: string }>('updater-available', (e) => {
    updateOk.value = true
    updateStatus.value = `发现新版本 v${e.payload.version}，点击「检查更新」查看`
  })
})

interface DataInfo {
  dir: string
  portable: boolean
  custom: boolean
}
const dataInfo = ref<DataInfo | null>(null)
const dataDir = ref('')
const applyingDir = ref(false)
const dataDirMsg = ref('')
const dataDirOk = ref(false)

async function refreshDataInfo() {
  try {
    dataInfo.value = await invoke<DataInfo>('get_data_info')
    if (!dataInfo.value.portable && !dataInfo.value.custom) dataDir.value = ''
    else if (dataInfo.value.custom) dataDir.value = dataInfo.value.dir
  } catch {
    /* ignore */
  }
}

async function browseDir() {
  try {
    const res = await invoke<{ path: string | null }>('pick_folder')
    if (res.path) dataDir.value = res.path
  } catch {
    /* 用户取消 */
  }
}

async function applyDataDir(restore = false) {
  applyingDir.value = true
  dataDirMsg.value = ''
  try {
    const path = restore ? '' : dataDir.value.trim()
    if (!restore && !path) {
      dataDirOk.value = false
      dataDirMsg.value = '请先点「浏览…」选择目录，或手动填写路径'
      return
    }
    const res = await invoke<{ ok: boolean; restored?: boolean; dir?: string }>('set_data_dir', { path })
    dataDirOk.value = true
    dataDirMsg.value = res.restored ? '已恢复默认目录并迁移数据' : `已迁移到：${res.dir ?? path}`
    await refreshDataInfo()
  } catch (e) {
    dataDirOk.value = false
    dataDirMsg.value = `失败：${e}`
  } finally {
    applyingDir.value = false
  }
}

async function toggleAutostart() {
  autostartErr.value = ''
  try {
    await invoke('set_autostart', { enable: autostart.value })
  } catch (e) {
    autostartErr.value = '设置失败，请重试'
    autostart.value = !autostart.value
  }
}

// ---------- 快捷键录入 ----------
const recording = ref(false)
const hotkeyErr = ref('')

async function toggleRecord() {
  hotkeyErr.value = ''
  if (recording.value) {
    recording.value = false
    await invoke('resume_hotkeys')
  } else {
    recording.value = true
    // 录入期间注销全局热键，避免旧组合键触发面板
    await invoke('suspend_hotkeys')
  }
}

async function stopRecording() {
  if (recording.value) {
    recording.value = false
    await invoke('resume_hotkeys')
  }
}

function keyFromEvent(e: KeyboardEvent): string | null {
  const code = e.code
  if (/^Key([A-Z])$/.test(code)) return code.slice(3).toLowerCase()
  if (/^Digit(\d)$/.test(code)) return code.slice(5)
  if (/^F(\d{1,2})$/.test(code)) return code.toLowerCase()
  const map: Record<string, string> = {
    Comma: ',', Period: '.', Slash: '/', Space: 'space',
    ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right',
    Semicolon: ';', Quote: "'", BracketLeft: '[', BracketRight: ']',
    Backslash: '\\', Minus: '-', Equal: '=', Backquote: '`',
  }
  return map[code] ?? null
}

if (typeof window !== 'undefined') {
  window.addEventListener('keydown', (e) => {
    if (!recording.value) return
    e.preventDefault()
    e.stopPropagation()
    if (e.key === 'Escape') {
      stopRecording()
      return
    }
    // 只按了修饰键时继续等待
    if (['Alt', 'Control', 'Shift', 'Meta'].includes(e.key)) return
    const key = keyFromEvent(e)
    if (!key) {
      hotkeyErr.value = `不支持的按键：${e.code}`
      return
    }
    if (!e.altKey && !e.ctrlKey && !e.metaKey) {
      hotkeyErr.value = '请搭配 Alt / Ctrl / Shift 修饰键，避免占用单键'
      return
    }
    const parts: string[] = []
    if (e.ctrlKey) parts.push('ctrl')
    if (e.altKey) parts.push('alt')
    if (e.shiftKey) parts.push('shift')
    if (e.metaKey) parts.push('meta')
    parts.push(key)
    cfg.value.quickbar.hotkey = parts.join('+')
    stopRecording()
  })
}

async function save() {
  saved.value = false
  saveErr.value = ''
  if (!selectedLangs.value.length) {
    saveErr.value = '至少选择一种可用语言'
    return
  }
  cfg.value.quickbar.targetLangs = unionSelected.value
  if (!unionLangs.value.includes(cfg.value.quickbar.defaultTarget)) {
    cfg.value.quickbar.defaultTarget = unionSelected.value[0] || 'en'
  }
  if (!unionLangs.value.includes(cfg.value.outputTarget)) {
    cfg.value.outputTarget = unionLangs.value[0] || 'zh-CN'
  }
  try {
    await invoke('save_config', { config: cfg.value })
    saved.value = true
    setTimeout(() => (saved.value = false), 2000)
    // 数据目录有变更且未应用 → 保存时自动迁移（免得用户漏点「应用」）
    const want = dataDir.value.trim()
    if (!dataInfo.value?.portable && want && want !== dataInfo.value?.dir) {
      await applyDataDir()
    }
  } catch (e) {
    saveErr.value = String(e)
  }
}

function openGlossary() {
  invoke('open_glossary')
}

// ---------- 自动更新 ----------
const appVersion = ref('…')
const updateBusy = ref(false)
const updateStatus = ref('')
const updateOk = ref<boolean | null>(null)
const updateFailed = ref(false)
const pendingUpdate = ref<Update | null>(null)
// Update 持有 Rust 侧资源（rid 为 JS 私有字段）：Vue 深度代理会让私有字段品牌校验
// 失败（TypeError: Cannot read private member）——必须 markRaw 阻止响应式代理
let pendingUpdateObj: Update | null = null

async function manualCheckUpdate() {
  updateBusy.value = true
  updateStatus.value = '检查中…'
  updateOk.value = null
  updateFailed.value = false
  pendingUpdate.value = null
  try {
    const update = await updaterCheck()
    if (update) {
      pendingUpdate.value = markRaw(update)
      pendingUpdateObj = update
      updateOk.value = true
      updateStatus.value = `发现新版本 v${update.version}，可下载安装`
    } else {
      pendingUpdateObj = null
      updateOk.value = true
      updateStatus.value = '已是最新版本'
    }
  } catch (e) {
    updateOk.value = false
    updateFailed.value = true
    updateStatus.value = `检查失败：${e}`
  } finally {
    updateBusy.value = false
  }
}

async function installUpdate() {
  const update = pendingUpdateObj
  if (!update) return
  updateBusy.value = true
  updateFailed.value = false
  let total = 0
  let downloaded = 0
  try {
    await update.downloadAndInstall((event) => {
      switch (event.event) {
        case 'Started':
          total = event.data.contentLength ?? 0
          updateStatus.value = '下载中…'
          break
        case 'Progress':
          downloaded += event.data.chunkLength
          updateStatus.value = total
            ? `下载中… ${Math.min(100, Math.round((downloaded / total) * 100))}%`
            : `下载中… ${(downloaded / 1048576).toFixed(1)}MB`
          break
        case 'Finished':
          updateStatus.value = '安装中…（即将自动重启）'
          break
      }
    })
    await relaunch()
  } catch (e) {
    updateOk.value = false
    updateFailed.value = true
    updateStatus.value = `下载/安装失败：${e}`
    updateBusy.value = false
  }
}

function openReleasePage() {
  invoke('open_release_page')
}

async function test() {
  testing.value = true
  testOk.value = null
  testMsg.value = ''
  cfg.value.quickbar.targetLangs = unionSelected.value
  try {
    await invoke('save_config', { config: cfg.value })
    const res = await invoke<{ ok: boolean; error?: string }>('test_engine')
    testOk.value = res.ok
    testMsg.value = res.ok ? '连接成功，模型可用' : res.error || '连接失败'
  } catch (e) {
    testOk.value = false
    testMsg.value = String(e)
  } finally {
    testing.value = false
  }
}
</script>
