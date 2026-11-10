<script setup lang="ts">
import { onMounted } from 'vue'
import { Check } from '@lucide/vue'
import type { DetectionMode, DetectionModeOption } from '../types/ip-monitor'

/**
 * Static mode popover. It reflects and changes the *selection* only —
 * Phase 1 never starts a detection from here (that is Phase 3).
 */
const props = defineProps<{
  options: readonly DetectionModeOption[]
  modelValue: DetectionMode
}>()

const emit = defineEmits<{
  select: [mode: DetectionMode]
  close: []
}>()

const itemEls: HTMLElement[] = []

function setItemRef(el: unknown, index: number): void {
  if (el instanceof HTMLElement) itemEls[index] = el
}

function focusSelected(): void {
  const index = props.options.findIndex((option) => option.value === props.modelValue)
  itemEls[index >= 0 ? index : 0]?.focus({ preventScroll: true })
}

onMounted(focusSelected)

function moveFocus(step: number): void {
  const count = props.options.length
  if (count === 0) return
  const current = itemEls.findIndex((el) => el === document.activeElement)
  const next = ((current + step) % count + count) % count
  itemEls[next]?.focus({ preventScroll: true })
}

function onKeydown(event: KeyboardEvent): void {
  switch (event.key) {
    case 'Escape':
      emit('close')
      break
    case 'ArrowDown':
      event.preventDefault()
      moveFocus(1)
      break
    case 'ArrowUp':
      event.preventDefault()
      moveFocus(-1)
      break
    case 'Home':
      event.preventDefault()
      itemEls[0]?.focus({ preventScroll: true })
      break
    case 'End':
      event.preventDefault()
      itemEls[itemEls.length - 1]?.focus({ preventScroll: true })
      break
    case 'Tab':
      emit('close')
      break
  }
}
</script>

<template>
  <div
    class="mode-menu ip-material ip-material--menu"
    role="menu"
    aria-label="Detection mode"
    @keydown="onKeydown"
  >
    <p class="mode-menu__title" aria-hidden="true">Detection mode</p>

    <button
      v-for="(option, index) in options"
      :key="option.value"
      :ref="(el) => setItemRef(el, index)"
      class="mode-item"
      :class="{ 'is-selected': option.value === modelValue }"
      type="button"
      role="menuitemradio"
      :aria-checked="option.value === modelValue"
      @click="emit('select', option.value)"
    >
      <span class="mode-item__text">
        <span class="mode-item__label">{{ option.label }}</span>
        <span class="mode-item__hint">{{ option.hint }}</span>
      </span>
      <span class="mode-item__check" aria-hidden="true">
        <Check v-if="option.value === modelValue" :size="14" :stroke-width="2.8" />
      </span>
    </button>
  </div>
</template>

<style scoped>
.mode-menu {
  width: 252px;
  padding: 9px;
  transform-origin: 100% 100%;
}

.mode-menu__title {
  padding: 2px 10px 7px;
  color: var(--ip-text-tertiary);
  font-size: 10.5px;
  font-weight: 700;
  letter-spacing: 0.12em;
  line-height: 1.2;
  text-transform: uppercase;
}

.mode-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 8px 10px;
  border-radius: var(--ip-radius-item);
  text-align: left;
  transition: background-color var(--ip-duration-fast) var(--ip-ease);
}

.mode-item + .mode-item {
  margin-top: 2px;
}

.mode-item:hover {
  background: var(--ip-hover);
}

.mode-item.is-selected {
  background: var(--ip-accent);
  color: var(--ip-ink-on-accent);
  box-shadow:
    inset 0 0 0 1px rgba(120, 140, 10, 0.22),
    0 4px 10px -6px rgba(120, 140, 10, 0.5);
}

.mode-item__text {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.mode-item__label {
  font-size: 13px;
  font-weight: 600;
  letter-spacing: -0.005em;
  line-height: 1.25;
}

.mode-item__hint {
  overflow: hidden;
  color: var(--ip-text-secondary);
  font-size: 11px;
  line-height: 1.3;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.mode-item.is-selected .mode-item__hint {
  color: rgba(35, 39, 13, 0.62);
}

.mode-item__check {
  display: grid;
  flex: none;
  place-items: center;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  color: transparent;
}

.mode-item.is-selected .mode-item__check {
  background: rgba(35, 39, 13, 0.14);
  color: var(--ip-ink-on-accent);
}
</style>
