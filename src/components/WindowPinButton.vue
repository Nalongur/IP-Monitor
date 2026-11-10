<script setup lang="ts">
import { Pin, PinOff } from '@lucide/vue'
import { useAppSettings } from '../composables/useAppSettings'

const { pinned, togglePin } = useAppSettings()
</script>

<template>
  <button
    class="pin"
    :class="{ 'is-pinned': pinned }"
    type="button"
    :aria-pressed="pinned"
    :aria-label="pinned ? '取消置顶' : '置顶显示'"
    :title="pinned ? '取消置顶' : '置顶显示'"
    @click="togglePin"
  >
    <Pin v-if="pinned" :size="14" :stroke-width="1.9" aria-hidden="true" />
    <PinOff v-else :size="14" :stroke-width="1.9" aria-hidden="true" />
  </button>
</template>

<style scoped>
.pin {
  display: grid;
  flex: none;
  place-items: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: 0;
  border-radius: 9px;
  color: var(--ip-text-secondary);
  background: transparent;
  cursor: pointer;
  transition:
    color var(--ip-duration-fast) var(--ip-ease),
    background-color var(--ip-duration-fast) var(--ip-ease),
    transform var(--ip-duration-fast) var(--ip-ease);
}

.pin:hover {
  color: var(--ip-text);
  background: var(--ip-hover);
}

.pin:active {
  transform: scale(0.92);
}

.pin.is-pinned {
  color: var(--ip-text);
  background: var(--ip-accent-tint);
}

.pin.is-pinned:hover {
  background: var(--ip-accent-soft);
}

.pin:focus-visible {
  outline: 2px solid var(--ip-focus);
  outline-offset: 2px;
}
</style>
