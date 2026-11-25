<template>
  <main class="panel">
    <header class="panel-head" data-tauri-drag-region>
      <h1 data-tauri-drag-region>剪贴板翻译</h1>
      <span class="chip language" data-tauri-drag-region>{{ outputLabel }}</span>
      <div class="spacer" data-tauri-drag-region></div>
      <button v-if="!compare" class="quiet header-copy" :disabled="!translation" @click="copy(translation, 'single')">{{ copied === 'single' ? '已复制' : '复制译文' }}</button>
      <button class="quiet window-control" aria-label="最小化" title="最小化" @click="minimize">−</button>
      <button class="quiet window-control" aria-label="关闭" title="关闭" @click="close">×</button>
    </header>
    <div ref="bodyEl" class="panel-body">
      <div v-if="streaming" class="stream-status" role="status"><span class="stream-dot"></span>翻译中…</div>
      <div v-if="note" class="feedback warning" role="status">{{ note }}</div>
      <div v-if="error" class="feedback" :class="hasResult ? 'warning' : 'error'" role="alert">{{ error }}</div>
      <div v-if="copyError" class="feedback error" role="alert">{{ copyError }}</div>
      <details v-if="original" :key="original" class="original">
        <summary>查看原文</summary>
        <div class="original-text" dir="auto">{{ original }}</div>
      </details>
      <div v-if="compare" class="compare-grid" :class="{ 'three-engines': engines.length > 2 }">
        <article v-for="eng in engines" :key="eng" class="compare-col">
          <header class="compare-head">
            <span class="chip">{{ engineName(eng) }}</span>
            <button class="quiet" :disabled="!compareTexts[eng]" :aria-label="'复制' + engineName(eng) + '译文'" @click="copy(compareTexts[eng], eng)">{{ copied === eng ? '已复制' : '复制' }}</button>
          </header>
          <div v-if="compareTexts[eng]" class="translation" :class="{ plain: !md }" dir="auto" v-html="renderMd(compareTexts[eng])"></div>
          <p v-else class="result-placeholder">{{ streaming ? '等待译文…' : '未返回译文' }}</p>
          <div v-if="glossaryWarns[eng]" class="feedback warning">{{ glossaryWarns[eng] }}</div>
        </article>
      </div>
      <div v-else-if="translation" class="translation single" :class="{ plain: !md }" dir="auto" v-html="renderMd(translation)"></div>
      <div v-else-if="!streaming && !error" class="empty-state"><strong>等待剪贴板内容</strong><p>复制外文文本后，译文将在这里显示。</p></div>
      <div v-if="!compare && glossaryWarns['']" class="feedback warning">{{ glossaryWarns[''] }}</div>
    </div>
  </main>
</template>

<script setup lang="ts">
import { ref, computed, shallowRef, onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke, engineName, langDef } from '../shared/api'
import '../shared/base.css'
import './panel.css'

// M6b：marked/dompurify 较大，懒加载（首渲染前就绪即可，不阻塞窗口创建）
type MdRenderer = { parse: (s: string) => string; sanitize: (h: string) => string }
const md = shallowRef<MdRenderer>()
const escapeHtml = (s: string) =>
  s.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!)
void (async () => {
  try {
    const [m, d] = await Promise.all([import('marked'), import('dompurify')])
    md.value = {
      parse: (s) => m.marked.parse(s, { async: false }) as string,
      sanitize: (h) => d.default.sanitize(h),
    }
  } catch { /* 解析器加载失败时继续显示纯文本。 */ }
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
const bodyEl = ref<HTMLElement>()
const copied = ref('')
const copyError = ref('')
let copyTimer: number | undefined

const outputLabel = computed(() => langDef(outputTarget.value).name)
const hasResult = computed(() => Boolean(translation.value) || Object.values(compareTexts.value).some(Boolean))

function renderMd(text: string): string {
  const renderer = md.value
  return renderer ? renderer.sanitize(renderer.parse(text)) : escapeHtml(text)
}

onMounted(async () => {
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
    copied.value = ''
    copyError.value = ''
    clearTimeout(copyTimer)
    streaming.value = true
    bodyEl.value?.scrollTo(0, 0)
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
  await listen<{ ok: boolean; error?: string | null }>('panel-done', (e) => {
    streaming.value = false
    error.value = e.payload.error || (e.payload.ok ? '' : '翻译失败')
  })
  await invoke('panel_ready', {})
})

async function copy(text: string, key: string) {
  if (!text) return
  copyError.value = ''
  try {
    await invoke('copy_text', { text })
    copied.value = key
    clearTimeout(copyTimer)
    copyTimer = window.setTimeout(() => { copied.value = '' }, 1600)
  } catch (e) {
    copyError.value = `复制失败：${e}`
  }
}
function close() {
  invoke('hide_panel')
}
function minimize() {
  invoke('minimize_panel')
}
</script>
