<template>
  <main class="quickbar">
    <header class="quickbar-head" data-tauri-drag-region>
      <h1 data-tauri-drag-region>快速翻译</h1>
      <div class="shortcuts" data-tauri-drag-region>
        <kbd data-tauri-drag-region>Shift+Enter</kbd><span data-tauri-drag-region>换行</span>
        <kbd data-tauri-drag-region>Esc</kbd><span data-tauri-drag-region>收起</span>
      </div>
      <button class="quiet close" aria-label="收起翻译面板" title="收起（Esc）" @click="hide">×</button>
    </header>
    <div class="input-zone" :style="{ height: inputH + 'px' }">
      <textarea
        ref="inputEl"
        v-model="source"
        aria-label="原文"
        placeholder="输入或粘贴要翻译的文字"
      ></textarea>
    </div>
    <div
      class="splitter"
      role="separator"
      tabindex="0"
      aria-label="调整原文区域高度"
      aria-orientation="horizontal"
      :aria-valuemin="56"
      :aria-valuemax="maxInputHeight"
      :aria-valuenow="Math.min(maxInputHeight, inputH)"
      title="拖动或使用方向键调整原文区域高度"
      @mousedown="startSplitDrag"
      @keydown.up.prevent="resizeInput(-16)"
      @keydown.down.prevent="resizeInput(16)"
    >
      <span></span>
    </div>
    <div class="output-zone" aria-label="译文">
      <div v-if="note" class="feedback warning" role="status">{{ note }}</div>
      <div v-if="error" class="feedback error" role="alert">{{ error }}</div>
      <article v-for="eng in multiOrder" :key="eng" class="result">
        <header class="result-head">
          <span class="chip">{{ engineName(eng) }}</span>
          <button class="quiet" :disabled="!multiTexts[eng]" :aria-label="'复制' + engineName(eng) + '译文'" @click="copy(multiTexts[eng], eng)">{{ copied === eng ? '已复制' : '复制' }}</button>
        </header>
        <div v-if="multiTexts[eng]" class="output-text" dir="auto">{{ multiTexts[eng] }}</div>
        <div v-else class="result-placeholder">{{ translating ? '等待译文…' : '未返回译文' }}</div>
        <div v-if="glossaryWarns[eng]" class="feedback warning">{{ glossaryWarns[eng] }}</div>
      </article>
      <article v-if="!multiOrder.length && (translated || translating)" class="result">
        <header class="result-head">
          <span class="chip">译文 · {{ langDef(resultTarget).name }}</span>
          <div v-if="translated" class="result-actions">
            <button class="quiet" @click="copy(translated, 'single')">{{ copied === 'single' ? '已复制' : '复制' }}</button>
            <button v-if="!translating && resultTarget !== 'zh-CN'" class="quiet" @click="paste">粘贴回填</button>
          </div>
        </header>
        <div v-if="translated" class="output-text" :class="{ streaming }" dir="auto">{{ translated }}</div>
        <div v-else class="result-placeholder">等待译文…</div>
        <div v-if="glossaryWarns['']" class="feedback warning">{{ glossaryWarns[''] }}</div>
      </article>
    </div>
    <footer class="toolbar" data-tauri-drag-region>
      <label class="language-label" for="target-language">译为</label>
      <select id="target-language" v-model="target">
        <option v-for="lang in langs" :key="lang" :value="lang">{{ langDef(lang).name }}</option>
      </select>
      <button class="quiet" :disabled="translating" title="框选屏幕区域，识别文字到输入框" @click="snap">截图</button>
      <span class="status" role="status" data-tauri-drag-region>{{ statusText }}</span>
      <button class="primary submit" :disabled="translating || !source.trim()" @click="submit">
        {{ translating ? '翻译中…' : '翻译' }} <kbd v-if="!translating" aria-hidden="true">↵</kbd>
      </button>
    </footer>
  </main>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke, type AppConfig, engineName, langDef } from '../shared/api'
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
const resultTarget = ref('en')
const note = ref('')
const multiOrder = ref<string[]>([])
const multiTexts = ref<Record<string, string>>({})
const glossaryWarns = ref<Record<string, string>>({})
let reqId = 0
const copied = ref('')
let copyTimer: number | undefined
const statusText = computed(() => {
  if (autoDone.value) return '已回填'
  if (translating.value) return ''
  if (translated.value || Object.values(multiTexts.value).some(Boolean)) return '翻译完成'
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
    if (e.payload.id === 0 && !e.payload.ok) {
      note.value = ''
      error.value = e.payload.error || '截图识别失败'
      return
    }
    if (e.payload.id !== reqId) return
    translating.value = false
    streaming.value = false
    if (e.payload.ok) {
      autoDone.value = !!e.payload.pasted
    } else {
      error.value = e.payload.error || '翻译失败'
    }
  })
  await listen<{ engine?: string; text: string }>('glossary-warn', (e) => {
    // 多引擎模式挂到对应引擎卡片；单引擎模式（无卡片）归到 ''
    const eng = e.payload.engine || ''
    const key = multiOrder.value.includes(eng) ? eng : ''
    glossaryWarns.value[key] = e.payload.text
  })
  await listen('quickbar-reset', () => {
    reqId += 1
    source.value = ''
    resetResult()
    note.value = ''
    inputEl.value?.focus()
    void refreshLanguages()
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
    if (e.payload.text) {
      source.value = e.payload.text
      resetResult()
    }
    inputEl.value?.focus()
  })
  await refreshLanguages()
})

let configuredTarget = ''
async function refreshLanguages() {
  try {
    const config = await invoke<AppConfig>('get_config')
    const list = [...new Set([...config.quickbar.targetLangs, config.outputTarget])]
    if (!list.length) return
    langs.value = list
    if (configuredTarget !== config.quickbar.defaultTarget || !list.includes(target.value)) {
      target.value = list.includes(config.quickbar.defaultTarget) ? config.quickbar.defaultTarget : list[0]
    }
    configuredTarget = config.quickbar.defaultTarget
  } catch (e) {
    error.value = `无法读取语言设置：${e}`
  }
}

const inputH = ref(Math.max(56, Number(localStorage.getItem('qb-input-h')) || 92))
const maxInputHeight = ref(56)
function updateInputLimit() {
  maxInputHeight.value = Math.max(56, window.innerHeight - 132)
}
updateInputLimit()
window.addEventListener('resize', updateInputLimit)

function resizeInput(delta: number) {
  inputH.value = Math.min(Math.max(Math.min(inputH.value, maxInputHeight.value) + delta, 56), maxInputHeight.value)
  localStorage.setItem('qb-input-h', String(inputH.value))
}

function startSplitDrag(e: MouseEvent) {
  if (e.button !== 0) return
  e.preventDefault()
  const startY = e.clientY
  const startH = Math.min(inputH.value, maxInputHeight.value)
  const onMove = (ev: MouseEvent) => {
    inputH.value = Math.min(Math.max(startH + ev.clientY - startY, 56), maxInputHeight.value)
  }
  const onUp = () => {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
    localStorage.setItem('qb-input-h', String(inputH.value))
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

// 面板内任意位置按 Enter 都触发翻译（按钮聚焦时除外，保留按钮原生行为）
if (typeof window !== 'undefined') {
  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      hide()
      return
    }
    if (e.key !== 'Enter' || e.shiftKey || e.isComposing) return
    const tag = (e.target as HTMLElement | null)?.tagName
    if (tag === 'BUTTON' || tag === 'SELECT' || tag === 'OPTION') return
    e.preventDefault()
    submit()
  })
}

function resetResult() {
  translating.value = false
  streaming.value = false
  translated.value = ''
  error.value = ''
  autoDone.value = false
  multiOrder.value = []
  multiTexts.value = {}
  glossaryWarns.value = {}
  copied.value = ''
  clearTimeout(copyTimer)
}

async function submit() {
  const text = source.value.trim()
  if (!text || translating.value) return
  resetResult()
  note.value = ''
  translating.value = true
  resultTarget.value = target.value
  const id = ++reqId
  try {
    await invoke('submit_input', { text, target: resultTarget.value, id })
  } catch (e) {
    if (id !== reqId) return
    translating.value = false
    streaming.value = false
    error.value = `翻译失败：${e}`
  }
}

function hide() {
  void invoke('hide_quickbar')
}

async function snap() {
  error.value = ''
  try {
    await invoke('request_screenshot', {})
  } catch (e) {
    error.value = `截图失败：${e}`
  }
}

async function copy(text: string, key: string) {
  if (!text) return
  try {
    await invoke('copy_text', { text })
    copied.value = key
    clearTimeout(copyTimer)
    copyTimer = window.setTimeout(() => { copied.value = '' }, 1600)
  } catch (e) {
    error.value = `复制失败：${e}`
  }
}

async function paste() {
  if (!translated.value) return
  try {
    await invoke('paste_text', { text: translated.value })
  } catch (e) {
    error.value = `回填失败：${e}`
  }
}
</script>
