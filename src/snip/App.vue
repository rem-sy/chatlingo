<template>
  <div class="snip" @mousedown="onDown" @mousemove="onMove" @mouseup="onUp">
    <img v-if="imgUrl" ref="imgEl" :src="imgUrl" class="bg" draggable="false" />
    <div v-if="selecting || rect.w > 0" class="dim" :style="dimStyle"></div>
    <div v-if="rect.w > 0" class="border" :style="rectStyle"></div>
    <div v-if="!selecting && rect.w > 0" class="hint">松开即识别翻译 · Esc 取消</div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '../shared/api'

const imgUrl = ref('')
const imgEl = ref<HTMLImageElement>()
const selecting = ref(false)
const startX = ref(0)
const startY = ref(0)
const rect = ref({ x: 0, y: 0, w: 0, h: 0 })
let submitting = false

const rectStyle = computed(() => ({
  left: `${rect.value.x}px`,
  top: `${rect.value.y}px`,
  width: `${rect.value.w}px`,
  height: `${rect.value.h}px`,
}))

// 遮罩：四块矩形拼出选区外暗区
const dimStyle = computed(() => {
  const { x, y, w, h } = rect.value
  return {
    clipPath: `polygon(0 0, 100% 0, 100% 100%, 0 100%, 0 0, ${x}px ${y}px, ${x}px ${y + h}px, ${x + w}px ${y + h}px, ${x + w}px ${y}px, ${x}px ${y}px)`,
  }
})

function onDown(e: MouseEvent) {
  if (submitting) return
  selecting.value = true
  startX.value = e.clientX
  startY.value = e.clientY
  rect.value = { x: e.clientX, y: e.clientY, w: 0, h: 0 }
}

function onMove(e: MouseEvent) {
  if (!selecting.value) return
  rect.value = {
    x: Math.min(startX.value, e.clientX),
    y: Math.min(startY.value, e.clientY),
    w: Math.abs(e.clientX - startX.value),
    h: Math.abs(e.clientY - startY.value),
  }
}

async function onUp() {
  if (!selecting.value || submitting) return
  selecting.value = false
  const r = rect.value
  if (r.w < 8 || r.h < 8) {
    // 太小的选区视为误触，取消
    rect.value = { x: 0, y: 0, w: 0, h: 0 }
    return
  }
  submitting = true
  try {
    const img = imgEl.value!
    // 显示坐标 → 图片原始像素
    const sx = img.naturalWidth / img.width
    const sy = img.naturalHeight / img.height
    const canvas = document.createElement('canvas')
    canvas.width = Math.round(r.w * sx)
    canvas.height = Math.round(r.h * sy)
    const ctx = canvas.getContext('2d')!
    ctx.drawImage(img, r.x * sx, r.y * sy, r.w * sx, r.h * sy, 0, 0, canvas.width, canvas.height)
    const dataUrl = canvas.toDataURL('image/png')
    rect.value = { x: 0, y: 0, w: 0, h: 0 }
    await invoke('submit_screenshot', { image: dataUrl })
  } catch (e) {
    console.error(e)
  } finally {
    submitting = false
  }
}

onMounted(async () => {
  await listen<{ image: string }>('snip-image', (e) => {
    imgUrl.value = e.payload.image
  })
  window.addEventListener('keydown', (ev) => {
    if (ev.key === 'Escape') {
      rect.value = { x: 0, y: 0, w: 0, h: 0 }
      imgUrl.value = ''
      invoke('cancel_screenshot')
    }
  })
})
</script>

<style scoped>
.snip {
  position: fixed;
  inset: 0;
}
.bg {
  position: absolute;
  inset: 0;
  width: 100vw;
  height: 100vh;
}
.dim {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.35);
}
.border {
  position: absolute;
  border: 1.5px solid #4f8cff;
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.4);
}
.hint {
  position: fixed;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%);
  background: rgba(30, 31, 36, 0.9);
  color: #e8e9ed;
  font-size: 13px;
  padding: 6px 16px;
  border-radius: 999px;
  border: 1px solid #3a3b45;
}
</style>
