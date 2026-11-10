<script setup lang="ts">
import { Globe } from '@lucide/vue'
import type { EdgeSide, EdgeSnapshot } from '../composables/useEdgeMode'

const props = defineProps<{
  side: EdgeSide | null
  readTab: () => Promise<EdgeSnapshot | null>
  moveTab: (tab: EdgeSnapshot, delta: number) => Promise<void>
  saveTab: (tab: EdgeSnapshot) => Promise<void>
}>()
const emit = defineEmits<{ expand: [] }>()

interface Gesture {
  id: number
  x: number
  y: number
  delta: number
  sent: number
  dragged: boolean
  sending: boolean
  cancelled: boolean
  origin: Promise<EdgeSnapshot | null>
}

let gesture: Gesture | undefined
let suppressClick = false
let queue = Promise.resolve()

function press(event: PointerEvent): void {
  if (event.button !== 0 || !event.isPrimary) return
  const button = event.currentTarget as HTMLButtonElement
  button.setPointerCapture(event.pointerId)
  suppressClick = false
  gesture = {
    id: event.pointerId, x: event.screenX, y: event.screenY,
    delta: 0, sent: 0, dragged: false, sending: false, cancelled: false,
    origin: queue.then(() => props.readTab()).catch(() => null),
  }
}

function move(event: PointerEvent): void {
  const current = gesture
  if (!current || current.id !== event.pointerId) return
  const dx = event.screenX - current.x
  const dy = event.screenY - current.y
  if (!current.dragged && Math.max(Math.abs(dx), Math.abs(dy)) < 4) return
  current.dragged = true
  current.delta = props.side === 'top' ? dx : dy
  if (current.sending) return

  current.sending = true
  queue = queue.then(async () => {
    const tab = await current.origin
    if (!tab) return
    while (!current.cancelled && current.sent !== current.delta) {
      current.sent = current.delta
      await props.moveTab(tab, current.sent)
    }
  }).catch((error) => {
    console.error('edge: could not move folded icon', error)
  }).finally(() => {
    current.sending = false
  })
}

function finishDrag(current: Gesture): void {
  if (!current.dragged) return
  queue = queue.then(async () => {
    const tab = await current.origin
    if (tab) await props.saveTab(tab)
  }).catch((error) => console.error('edge: could not save folded icon position', error))
}

function release(event: PointerEvent): void {
  if (gesture?.id !== event.pointerId) return
  move(event)
  suppressClick = gesture.dragged
  finishDrag(gesture)
  gesture = undefined
}

function cancel(event: PointerEvent): void {
  if (gesture?.id !== event.pointerId) return
  gesture.cancelled = true
  finishDrag(gesture)
  suppressClick = true
  gesture = undefined
}

function activate(event: MouseEvent): void {
  // Native button keyboard activation has detail zero.
  if (event.detail === 0 || !suppressClick) emit('expand')
  suppressClick = false
}
</script>

<template>
  <button
    class="tab"
    type="button"
    aria-label="展开 IP Monitor，可沿屏幕边缘拖动"
    title="单击展开，沿屏幕边缘拖动"
    @pointerdown="press"
    @pointermove="move"
    @pointerup="release"
    @pointercancel="cancel"
    @lostpointercapture="cancel"
    @click="activate"
  >
    <Globe :size="18" :stroke-width="1.8" aria-hidden="true" />
  </button>
</template>

<style scoped>
.tab {
  display: grid;
  place-items: center;
  box-sizing: border-box;
  width: min(100%, 32px);
  height: min(100%, 32px);
  padding: 0;
  border: 1px solid rgba(244, 249, 255, 0.72);
  border-radius: 9px;
  background: linear-gradient(145deg, rgba(250, 253, 255, 0.72), rgba(225, 233, 241, 0.49));
  color: #24364a;
  box-shadow: inset 0 0 0 1px rgba(45, 63, 80, 0.12);
  cursor: grab;
  touch-action: none;
  user-select: none;
}

.tab:active { cursor: grabbing; }

.tab:focus-visible {
  border-color: var(--ip-accent-ink);
  outline: 1px solid var(--ip-accent-ink);
  outline-offset: -3px;
}
</style>
