<template>
  <main class="library">
    <header class="library-head">
      <div class="library-heading">
        <h1 class="view-title">术语库</h1>
        <span class="chip" role="status">{{ loading ? '加载中…' : entries.length + ' 条术语' }}</span>
      </div>
      <div class="library-toolbar">
        <input v-model="query" type="search" aria-label="搜索术语" placeholder="搜索原文、译法或备注" />
        <button :disabled="busy" title="导入 JSON 或 CSV" @click="fileEl?.click()">导入</button>
        <input ref="fileEl" type="file" accept=".json,.csv" @change="importFile" hidden />
        <button :disabled="busy || loading || !entries.length" @click="exportJson">导出</button>
        <button class="danger" :disabled="busy || loading || !entries.length" @click="clearAll">清空</button>
        <button class="primary" :disabled="busy" @click="startAdd">新增术语</button>
      </div>
    </header>
    <div class="library-body">
      <div class="library-inner">
        <div v-if="message" class="feedback" :class="messageKind" :role="messageKind === 'error' ? 'alert' : 'status'">{{ message }}</div>
        <form v-if="editing" class="library-entry editor" @submit.prevent="saveEdit" @keydown.esc.prevent="cancelEdit">
          <h2>{{ editing.id ? '编辑术语' : '新增术语' }}</h2>
          <div class="editor-fields">
            <label>原文<input ref="termInput" v-model="editing.source" type="text" placeholder="如 RAG" required /></label>
            <label>固定译法<input v-model="editing.target" type="text" placeholder="如 检索增强生成" required /></label>
            <label class="editor-note">备注（可选）<input v-model="editing.note" type="text" /></label>
          </div>
          <div v-if="editErr" class="feedback error" role="alert">{{ editErr }}</div>
          <div class="editor-footer">
            <label><input v-model="editing.enabled" type="checkbox" />启用术语</label>
            <button type="button" :disabled="busy" @click="cancelEdit">取消</button>
            <button class="primary" type="submit" :disabled="busy">{{ busy ? '保存中…' : '保存术语' }}</button>
          </div>
        </form>
        <div class="library-list" :aria-busy="loading">
          <div v-if="!entries.length && !loading && !loadError" class="empty-state">
            <strong>{{ query.trim() ? '未找到匹配的术语' : '暂无术语' }}</strong>
            <p>{{ query.trim() ? '试试其他关键词。' : '新增或导入术语，为专有名词指定固定译法。' }}</p>
          </div>
          <article v-for="entry in entries" :key="entry.id" class="library-entry term" :class="{ conflict: conflicts.has(entry.id) }">
            <div class="term-row">
              <input type="checkbox" :checked="entry.enabled" :disabled="busy" :aria-label="'启用术语 ' + entry.source" @change="toggle(entry, $event)" />
              <div class="term-content">
                <div class="term-pair">
                  <strong dir="auto">{{ entry.source }}</strong><span class="arrow" aria-hidden="true">→</span><span dir="auto">{{ entry.target }}</span>
                </div>
                <div v-if="entry.note" class="term-note" dir="auto">{{ entry.note }}</div>
              </div>
              <div class="actions">
                <button class="quiet" :disabled="busy" @click="startEdit(entry)">编辑</button>
                <button class="quiet danger" :disabled="busy" @click="del(entry)">删除</button>
              </div>
            </div>
            <div v-if="conflicts.has(entry.id)" class="feedback warning">同一原文对应多个译法。请确认要启用的译法。</div>
          </article>
        </div>
      </div>
    </div>
  </main>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, onMounted, watch } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke, type GlossaryEntry } from '../shared/api'
import '../shared/base.css'
import '../shared/library.css'
import './glossary.css'

const entries = ref<GlossaryEntry[]>([])
const query = ref('')
const loading = ref(true)
const loadError = ref(false)
const editing = ref<GlossaryEntry | null>(null)
const editErr = ref('')
const busy = ref(false)
const message = ref('')
const messageKind = ref<'error' | 'success'>('success')
const termInput = ref<HTMLInputElement>()
const fileEl = ref<HTMLInputElement>()
let timer: number | undefined
let loadId = 0

// 冲突检测：同 source 且 target 不同 → 标黄
const conflicts = computed(() => {
  const targets = new Map<string, Set<string>>()
  for (const entry of entries.value) {
    const source = entry.source.toLowerCase()
    const group = targets.get(source) ?? new Set<string>()
    group.add(entry.target)
    targets.set(source, group)
  }
  return new Set(entries.value
    .filter((entry) => (targets.get(entry.source.toLowerCase())?.size ?? 0) > 1)
    .map((entry) => entry.id))
})

async function load() {
  const id = ++loadId
  const requestedQuery = query.value
  loading.value = true
  loadError.value = false
  try {
    const found = await invoke<GlossaryEntry[]>('glossary_list', { query: requestedQuery })
    if (id === loadId && requestedQuery === query.value) entries.value = found
  } catch (e) {
    if (id !== loadId || requestedQuery !== query.value) return
    loadError.value = true
    report(`无法读取术语：${e}`, 'error')
  } finally {
    if (id === loadId && requestedQuery === query.value) loading.value = false
  }
}

watch(query, () => {
  loading.value = true
  message.value = ''
  clearTimeout(timer)
  timer = window.setTimeout(load, 200)
})

function report(text: string, kind: 'error' | 'success') {
  message.value = text
  messageKind.value = kind
}

function startAdd() {
  startEdit({ id: 0, source: '', target: '', note: '', enabled: true, createdAt: 0 })
}

async function startEdit(e: GlossaryEntry) {
  editErr.value = ''
  editing.value = { ...e }
  await nextTick()
  termInput.value?.focus()
}

function cancelEdit() {
  if (!busy.value) editing.value = null
}

async function saveEdit() {
  const e = editing.value
  if (!e || busy.value) return
  if (!e.source.trim() || !e.target.trim()) {
    editErr.value = '原文与译法均不能为空'
    return
  }
  busy.value = true
  message.value = ''
  try {
    await invoke('glossary_upsert', { entry: e })
    editing.value = null
    editErr.value = ''
    await load()
  } catch (err) {
    editErr.value = String(err)
  } finally {
    busy.value = false
  }
}

async function toggle(entry: GlossaryEntry, event: Event) {
  const checkbox = event.currentTarget as HTMLInputElement
  const enabled = checkbox.checked
  checkbox.checked = entry.enabled
  await updateEntries('glossary_upsert', { entry: { ...entry, enabled } }, '更新')
}

async function del(entry: GlossaryEntry) {
  if (!confirm(`删除术语「${entry.source} → ${entry.target}」？`)) return
  if (await updateEntries('glossary_delete', { id: entry.id }, '删除') && editing.value?.id === entry.id) editing.value = null
}

async function clearAll() {
  if (!confirm('确定清空全部术语？')) return
  if (await updateEntries('glossary_clear', {}, '清空')) editing.value = null
}

async function updateEntries(command: string, args: Record<string, unknown>, action: string) {
  if (busy.value) return false
  busy.value = true
  message.value = ''
  try {
    await invoke(command, args)
    await load()
    return true
  } catch (e) {
    report(`${action}失败：${e}`, 'error')
    return false
  } finally {
    busy.value = false
  }
}

async function exportJson() {
  try {
    const json = await invoke<string>('glossary_export')
    const blob = new Blob([json], { type: 'application/json;charset=utf-8' })
    const a = document.createElement('a')
    a.href = URL.createObjectURL(blob)
    a.download = `chatlingo-glossary-${new Date().toISOString().slice(0, 10)}.json`
    a.click()
    setTimeout(() => URL.revokeObjectURL(a.href), 1000)
  } catch (e) {
    report(`导出失败：${e}`, 'error')
  }
}

async function importFile(ev: Event) {
  const input = ev.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file || busy.value) return
  busy.value = true
  try {
    const content = await file.text()
    const res = await invoke<{ imported: number; skipped: number }>('glossary_import', { content })
    report(`导入完成：${res.imported} 条生效，${res.skipped} 条跳过`, 'success')
    await load()
  } catch (e) {
    report(`导入失败：${e}`, 'error')
  } finally {
    busy.value = false
  }
}

onMounted(async () => {
  await listen('glossary-refresh', load)
  await load()
})
</script>
