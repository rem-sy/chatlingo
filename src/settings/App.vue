<template>
  <main class="settings">
    <aside class="settings-sidebar">
      <div class="app-brand"><span class="brand-mark" aria-hidden="true">译</span><span>ChatLingo</span></div>
      <nav aria-label="设置分类">
        <button
          v-for="item in pages"
          :key="item.id"
          :aria-current="page === item.id ? 'page' : undefined"
          @click="page = item.id"
        >{{ item.title }}</button>
      </nav>
    </aside>

    <div class="settings-main">
      <div ref="scrollArea" class="settings-scroll">
        <header class="settings-heading">
          <h1 id="settings-title" class="view-title">{{ currentPage.title }}</h1>
          <p class="view-description">{{ currentPage.description }}</p>
        </header>

        <div v-show="page === 'engines'" class="settings-content" aria-labelledby="settings-title">
          <section>
            <header class="card-heading">
              <h2><label for="llm-enabled">大模型</label></h2>
              <input id="llm-enabled" v-model="cfg.engine.enabled" class="switch" type="checkbox" role="switch" aria-label="启用大模型" />
            </header>
            <div class="form-row">
              <label for="provider">渠道预设</label>
              <div class="field">
                <select id="provider" v-model="selectedProvider" @change="applyProvider">
                  <option v-for="p in providers" :key="p.name" :value="p.name">{{ p.name }}</option>
                </select>
                <span class="hint">选择预设可填充地址、接口格式与推荐模型。{{ selectedPreset?.netNote }}</span>
              </div>
            </div>
            <div class="form-row">
              <label for="base-url">Base URL</label>
              <input id="base-url" v-model="cfg.engine.baseUrl" type="text" placeholder="https://api.deepseek.com/v1" spellcheck="false" />
            </div>
            <div class="form-row">
              <label for="api-format">接口格式</label>
              <div class="field">
                <select id="api-format" v-model="cfg.engine.apiFormat">
                  <option value="openai-completions">OpenAI Chat 兼容</option>
                  <option value="openai-responses">OpenAI Responses</option>
                  <option value="anthropic-messages">Anthropic Messages</option>
                </select>
                <span class="hint">{{ formatHint }}
                  <a v-if="selectedPreset?.docUrl && selectedPreset.name !== '自定义'" :href="selectedPreset.docUrl" target="_blank" rel="noopener">渠道文档 ↗</a>
                </span>
              </div>
            </div>
            <div class="form-row">
              <label for="llm-key">API Key</label>
              <input id="llm-key" v-model="cfg.engine.apiKey" type="password" placeholder="sk-..." />
            </div>
            <div class="form-row">
              <label for="llm-model">模型</label>
              <div class="field-controls">
                <input id="llm-model" v-model="cfg.engine.model" type="text" list="model-list" placeholder="例如 deepseek-chat" spellcheck="false" />
                <button :disabled="loadingModels || cfg.engine.apiFormat === 'anthropic-messages'" @click="fetchModels">{{ loadingModels ? '获取中…' : '获取模型' }}</button>
              </div>
            </div>
            <div v-if="modelMsg" class="feedback" :class="models.length ? 'success' : 'error'" role="status">{{ modelMsg }}</div>
            <div class="card-actions">
              <button :disabled="testing" @click="test">{{ testing ? '测试中…' : '测试连接' }}</button>
              <span v-if="testMsg" class="feedback" :class="testOk ? 'success' : 'error'" role="status">{{ testMsg }}</span>
            </div>
          </section>

          <section>
            <header class="card-heading">
              <h2><label for="deepl-enabled">DeepL</label></h2>
              <input id="deepl-enabled" v-model="cfg.deepl.enabled" class="switch" type="checkbox" role="switch" aria-label="启用 DeepL" />
            </header>
            <div class="form-row">
              <label for="deepl-key">API Key</label>
              <div class="field">
                <input id="deepl-key" v-model="cfg.deepl.apiKey" type="password" placeholder="免费 API Key 以 :fx 结尾" />
                <a class="hint" href="https://www.deepl.com/pro#developer" target="_blank" rel="noopener">获取 DeepL API Key ↗</a>
              </div>
            </div>
          </section>

          <section>
            <header class="card-heading">
              <h2><label for="aliyun-enabled">阿里云机器翻译</label></h2>
              <input id="aliyun-enabled" v-model="cfg.aliyun.enabled" class="switch" type="checkbox" role="switch" aria-label="启用阿里云机器翻译" />
            </header>
            <div class="form-row">
              <label for="aliyun-id">AccessKey ID</label>
              <input id="aliyun-id" v-model="cfg.aliyun.accessKeyId" type="password" />
            </div>
            <div class="form-row">
              <label for="aliyun-key">AccessKey Secret</label>
              <input id="aliyun-key" v-model="cfg.aliyun.accessKeySecret" type="password" />
            </div>
          </section>
        </div>

        <div v-show="page === 'languages'" class="settings-content" aria-labelledby="settings-title">
          <section>
            <header class="card-heading"><h2>目标语言</h2></header>
            <div class="field">
              <span id="available-languages" class="field-label">快速翻译可用语言</span>
              <div v-if="unionLangs.length" class="lang-grid" role="group" aria-labelledby="available-languages">
                <label v-for="code in unionLangs" :key="code" class="lang-check">
                  <input v-model="selectedLangs" type="checkbox" :value="code" />
                  {{ langDef(code).name }}
                </label>
              </div>
              <div v-else class="feedback warning">启用翻译引擎以选择可用语言。</div>
            </div>
            <div class="form-row">
              <label for="default-target">快速翻译默认目标</label>
              <select id="default-target" v-model="cfg.quickbar.defaultTarget" :disabled="!unionSelected.length">
                <option v-for="code in unionSelected" :key="code" :value="code">{{ langDef(code).name }}</option>
              </select>
            </div>
            <div class="form-row">
              <label for="output-target">剪贴板翻译目标</label>
              <select id="output-target" v-model="cfg.outputTarget" :disabled="!unionLangs.length">
                <option v-for="code in unionLangs" :key="code" :value="code">{{ langDef(code).name }}</option>
              </select>
            </div>
          </section>

          <section>
            <header class="card-heading"><h2>快速翻译</h2></header>
            <div class="form-row">
              <label for="hotkey">呼出快捷键</label>
              <div class="field">
                <div class="field-controls">
                  <input id="hotkey" :value="recording ? '请按下组合键…' : cfg.quickbar.hotkey" type="text" readonly :class="{ recording }" />
                  <button @click="toggleRecord">{{ recording ? '取消录入' : '录入快捷键' }}</button>
                </div>
                <span class="hint" role="status">{{ hotkeyErr || '录入时可按 Esc 取消。' }}</span>
              </div>
            </div>
            <div class="form-row">
              <label for="auto-paste">译文自动回填</label>
              <div class="field-controls">
                <input id="auto-paste" v-model="cfg.quickbar.autoPaste" class="switch" type="checkbox" role="switch" />
                <span class="hint">将译文粘贴回原输入框。</span>
              </div>
            </div>
          </section>
        </div>

        <div v-show="page === 'capture'" class="settings-content" aria-labelledby="settings-title">
          <section>
            <header class="card-heading"><h2>剪贴板监听</h2></header>
            <div class="form-row">
              <label for="monitor">自动监听</label>
              <div class="field-controls">
                <input id="monitor" v-model="cfg.clipboard.monitor" class="switch" type="checkbox" role="switch" />
                <span class="hint">复制外文时显示剪贴板翻译面板。</span>
              </div>
            </div>
            <div class="form-row">
              <label for="min-length">最小触发长度</label>
              <input id="min-length" v-model.number="cfg.clipboard.minLength" type="number" min="0" step="1" class="short-field" />
            </div>
          </section>

          <section>
            <header class="card-heading"><h2>截图识别</h2></header>
            <div class="form-row">
              <label for="vision-model">视觉模型</label>
              <div class="field">
                <div class="field-controls">
                  <input id="vision-model" v-model="cfg.clipboard.visionModel" type="text" list="model-list" placeholder="留空时使用翻译模型" spellcheck="false" />
                  <button :disabled="loadingModels" @click="fetchModels">{{ loadingModels ? '获取中…' : '获取模型' }}</button>
                </div>
                <span class="hint">模型须支持图片输入。</span>
              </div>
            </div>
            <div v-if="modelMsg" class="feedback" :class="models.length ? 'success' : 'error'" role="status">{{ modelMsg }}</div>
          </section>
        </div>

        <div v-show="page === 'glossary'" class="settings-content" aria-labelledby="settings-title">
          <section>
            <div class="form-row first-row">
              <label for="use-glossary">启用术语库</label>
              <input id="use-glossary" v-model="cfg.glossary.enabled" class="switch" type="checkbox" role="switch" />
            </div>
            <div class="form-row">
              <label for="max-terms">最大注入条数</label>
              <div class="field-controls">
                <input id="max-terms" v-model.number="cfg.glossary.maxTerms" type="number" min="0" step="1" class="short-field" />
                <span class="hint">超出上限时优先使用较长的术语。</span>
              </div>
            </div>
            <div class="card-actions"><button @click="openGlossary">打开术语库</button></div>
          </section>
        </div>

        <div v-show="page === 'general'" class="settings-content" aria-labelledby="settings-title">
          <section>
            <header class="card-heading"><h2>应用</h2></header>
            <div class="form-row">
              <label for="autostart">开机自启</label>
              <div class="field-controls">
                <input id="autostart" v-model="autostart" class="switch" type="checkbox" role="switch" @change="toggleAutostart" />
                <span class="hint" role="status">{{ autostartErr || '开机后自动驻留托盘。' }}</span>
              </div>
            </div>
          </section>
          <section>
            <header class="card-heading"><h2>数据目录</h2></header>
            <div class="field">
              <label for="data-directory" class="field-label">存储位置</label>
              <input id="data-directory" v-model="dataDir" type="text" placeholder="%APPDATA%" :disabled="dataInfo?.portable" />
              <span v-if="dataInfo" class="hint">当前目录 <span class="directory">{{ dataInfo.dir }}</span> · {{ dataInfo.portable ? '便携模式' : dataInfo.custom ? '自定义' : '默认' }}</span>
              <span v-if="!dataInfo?.portable" class="hint">应用或保存新路径时将迁移数据。可在 exe 同目录创建 chatlingo.portable 文件以使用便携模式。</span>
            </div>
            <div class="card-actions">
              <button :disabled="dataInfo?.portable" @click="browseDir">浏览…</button>
              <button :disabled="dataInfo?.portable || applyingDir" @click="applyDataDir()">{{ applyingDir ? '迁移中…' : '应用目录' }}</button>
              <button v-if="dataInfo?.custom" :disabled="applyingDir" @click="applyDataDir(true)">恢复默认</button>
            </div>
            <div v-if="dataDirMsg" class="feedback" :class="dataDirOk ? 'success' : 'error'" role="status">{{ dataDirMsg }}</div>
          </section>
          <section>
            <header class="card-heading"><h2>软件更新</h2><span class="chip">v{{ appVersion }}</span></header>
            <div class="card-actions">
              <button :disabled="updateBusy" @click="manualCheckUpdate">检查更新</button>
              <button v-if="pendingUpdate" class="primary" :disabled="updateBusy" @click="installUpdate">安装 v{{ pendingUpdate.version }}</button>
              <button v-if="updateFailed" @click="openReleasePage">打开发布页 ↗</button>
            </div>
            <div v-if="updateStatus" class="feedback" :class="updateFailed ? 'error' : updateOk && !updateBusy ? 'success' : ''" role="status">{{ updateStatus }}</div>
            <p v-if="pendingUpdate?.body" class="hint update-notes">{{ pendingUpdate.body }}</p>
          </section>
        </div>
      </div>

      <footer class="settings-footer">
        <span v-if="saved" class="feedback success" role="status">已保存</span>
        <span v-if="saveErr" class="feedback error" role="alert">{{ saveErr }}</span>
        <button class="primary" :disabled="saving" @click="save">{{ saving ? '保存中…' : '保存设置' }}</button>
      </footer>
    </div>

    <datalist id="model-list">
      <option v-for="model in modelOptions" :key="model" :value="model" />
    </datalist>
  </main>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, markRaw } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke, type AppConfig, type ApiFormat, type ProviderPreset, LANG_CATALOG, langDef, SUPPORTED_LANGS } from '../shared/api'
import { check as updaterCheck, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { getVersion } from '@tauri-apps/api/app'
import '../shared/base.css'
import './settings.css'

const pages = [
  { id: 'engines', title: '翻译引擎', description: '配置翻译服务。启用多个引擎时可对比各自的译文。' },
  { id: 'languages', title: '语言与快捷键', description: '选择目标语言并设置快速翻译的操作方式。' },
  { id: 'capture', title: '剪贴板与截图', description: '控制自动翻译和截图文字识别。' },
  { id: 'glossary', title: '术语库', description: '让翻译引擎使用你指定的术语译法。' },
  { id: 'general', title: '通用与更新', description: '管理开机启动、本地数据和应用版本。' },
] as const
const page = ref<(typeof pages)[number]['id']>('engines')
const currentPage = computed(() => pages.find((item) => item.id === page.value) ?? pages[0])
const scrollArea = ref<HTMLElement>()
watch(page, () => scrollArea.value?.scrollTo(0, 0))

const cfg = ref<AppConfig>(defaultCfg())
const selectedLangs = ref<string[]>(['en', 'ja'])
const saved = ref(false)
const saving = ref(false)
let saveTimer: number | undefined
const saveErr = ref('')
const testing = ref(false)
const autostart = ref(false)
const autostartErr = ref('')
const testOk = ref<boolean | null>(null)
const testMsg = ref('')
const models = ref<string[]>([])
const loadingModels = ref(false)
const modelMsg = ref('')

// ---- 渠道预设 ----
const providers = ref<ProviderPreset[]>([{ name: '自定义', baseUrl: '', apiFormat: 'openai-completions', models: [], docUrl: '', netNote: '' }])
const selectedProvider = ref('自定义')
const selectedPreset = computed(() => providers.value.find((p) => p.name === selectedProvider.value))
const modelOptions = computed(() => [...new Set([...(selectedPreset.value?.models ?? []), ...models.value])])
const formatHint = computed(() => {
  switch (cfg.value.engine.apiFormat) {
    case 'openai-completions':
      return '使用 /chat/completions。'
    case 'openai-responses':
      return '使用 /responses。'
    case 'anthropic-messages':
      return '使用 /v1/messages。模型名称可手动输入。'
    default:
      return ''
  }
})

function applyProvider() {
  const p = selectedPreset.value
  if (!p) return
  models.value = []
  modelMsg.value = ''
  if (p.baseUrl) {
    cfg.value.engine.baseUrl = p.baseUrl
    cfg.value.engine.apiFormat = p.apiFormat
    if (p.models.length) cfg.value.engine.model = p.models[0]
  }
}

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

watch([unionSelected, unionLangs], ([selected, available]) => {
  const bar = cfg.value.quickbar
  if (selected.length && !selected.includes(bar.defaultTarget)) bar.defaultTarget = selected[0]
  if (available.length && !available.includes(cfg.value.outputTarget)) {
    cfg.value.outputTarget = available.includes('zh-CN') ? 'zh-CN' : available[0]
  }
})

function defaultCfg(): AppConfig {
  return {
    engine: { enabled: true, baseUrl: 'https://api.deepseek.com/v1', apiKey: '', model: 'deepseek-chat', apiFormat: 'openai-completions' as ApiFormat },
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
    if (!c.engine) c.engine = defaultCfg().engine
    if (!c.engine.apiFormat) c.engine.apiFormat = 'openai-completions'
    if (!c.glossary) c.glossary = { enabled: true, maxTerms: 30 }
    cfg.value = c
    selectedLangs.value = c.quickbar.targetLangs.filter((code) => unionLangs.value.includes(code))
    if (!selectedLangs.value.length) selectedLangs.value = unionLangs.value.slice(0, 1)
    // 渠道预设列表（Rust 嵌入 providers.json）
    try {
      providers.value = await invoke<ProviderPreset[]>('list_providers')
      const match = providers.value.find(
        (p) => p.baseUrl && p.baseUrl === c.engine.baseUrl && p.apiFormat === c.engine.apiFormat
      )
      selectedProvider.value = match ? match.name : '自定义'
    } catch (e) {
      console.warn('providers 加载失败', e)
    }
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
    page.value = 'general'
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
  if (saving.value) return
  clearTimeout(saveTimer)
  saved.value = false
  saveErr.value = ''
  if (unionLangs.value.length && !unionSelected.value.length) {
    page.value = 'languages'
    saveErr.value = '至少选择一种可用语言'
    return
  }
  for (const [value, destination, label] of [
    [cfg.value.clipboard.minLength, 'capture', '最小触发长度'],
    [cfg.value.glossary.maxTerms, 'glossary', '最大注入条数'],
  ] as const) {
    if (!Number.isSafeInteger(value) || value < 0) {
      page.value = destination
      saveErr.value = `${label}须为大于或等于 0 的整数`
      return
    }
  }
  cfg.value.quickbar.targetLangs = unionSelected.value
  saving.value = true
  try {
    await invoke('save_config', { config: cfg.value })
    // 数据目录有变更且未应用 → 保存时自动迁移（免得用户漏点「应用」）
    const want = dataDir.value.trim()
    if (!dataInfo.value?.portable && want && want !== dataInfo.value?.dir) {
      await applyDataDir()
      if (!dataDirOk.value) {
        page.value = 'general'
        saveErr.value = dataDirMsg.value
        return
      }
    }
    saved.value = true
    saveTimer = window.setTimeout(() => { saved.value = false }, 2000)
  } catch (e) {
    saveErr.value = String(e)
  } finally {
    saving.value = false
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
