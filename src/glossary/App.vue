<template>
  <div class="glossary">
    <div class="toolbar">
      <input v-model="query" type="text" placeholder="搜索原文、译法或备注…" @input="debouncedLoad" />
      <span class="count">{{ entries.length }} 条</span>
      <button @click="startAdd">＋ 新增</button>
      <label class="import-btn">
        导入<input type="file" accept=".json,.csv" @change="importFile" hidden />
      </label>
      <button v-if="entries.length" @click="exportJson">导出</button>
      <button v-if="entries.length" class="danger" @click="clearAll">清空</button>
    </div>

    <div class="edit-row" v-if="editing">
      <input v-model="editing.source" type="text" placeholder="原文术语（如 RAG）" />
      <span class="arrow">→</span>
      <input v-model="editing.target" type="text" placeholder="固定译法（如 检索增强生成）" />
      <input v-model="editing.note" type="text" placeholder="备注（可选）" class="note" />
      <label class="chk"><input type="checkbox" v-model="editing.enabled" /> 启用</label>
      <button class="primary" @click="saveEdit" :disabled="saving">{{ saving ? '保存中…' : '保存' }}</button>
      <button @click="editing = null">取消</button>
    </div>
    <div class="edit-row err" v-if="editErr">{{ editErr }}</div>

    <div class="list">
      <div v-if="!entries.length && !loading" class="empty">
        暂无术语。翻译时命中的术语将按固定译法注入所有引擎。
      </div>
      <div v-for="e in entries" :key="e.id" class="entry" :class="{ off: !e.enabled, conflict: conflicts.has(e.id) }">
        <div class="row">
          <input type="checkbox" :checked="e.enabled" @change="toggle(e)" title="启用/停用" />
          <span class="source">{{ e.source }}</span>
          <span class="arrow">→</span>
          <span class="target">{{ e.target }}</span>
          <span class="note" v-if="e.note">{{ e.note }}</span>
          <div class="spacer"></div>
          <button class="mini" @click="startEdit(e)">编辑</button>
          <button class="mini danger" @click="del(e)">删除</button>
        </div>
        <div class="conflict-tip" v-if="conflicts.has(e.id)">⚠ 同一原文存在多条不同译法，翻译时取最先创建的一条</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke, type GlossaryEntry } from '../shared/api'
import '../shared/base.css'
import './glossary.css'

const entries = ref<GlossaryEntry[]>([])
const query = ref('')
const loading = ref(false)
const editing = ref<GlossaryEntry | null>(null)
const editErr = ref('')
const saving = ref(false)
let timer: number | undefined

// 冲突检测：同 source 且 target 不同 → 标黄
const conflicts = computed(() => {
  const bad = new Set<number>()
  const seen = new Map<string, { id: number; target: string }>()
  for (const e of entries.value) {
    const key = e.source.toLowerCase()
    const prev = seen.get(key)
    if (prev && prev.target !== e.target) {
      bad.add(prev.id)
      bad.add(e.id)
    } else if (!prev) {
      seen.set(key, { id: e.id, target: e.target })
    }
  }
  return bad
})

async function load() {
  loading.value = true
  try {
    entries.value = await invoke<GlossaryEntry[]>('glossary_list', { query: query.value })
  } finally {
    loading.value = false
  }
}

function debouncedLoad() {
  if (timer) clearTimeout(timer)
  timer = window.setTimeout(load, 300)
}

function startAdd() {
  editErr.value = ''
  editing.value = { id: 0, source: '', target: '', note: '', enabled: true, createdAt: 0 }
}

function startEdit(e: GlossaryEntry) {
  editErr.value = ''
  editing.value = { ...e }
}

async function saveEdit() {
  const e = editing.value
  if (!e) return
  if (!e.source.trim() || !e.target.trim()) {
    editErr.value = '原文与译法均不能为空'
    return
  }
  saving.value = true
  try {
    await invoke('glossary_upsert', { entry: e })
    editing.value = null
    editErr.value = ''
    await load()
  } catch (err) {
    editErr.value = String(err)
  } finally {
    saving.value = false
  }
}

async function toggle(e: GlossaryEntry) {
  await invoke('glossary_upsert', { entry: { ...e, enabled: !e.enabled } })
  await load()
}

async function del(e: GlossaryEntry) {
  if (!confirm(`删除术语「${e.source} → ${e.target}」？`)) return
  await invoke('glossary_delete', { id: e.id })
  await load()
}

async function clearAll() {
  if (!confirm('确定清空全部术语？')) return
  await invoke('glossary_clear')
  await load()
}

async function exportJson() {
  const json = await invoke<string>('glossary_export')
  const blob = new Blob([json], { type: 'application/json;charset=utf-8' })
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = `chatlingo-glossary-${new Date().toISOString().slice(0, 10)}.json`
  a.click()
  URL.revokeObjectURL(a.href)
}

async function importFile(ev: Event) {
  const input = ev.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  const content = await file.text()
  try {
    const res = await invoke<{ imported: number; skipped: number }>('glossary_import', { content })
    alert(`导入完成：${res.imported} 条生效，${res.skipped} 条跳过`)
    await load()
  } catch (e) {
    alert(`导入失败：${e}`)
  }
}

onMounted(async () => {
  await load()
  await listen('glossary-refresh', load)
})
</script>
