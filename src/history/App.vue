<template>
  <div class="history">
    <div class="toolbar">
      <input v-model="query" type="text" placeholder="搜索原文或译文…" @input="debouncedLoad" />
      <span class="count">{{ entries.length }} 条</span>
      <button v-if="entries.length" @click="clearAll">清空全部</button>
    </div>
    <div class="list">
      <div v-if="!entries.length && !loading" class="empty">暂无历史记录</div>
      <div v-for="e in entries" :key="e.id" class="entry">
        <div class="entry-head">
          <span class="dir" :class="e.direction">{{ e.direction === 'input' ? '输入 → 外文' : 'Agent 输出 → 中文' }}</span>
          <span class="engine">{{ e.engine }}</span>
          <span class="time">{{ fmt(e.ts) }}</span>
          <div class="spacer"></div>
          <button class="mini" @click="copy(e.translated)">复制译文</button>
          <button class="mini" @click="copy(e.sourceText)">复制原文</button>
        </div>
        <div class="source">{{ e.sourceText }}</div>
        <div class="target">{{ e.translated }}</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '../shared/api'
import '../shared/base.css'
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
const loading = ref(false)
let timer: number | undefined

async function load() {
  loading.value = true
  try {
    entries.value = await invoke<Entry[]>('history_list', { query: query.value })
  } finally {
    loading.value = false
  }
}

function debouncedLoad() {
  if (timer) clearTimeout(timer)
  timer = window.setTimeout(load, 300)
}

function fmt(ts: number): string {
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getMonth() + 1}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

function copy(text: string) {
  invoke('copy_text', { text })
}

async function clearAll() {
  if (!confirm('确定清空全部翻译历史？')) return
  await invoke('history_clear')
  await load()
}

onMounted(async () => {
  await load()
  await listen('history-refresh', load)
})
</script>
