<template>
  <div class="panel">
    <div class="panel-head">
      <span class="badge">Agent 输出 → {{ outputLabel }}</span>
      <span class="status">{{ statusText }}</span>
      <div class="spacer"></div>
      <button class="mini" title="复制译文" @click="copyTranslation">复制</button>
      <button class="mini" title="关闭" @click="close">×</button>
    </div>
    <div class="panel-body">
      <details class="original">
        <summary>原文</summary>
        <div class="original-text">{{ original }}</div>
      </details>
      <div class="translation markdown-body" v-html="rendered"></div>
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
const streaming = ref(false)
const error = ref('')
const outputTarget = ref('zh-CN')

const outputLabel = computed(() => LANG_LABELS[outputTarget.value] ?? outputTarget.value)
const statusText = computed(() =>
  error.value ? '失败' : streaming.value ? '翻译中…' : translation.value ? '完成' : '等待'
)

const rendered = computed(() => {
  if (!translation.value) return ''
  const html = marked.parse(translation.value, { async: false }) as string
  return DOMPurify.sanitize(html)
})

onMountedHook()

async function onMountedHook() {
  await invoke('panel_ready', {})
  await listen<{ original: string; outputTarget: string }>('panel-start', (e) => {
    original.value = e.payload.original
    outputTarget.value = e.payload.outputTarget
    translation.value = ''
    error.value = ''
    streaming.value = true
  })
  await listen<{ text: string }>('panel-chunk', (e) => {
    translation.value += e.payload.text
  })
  await listen<{ ok: boolean; error?: string }>('panel-done', (e) => {
    streaming.value = false
    if (!e.payload.ok) error.value = e.payload.error || '翻译失败'
  })
}

function copyTranslation() {
  if (translation.value) invoke('copy_text', { text: translation.value })
}
function close() {
  invoke('hide_panel')
}
</script>
