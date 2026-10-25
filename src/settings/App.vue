<template>
  <div class="settings">
    <h1>ChatLingo 设置</h1>

    <section>
      <h2>翻译引擎（OpenAI 兼容）</h2>
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
      <h2>兜底引擎（DeepL，可选）</h2>
      <div class="form-row">
        <label>DeepL API Key</label>
        <input v-model="cfg.deepl.apiKey" type="password" placeholder="留空则不启用兜底；免费版 Key 以 :fx 结尾" />
      </div>
      <div class="form-row">
        <label></label>
        <span class="hint">大模型请求失败时自动用 DeepL 翻译；每月免费 50 万字符</span>
      </div>
      <div class="form-row">
        <label>双引擎对比</label>
        <input type="checkbox" v-model="cfg.compareMode" />
        <span class="hint">输出方向同时用大模型和 DeepL 翻译，面板并列展示择优</span>
      </div>
    </section>

    <section>
      <h2>语言</h2>
      <div class="form-row">
        <label>可用语言</label>
        <div class="lang-grid">
          <label v-for="l in LANG_CATALOG" :key="l.code" class="lang-check">
            <input
              type="checkbox"
              :value="l.code"
              v-model="selectedLangs"
            />
            {{ l.label }} {{ l.name }}
          </label>
        </div>
      </div>
      <div class="form-row">
        <label>默认目标</label>
        <select v-model="cfg.quickbar.defaultTarget">
          <option v-for="c in selectedLangs" :key="c" :value="c">{{ langDef(c).label }} {{ langDef(c).name }}</option>
        </select>
        <span class="hint">Alt+T 面板初始选中的目标语言</span>
      </div>
      <div class="form-row">
        <label>输出方向目标</label>
        <select v-model="cfg.outputTarget">
          <option v-for="l in LANG_CATALOG" :key="l.code" :value="l.code">{{ l.label }} {{ l.name }}</option>
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
        <button :disabled="dataInfo?.portable || applyingDir" @click="applyDataDir">{{ applyingDir ? '迁移中…' : '应用' }}</button>
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
    </section>

    <footer>
      <span class="saved" v-if="saved">已保存 ✓</span>
      <span class="err" v-if="saveErr">{{ saveErr }}</span>
      <button class="primary" @click="save">保存</button>
    </footer>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke, type AppConfig, LANG_CATALOG, langDef } from '../shared/api'
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
  cfg.value.quickbar.targetLangs = orderedLangs.value
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
const orderedLangs = computed(() =>
  LANG_CATALOG.map((l) => l.code).filter((c) => selectedLangs.value.includes(c))
)

function defaultCfg(): AppConfig {
  return {
    engine: { baseUrl: 'https://api.deepseek.com/v1', apiKey: '', model: 'deepseek-chat' },
    deepl: { apiKey: '' },
    compareMode: false,
    quickbar: { hotkey: 'alt+t', targetLangs: ['en', 'ja'], defaultTarget: 'en', autoCopy: true, autoPaste: true },
    clipboard: { monitor: true, minLength: 20, visionModel: '' },
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
    if (!c.deepl) c.deepl = { apiKey: '' }
    cfg.value = c
    selectedLangs.value = [...c.quickbar.targetLangs]
    if (!selectedLangs.value.length) selectedLangs.value = ['en']
  } catch (e) {
    saveErr.value = String(e)
  }
  try {
    autostart.value = await invoke<boolean>('get_autostart')
  } catch {
    autostartErr.value = '状态读取失败'
  }
  await refreshDataInfo()
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
    await invoke('set_data_dir', { path })
    dataDirOk.value = true
    dataDirMsg.value = restore ? '已恢复默认目录并迁移数据' : '数据目录已应用并完成迁移'
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
  cfg.value.quickbar.targetLangs = orderedLangs.value
  if (!selectedLangs.value.includes(cfg.value.quickbar.defaultTarget)) {
    cfg.value.quickbar.defaultTarget = orderedLangs.value[0]
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

async function test() {
  testing.value = true
  testOk.value = null
  testMsg.value = ''
  cfg.value.quickbar.targetLangs = orderedLangs.value
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
