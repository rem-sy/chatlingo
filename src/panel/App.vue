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
        <div class="compare-col">
          <div class="compare-head">
            <span class="compare-tag llm">大模型</span>
            <button class="mini" title="复制大模型译文" @click="copy(llmText)">复制</button>
          </div>
          <div class="translation markdown-body" v-html="renderMd(llmText)"></div>
        </div>
        <div class="compare-col">
          <div class="compare-head">
            <span class="compare-tag deepl">DeepL</span>
            <button class="mini" title="复制 DeepL 译文" @click="copy(deeplText)">复制</button>
          </div>
          <div class="translation plain">{{ deeplText }}</div>
        </div>
      </template>
      <div v-else class="translation markdown-body" v-html="rendered"></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke, LANG_LABELS } from '../shared/api'
import { marked } from 'marked'
import DOMPurify from 'dompurify'
import '../shared/base.css'
import './panel.css'

const original = ref('')
const translation = ref('')
const llmText = ref('')
const deeplText = ref('')
const compare = ref(false)
const streaming = ref(false)
const error = ref('')
const note = ref('')
const outputTarget = ref('zh-CN')

const outputLabel = computed(() => LANG_LABELS[outputTarget.value] ?? outputTarget.value)
const statusText = computed(() =>
  error.value ? '失败' : streaming.value ? '翻译中…' : (compare.value ? llmText.value : translation.value) ? '完成' : '等待'
)

function renderMd(text: string): string {
  if (!text) return ''
  const html = marked.parse(text, { async: false }) as string
  return DOMPurify.sanitize(html)
}

const rendered = computed(() => renderMd(translation.value))

onMountedHook()

async function onMountedHook() {
  await invoke('panel_ready', {})
  await listen<{ original: string; outputTarget: string; compare?: boolean }>('panel-start', (e) => {
    original.value = e.payload.original
    outputTarget.value = e.payload.outputTarget
    compare.value = !!e.payload.compare
    translation.value = ''
    llmText.value = ''
    deeplText.value = ''
    error.value = ''
    note.value = ''
    streaming.value = true
  })
  await listen<{ note: string }>('engine-note', (e) => {
    note.value = e.payload.note
  })
  await listen<{ text: string; engine?: string }>('panel-chunk', (e) => {
    if (e.payload.engine === 'llm') {
      llmText.value += e.payload.text
    } else if (e.payload.engine === 'deepl') {
      deeplText.value += e.payload.text
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
