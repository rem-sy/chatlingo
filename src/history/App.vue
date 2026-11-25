<template>
  <main class="library">
    <header class="library-head">
      <div class="library-heading">
        <h1 class="view-title">翻译历史</h1>
        <span class="chip" role="status">{{ loading ? '加载中…' : entries.length + ' 条记录' }}</span>
      </div>
      <div class="library-toolbar">
        <input v-model="query" type="search" aria-label="搜索翻译历史" placeholder="搜索原文或译文" />
        <button class="danger" :disabled="!entries.length || loading || clearing" @click="clearAll">{{ clearing ? '清空中…' : '清空历史' }}</button>
      </div>
    </header>
    <div class="library-body">
      <div class="library-inner">
        <div v-if="error" class="feedback error" role="alert">{{ error }}</div>
        <div class="library-list" :aria-busy="loading">
          <div v-if="!entries.length && !loading && !loadError" class="empty-state">
            <strong>{{ query.trim() ? '未找到匹配的记录' : '暂无翻译历史' }}</strong>
            <p>{{ query.trim() ? '试试其他关键词。' : '完成翻译后可在这里查看原文和译文。' }}</p>
          </div>
          <article v-for="entry in entries" :key="entry.id" class="library-entry">
            <header class="entry-head">
              <div class="entry-meta">
                <span class="chip">{{ entry.direction === 'input' ? '快速翻译' : '剪贴板翻译' }}</span>
                <span v-if="entry.engine">{{ engineName(entry.engine) }}</span>
                <span>{{ fmt(entry.ts) }}</span>
              </div>
              <div class="actions">
                <button class="quiet" @click="copy(entry.sourceText, entry.id + ':source')">{{ copied === entry.id + ':source' ? '已复制' : '复制原文' }}</button>
                <button class="quiet" @click="copy(entry.translated, entry.id + ':target')">{{ copied === entry.id + ':target' ? '已复制' : '复制译文' }}</button>
              </div>
            </header>
            <dl class="entry-text">
              <div><dt>原文</dt><dd dir="auto">{{ entry.sourceText }}</dd></div>
              <div><dt>译文</dt><dd dir="auto">{{ entry.translated }}</dd></div>
            </dl>
          </article>
        </div>
      </div>
    </div>
  </main>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke, engineName } from '../shared/api'
import '../shared/base.css'
import '../shared/library.css'
import './history.css'

interface Entry {
  id: number
  ts: number
  direction: string
  sourceText: string
  translated: string
  engine: string
}

const entries = ref<Entry[]>([])
const query = ref('')
const loading = ref(true)
const clearing = ref(false)
const error = ref('')
const loadError = ref(false)
const copied = ref('')
let timer: number | undefined
let copyTimer: number | undefined
let loadId = 0

async function load() {
  const id = ++loadId
  const requestedQuery = query.value
  loading.value = true
  loadError.value = false
  error.value = ''
  try {
    const found = await invoke<Entry[]>('history_list', { query: requestedQuery })
    if (id === loadId && requestedQuery === query.value) entries.value = found
  } catch (e) {
    if (id !== loadId || requestedQuery !== query.value) return
    loadError.value = true
    error.value = `无法读取翻译历史：${e}`
  } finally {
    if (id === loadId && requestedQuery === query.value) loading.value = false
  }
}

watch(query, () => {
  loading.value = true
  clearTimeout(timer)
  timer = window.setTimeout(load, 200)
})

function fmt(ts: number): string {
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getMonth() + 1}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

async function copy(text: string, key: string) {
  error.value = ''
  try {
    await invoke('copy_text', { text })
    copied.value = key
    clearTimeout(copyTimer)
    copyTimer = window.setTimeout(() => { copied.value = '' }, 1600)
  } catch (e) {
    error.value = `复制失败：${e}`
  }
}

async function clearAll() {
  if (clearing.value || !confirm('确定清空全部翻译历史？')) return
  clearing.value = true
  error.value = ''
  try {
    await invoke('history_clear')
    await load()
  } catch (e) {
    error.value = `清空失败：${e}`
  } finally {
    clearing.value = false
  }
}

onMounted(async () => {
  await listen('history-refresh', load)
  await load()
})
</script>
