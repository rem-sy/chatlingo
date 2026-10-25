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
        <input v-model="cfg.quickbar.hotkey" type="text" placeholder="alt+t" />
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
})

async function toggleAutostart() {
  autostartErr.value = ''
  try {
    await invoke('set_autostart', { enable: autostart.value })
  } catch (e) {
    autostartErr.value = '设置失败，请重试'
    autostart.value = !autostart.value
  }
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
