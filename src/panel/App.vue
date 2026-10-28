<template>
  <div class="panel">
    <div class="panel-head" data-tauri-drag-region>
      <span class="badge">Agent 输出 → {{ outputLabel }}</span>
      <span class="status">{{ statusText }}</span>
      <div class="spacer" data-tauri-drag-region></div>
      <button class="mini" title="最小化" @click="minimize">—</button>
      <button v-if="!compare" class="mini" title="复制译文" @click="copyTranslation">复制</button>
      <button class="mini" title="关闭" @click="close">×</button>
    </div>
    <div class="panel-body">
      <div v-if="note" class="engine-note">{{ note }}</div>
      <details class="original">
        <summary>原文</summary>
        <div class="original-text">{{ original }}</div>
      </details>
      <template v-if="compare">
        <div v-for="eng in engines" :key="eng" class="compare-col">
          <div class="compare-head">
            <span class="compare-tag">{{ engLabel(eng) }}</span>
            <button class="mini" :title="'复制' + engLabel(eng) + '译文'" @click="copy(compareTexts[eng] || '')">复制</button>
          </div>
          <div class="translation markdown-body" v-html="renderMd(compareTexts[eng] || '')"></div>
          <div v-if="glossaryWarns[eng]" class="glossary-warn">{{ glossaryWarns[eng] }}</div>
        </div>
      </template>
      <div v-else class="translation markdown-body" v-html="rendered"></div>
      <div v-if="!compare && glossaryWarns['']" class="glossary-warn">{{ glossaryWarns[''] }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke, langLabel } from '../shared/api'
import '../shared/base.css'
import './panel.css'

// M6b：marked/dompurify 较大，懒加载（首渲染前就绪即可，不阻塞窗口创建）
type MdRenderer = { parse: (s: string) => string; sanitize: (h: string) => string }
let md: MdRenderer | null = null
const mdReady = ref(false)
const escapeHtml = (s: string) =>
  s.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!)
void (async () => {
  try {
    const [m, d] = await Promise.all([import('marked'), import('dompurify')])
    md = {
      parse: (s) => m.marked.parse(s, { async: false }) as string,
      sanitize: (h) => d.default.sanitize(h),
    }
  } catch {
    md = { parse: escapeHtml, sanitize: (h) => h }
  }
  mdReady.value = true
})()

const original = ref('')
const translation = ref('')
const compare = ref(false)
const engines = ref<string[]>([])
const compareTexts = ref<Record<string, string>>({})
const streaming = ref(false)
const error = ref('')
const note = ref('')
const glossaryWarns = ref<Record<string, string>>({})
const outputTarget = ref('zh-CN')

const ENGINE_NAMES: Record<string, string> = {
  llm: '大模型',
  deepl: 'DeepL',
  tencent: '腾讯云',
}
const engLabel = (id: string) => ENGINE_NAMES[id] ?? id

const outputLabel = computed(() => langLabel(outputTarget.value))
const statusText = computed(() =>
  error.value
    ? '失败'
    : streaming.value
      ? '翻译中…'
      : (compare.value ? Object.values(compareTexts.value).some((t) => t) : translation.value)
        ? '完成'
        : '等待'
)

function renderMd(text: string): string {
  if (!text) return ''
  if (!md) return escapeHtml(text)
  return md.sanitize(md.parse(text))
}

const rendered = computed(() => {
  void mdReady.value
  return renderMd(translation.value)
})

onMountedHook()

async function onMountedHook() {
  await invoke('panel_ready', {})
  await listen<{ original: string; outputTarget: string; compare?: boolean; engines?: string[] }>('panel-start', (e) => {
    original.value = e.payload.original
    outputTarget.value = e.payload.outputTarget
    compare.value = !!e.payload.compare
    engines.value = e.payload.engines ?? []
    compareTexts.value = {}
    translation.value = ''
    error.value = ''
    note.value = ''
    glossaryWarns.value = {}
    streaming.value = true
  })
  await listen<{ engine?: string; text: string }>('glossary-warn', (e) => {
    // 单引擎模式事件带 engine id 但面板无对应列 → 归到 ''
    const key = compare.value && e.payload.engine ? e.payload.engine : ''
    glossaryWarns.value[key] = e.payload.text
  })
  await listen<{ note: string }>('engine-note', (e) => {
    note.value = e.payload.note
  })
  await listen<{ text: string; engine?: string }>('panel-chunk', (e) => {
    if (e.payload.engine) {
      compareTexts.value[e.payload.engine] = (compareTexts.value[e.payload.engine] || '') + e.payload.text
    } else {
      translation.value += e.payload.text
    }
  })
  await listen<{ ok: boolean; error?: string }>('panel-done', (e) => {
    streaming.value = false
    if (!e.payload.ok) error.value = e.payload.error || '翻译失败'
  })
}

function copyTranslation() {
  const text = translation.value
  if (text) invoke('copy_text', { text })
}
function copy(text: string) {
  if (text) invoke('copy_text', { text })
}
function close() {
  invoke('hide_panel')
}
function minimize() {
  invoke('minimize_panel')
}
</script>
