<template>
  <div class="quickbar" :class="{ translating }" data-tauri-drag-region>
    <div class="dragbar" data-tauri-drag-region title="按住拖动">⠿</div>
    <div class="input-zone">
      <textarea
        ref="inputEl"
        v-model="source"
        :placeholder="placeholder"
        rows="3"
      ></textarea>
    </div>
    <div v-if="note" class="note">{{ note }}</div>
    <div v-if="translated || streaming || error || multiOrder.length" class="divider"></div>
    <div v-if="error" class="error">{{ error }}</div>
    <div v-else-if="multiOrder.length" class="output-zone">
      <div v-for="eng in multiOrder" :key="eng" class="multi-block">
        <div class="multi-head">
          <span class="multi-tag">{{ engName(eng) }}</span>
          <button class="multi-copy" @click="copy(multiTexts[eng] || '')">复制</button>
        </div>
        <div class="output-text">{{ multiTexts[eng] }}</div>
      </div>
    </div>
    <div v-else-if="translated || streaming" class="output-zone">
      <div class="output-text" :class="{ streaming }">{{ display }}</div>
    </div>
    <div class="toolbar" data-tauri-drag-region>
      <div class="langs">
        <button
          v-for="lang in langs"
          :key="lang"
          class="lang-pill"
          :class="{ active: target === lang }"
          @click="target = lang"
        >
          {{ label(lang) }}
        </button>
      </div>
      <button class="snap-btn" title="框选屏幕区域，识别文字到输入框" @click="snap">截图</button>
      <div class="status" data-tauri-drag-region>{{ statusText }}</div>
      <div class="actions">
        <button class="primary" :disabled="translating || !source.trim()" @click="submit">{{ translating ? '翻译中…' : '翻译' }}</button>
        <button v-if="translated && !translating" @click="copy">复制</button>
        <button v-if="translated && !translating && target !== 'zh-CN'" class="primary" @click="paste">粘贴回填</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke, type AppConfig, langLabel } from '../shared/api'
import '../shared/base.css'
import './quickbar.css'

const inputEl = ref<HTMLTextAreaElement>()
const source = ref('')
const translated = ref('')
const streaming = ref(false)
const translating = ref(false)
const error = ref('')
const target = ref('en')
const langs = ref<string[]>(['en', 'ja', 'zh-CN'])
const cfg = ref<AppConfig | null>(null)
const note = ref('')
const multiOrder = ref<string[]>([])
const multiTexts = ref<Record<string, string>>({})
let reqId = 0

const ENGINE_NAMES: Record<string, string> = { llm: '大模型', deepl: 'DeepL', aliyun: '阿里云' }
const engName = (id: string) => ENGINE_NAMES[id] ?? id

const label = (l: string) => langLabel(l)
const targetLabel = computed(() => label(target.value))
const placeholder = computed(() =>
  target.value === 'zh-CN'
    ? '输入、粘贴或截图识别文字，回车翻译为中文…'
    : `输入、粘贴或截图识别文字，回车翻译为 ${targetLabel.value}…`
)
const display = computed(() => translated.value || '')
const statusText = computed(() => {
  if (note.value) return note.value
  if (translating.value) return `翻译中… → ${targetLabel.value}`
  if (error.value) return '失败'
  if (translated.value) return `已译为 ${targetLabel.value}${autoDone.value ? ' · 已回填' : ''}`
  return ''
})
const autoDone = ref(false)

onMounted(async () => {
  inputEl.value?.focus()
  // 拖动后记住位置（防抖），下次 Alt+T 停在用户拖到的位置
  let moveTimer: number | undefined
  await getCurrentWindow().onMoved(({ payload }) => {
    if (moveTimer) clearTimeout(moveTimer)
    moveTimer = window.setTimeout(() => {
      invoke('save_quickbar_pos', { x: payload.x, y: payload.y })
    }, 500)
  })
  cfg.value = await invoke<AppConfig>('get_config')
  // 统一方向：输入方向目标语言 + 输出方向（中文），去重
  const list = [...cfg.value.quickbar.targetLangs]
  if (!list.includes(cfg.value.outputTarget)) list.push(cfg.value.outputTarget)
  langs.value = list
  target.value = cfg.value.quickbar.defaultTarget

  await listen<{ id: number; text: string; engine?: string }>('quickbar-chunk', (e) => {
    if (e.payload.id !== reqId) return
    streaming.value = true
    if (e.payload.engine) {
      multiTexts.value[e.payload.engine] = (multiTexts.value[e.payload.engine] || '') + e.payload.text
    } else {
      translated.value += e.payload.text
    }
  })
  await listen<{ id: number; engines: string[] }>('quickbar-multi-start', (e) => {
    if (e.payload.id !== reqId) return
    multiOrder.value = e.payload.engines
    multiTexts.value = {}
  })
  await listen<{ id: number; ok: boolean; error?: string; pasted?: boolean }>('quickbar-done', (e) => {
    if (e.payload.id !== reqId) return
    translating.value = false
    streaming.value = false
    if (e.payload.ok) {
      autoDone.value = !!e.payload.pasted
    } else {
      error.value = e.payload.error || '翻译失败'
    }
  })
  await listen('quickbar-reset', () => {
    source.value = ''
    translated.value = ''
    error.value = ''
    note.value = ''
    multiOrder.value = []
    multiTexts.value = {}
    translating.value = false
    streaming.value = false
    autoDone.value = false
    inputEl.value?.focus()
  })
  await listen<{ note: string }>('engine-note', (e) => {
    note.value = e.payload.note
  })
  // 截图识别的文字回填输入框（可编辑，回车再翻译）
  await listen<{ text: string }>('quickbar-chunk-status', (e) => {
    note.value = e.payload.text
  })
  await listen<{ text: string }>('quickbar-ocr', (e) => {
    note.value = ''
    source.value = e.payload.text
    translated.value = ''
    error.value = ''
    inputEl.value?.focus()
  })
})

// 面板内任意位置按 Enter 都触发翻译（按钮聚焦时除外，保留按钮原生行为）
if (typeof window !== 'undefined') {
  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      invoke('hide_quickbar')
      return
    }
    if (e.key !== 'Enter' || e.shiftKey || e.isComposing) return
    const tag = (e.target as HTMLElement | null)?.tagName
    if (tag === 'BUTTON' || tag === 'SELECT' || tag === 'OPTION') return
    e.preventDefault()
    submit()
  })
}

async function submit() {
  const text = source.value.trim()
  if (!text || translating.value) return
  translating.value = true
  streaming.value = false
  translated.value = ''
  error.value = ''
  autoDone.value = false
  multiOrder.value = []
  multiTexts.value = {}
  reqId += 1
  await invoke('submit_input', { text, target: target.value, id: reqId })
}

async function snap() {
  await invoke('request_screenshot', {})
}

async function copy() {
  if (!translated.value) return
  await invoke('copy_text', { text: translated.value })
}

async function paste() {
  if (!translated.value) return
  await invoke('paste_text', { text: translated.value })
}
</script>
