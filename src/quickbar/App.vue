<template>
  <div class="quickbar" :class="{ translating }">
    <div class="input-zone">
      <textarea
        ref="inputEl"
        v-model="source"
        :placeholder="`输入中文，回车翻译为 ${targetLabel}…`"
        rows="3"
        @keydown="onKeydown"
      ></textarea>
    </div>
    <div v-if="translated || streaming || error" class="divider"></div>
    <div v-if="error" class="error">{{ error }}</div>
    <div v-else-if="note" class="note">{{ note }}</div>
    <div v-else-if="translated || streaming" class="output-zone">
      <div class="output-text" :class="{ streaming }">{{ display }}</div>
    </div>
    <div class="toolbar">
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
      <div class="status">{{ statusText }}</div>
      <div class="actions">
        <button v-if="translated && !translating" @click="copy">复制</button>
        <button v-if="translated && !translating" class="primary" @click="paste">粘贴回填</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke, type AppConfig, LANG_LABELS } from '../shared/api'
import '../shared/base.css'
import './quickbar.css'

const inputEl = ref<HTMLTextAreaElement>()
const source = ref('')
const translated = ref('')
const streaming = ref(false)
const translating = ref(false)
const error = ref('')
const target = ref('en')
const langs = ref<string[]>(['en', 'ja'])
const cfg = ref<AppConfig | null>(null)
const note = ref('')
let reqId = 0

const label = (l: string) => LANG_LABELS[l] ?? l
const targetLabel = computed(() => label(target.value))
const display = computed(() => translated.value || '')
const statusText = computed(() => {
  if (translating.value) return `翻译中… → ${targetLabel.value}`
  if (error.value) return '失败'
  if (translated.value) return `已译为 ${targetLabel.value}${autoDone.value ? ' · 已回填' : ''}`
  return ''
})
const autoDone = ref(false)

onMounted(async () => {
  inputEl.value?.focus()
  cfg.value = await invoke<AppConfig>('get_config')
  langs.value = cfg.value.quickbar.targetLangs
  target.value = cfg.value.quickbar.defaultTarget
  await listen<{ id: number; text: string }>('quickbar-chunk', (e) => {
    if (e.payload.id !== reqId) return
    streaming.value = true
    translated.value += e.payload.text
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
    translating.value = false
    streaming.value = false
    autoDone.value = false
    inputEl.value?.focus()
  })
  await listen<{ note: string }>('engine-note', (e) => {
    note.value = e.payload.note
  })
})

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
    e.preventDefault()
    submit()
  } else if (e.key === 'Escape') {
    invoke('hide_quickbar')
  }
}

async function submit() {
  const text = source.value.trim()
  if (!text || translating.value) return
  translating.value = true
  streaming.value = false
  translated.value = ''
  error.value = ''
  autoDone.value = false
  reqId += 1
  await invoke('submit_input', { text, target: target.value, id: reqId })
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
