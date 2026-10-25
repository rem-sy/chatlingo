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
        <input v-model="cfg.engine.model" type="text" placeholder="deepseek-chat" />
      </div>
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
      <h2>输入方向（我的输入 → 外语）</h2>
      <div class="form-row">
        <label>呼出快捷键</label>
        <input v-model="cfg.quickbar.hotkey" type="text" placeholder="alt+t" />
      </div>
      <div class="form-row">
        <label>目标语言</label>
        <input v-model="targetLangsStr" type="text" placeholder="en, ja" />
      </div>
      <div class="form-row">
        <label>默认目标</label>
        <select v-model="cfg.quickbar.defaultTarget">
          <option v-for="l in langsList" :key="l" :value="l">{{ l }}</option>
        </select>
      </div>
      <div class="form-row">
        <label>译文自动回填</label>
        <input type="checkbox" v-model="cfg.quickbar.autoPaste" />
        <span class="hint">翻译完成后自动写入剪贴板并模拟 Ctrl+V 粘贴回原输入框</span>
      </div>
    </section>

    <section>
      <h2>输出方向（Agent 输出 → 中文）</h2>
      <div class="form-row">
        <label>翻译剪贴板快捷键</label>
        <input v-model="cfg.clipboard.translateHotkey" type="text" placeholder="alt+y" />
      </div>
      <div class="form-row">
        <label>剪贴板自动监听</label>
        <input type="checkbox" v-model="cfg.clipboard.monitor" />
        <span class="hint">复制外文文本时自动弹出译文面板</span>
      </div>
      <div class="form-row">
        <label>最小触发长度</label>
        <input v-model.number="cfg.clipboard.minLength" type="text" />
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
import { invoke, type AppConfig } from '../shared/api'
import '../shared/base.css'
import './settings.css'

const cfg = ref<AppConfig>(defaultCfg())
const targetLangsStr = ref('en, ja')
const saved = ref(false)
const saveErr = ref('')
const testing = ref(false)
const autostart = ref(false)
const autostartErr = ref('')
const testOk = ref<boolean | null>(null)
const testMsg = ref('')

const langsList = computed(() =>
  targetLangsStr.value.split(/[,，\s]+/).filter(Boolean)
)

function defaultCfg(): AppConfig {
  return {
    engine: { baseUrl: 'https://api.deepseek.com/v1', apiKey: '', model: 'deepseek-chat' },
    deepl: { apiKey: '' },
    compareMode: false,
    quickbar: { hotkey: 'alt+t', targetLangs: ['en', 'ja'], defaultTarget: 'en', autoCopy: true, autoPaste: true },
    clipboard: { monitor: true, minLength: 20, translateHotkey: 'alt+y' },
    outputTarget: 'zh-CN',
  }
}

onMounted(async () => {
  try {
    const c = await invoke<AppConfig>('get_config')
    if (!c.deepl) c.deepl = { apiKey: '' }
    cfg.value = c
    targetLangsStr.value = cfg.value.quickbar.targetLangs.join(', ')
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
  cfg.value.quickbar.targetLangs = langsList.value
  if (!cfg.value.quickbar.targetLangs.includes(cfg.value.quickbar.defaultTarget)) {
    cfg.value.quickbar.defaultTarget = cfg.value.quickbar.targetLangs[0] || 'en'
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
  cfg.value.quickbar.targetLangs = langsList.value
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
