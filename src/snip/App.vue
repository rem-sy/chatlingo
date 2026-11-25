<template>
  <div
    class="snip"
    @pointerdown="onDown"
    @pointermove="onMove"
    @pointerup="onUp"
    @pointercancel="clearSelection"
  >
    <img v-if="imgUrl" ref="imgEl" :src="imgUrl" class="capture" alt="" draggable="false" />
    <div v-if="imgUrl" class="dim" :style="dimStyle"></div>
    <div v-if="rect.w && rect.h" class="selection" :style="rectStyle"></div>
    <div class="capture-hint" @pointerdown.stop>
      <span v-if="selecting" aria-hidden="true">{{ dimensions }}</span>
      <span v-else role="status">{{ error || (imgUrl ? '拖动选取需要识别的文字区域' : '正在载入屏幕…') }}</span>
      <button class="quiet" aria-label="取消截图" @click="cancel">取消 <kbd>Esc</kbd></button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '../shared/api'
import '../shared/base.css'

const imgUrl = ref('')
const imgEl = ref<HTMLImageElement>()
const selecting = ref(false)
const error = ref('')
const rect = ref({ x: 0, y: 0, w: 0, h: 0 })
let startX = 0
let startY = 0
let pointerId: number | null = null
let submitting = false

const rectStyle = computed(() => ({
  left: `${rect.value.x}px`,
  top: `${rect.value.y}px`,
  width: `${rect.value.w}px`,
  height: `${rect.value.h}px`,
}))
const dimensions = computed(() => {
  const img = imgEl.value
  const sx = img?.width ? img.naturalWidth / img.width : 1
  const sy = img?.height ? img.naturalHeight / img.height : 1
  return `${Math.round(rect.value.w * sx)} × ${Math.round(rect.value.h * sy)} px`
})
const dimStyle = computed(() => {
  const { x, y, w, h } = rect.value
  return {
    clipPath: `polygon(evenodd, 0 0, 100% 0, 100% 100%, 0 100%, 0 0, ${x}px ${y}px, ${x}px ${y + h}px, ${x + w}px ${y + h}px, ${x + w}px ${y}px, ${x}px ${y}px)`,
  }
})

function onDown(event: PointerEvent) {
  if (!event.isPrimary || event.button !== 0 || submitting || !imgEl.value?.naturalWidth) return
  startX = event.clientX
  startY = event.clientY
  pointerId = event.pointerId
  selecting.value = true
  error.value = ''
  rect.value = { x: startX, y: startY, w: 0, h: 0 }
  const overlay = event.currentTarget as HTMLElement
  overlay.setPointerCapture(pointerId)
}

function onMove(event: PointerEvent) {
  if (event.pointerId !== pointerId) return
  const x = Math.max(0, Math.min(event.clientX, window.innerWidth))
  const y = Math.max(0, Math.min(event.clientY, window.innerHeight))
  rect.value = {
    x: Math.min(startX, x),
    y: Math.min(startY, y),
    w: Math.abs(x - startX),
    h: Math.abs(y - startY),
  }
}

function clearSelection() {
  pointerId = null
  selecting.value = false
  rect.value = { x: 0, y: 0, w: 0, h: 0 }
}

async function onUp(event: PointerEvent) {
  if (event.pointerId !== pointerId) return
  onMove(event)
  const { x, y, w, h } = rect.value
  clearSelection()
  const img = imgEl.value
  if (!img || w < 8 || h < 8) return

  submitting = true
  try {
    const sx = img.naturalWidth / img.width
    const sy = img.naturalHeight / img.height
    const canvas = document.createElement('canvas')
    canvas.width = Math.round(w * sx)
    canvas.height = Math.round(h * sy)
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('无法读取截图')
    ctx.drawImage(img, x * sx, y * sy, w * sx, h * sy, 0, 0, canvas.width, canvas.height)
    await invoke('submit_screenshot', { image: canvas.toDataURL('image/png') })
  } catch (e) {
    error.value = `识别失败：${e}`
  } finally {
    submitting = false
  }
}

function cancel() {
  clearSelection()
  imgUrl.value = ''
  error.value = ''
  void invoke('cancel_screenshot')
}

onMounted(async () => {
  window.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') cancel()
  })
  await listen<{ image: string }>('snip-image', (event) => {
    clearSelection()
    error.value = ''
    imgUrl.value = event.payload.image
  })
})
</script>

<style scoped>
.snip { position: fixed; inset: 0; background: transparent; touch-action: none; }
.capture, .dim { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
.dim { background: #100b27aa; }
.selection {
  position: absolute;
  border: 2px solid #c9bdff;
  box-shadow: 0 0 0 1px #33216e;
  pointer-events: none;
}
.capture-hint {
  position: absolute;
  left: 50%;
  bottom: 24px;
  display: flex;
  align-items: center;
  gap: 10px;
  width: max-content;
  max-width: calc(100vw - 24px);
  min-height: 42px;
  padding: 5px 7px 5px 13px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface);
  box-shadow: 0 4px 14px #100b2733;
  transform: translateX(-50%);
  color: var(--text);
  font-size: 13px;
}
.capture-hint button { cursor: pointer; white-space: nowrap; }
.capture-hint span { overflow-wrap: anywhere; font-variant-numeric: tabular-nums; }
</style>

<style>
html, body { background: transparent; }
</style>
