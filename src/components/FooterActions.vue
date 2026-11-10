<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import { Check, ChevronUp, Copy, RefreshCw, SlidersHorizontal } from '@lucide/vue'

withDefaults(
  defineProps<{
    modeLabel: string
    canCopy?: boolean
    isLoading?: boolean
    menuOpen?: boolean
    copied?: boolean
    message?: string | null
  }>(),
  {
    canCopy: false,
    isLoading: false,
    menuOpen: false,
    copied: false,
    message: null,
  },
)

const emit = defineEmits<{
  refresh: []
  copy: []
  'toggle-mode-menu': []
}>()

/**
 * One full turn per press, re-keyed so repeated clicks restart the animation
 * instead of stacking it. Phase 2 switches to the loading spin, which must
 * still run at least one turn before it stops.
 */
const spinToken = ref(0)
const spinOnce = ref(false)
let spinTimer: number | undefined

function onRefreshClick(): void {
  spinToken.value += 1
  spinOnce.value = true
  window.clearTimeout(spinTimer)
  spinTimer = window.setTimeout(() => {
    spinOnce.value = false
  }, 520)
  emit('refresh')
}

onBeforeUnmount(() => window.clearTimeout(spinTimer))
</script>

<template>
  <div class="actions">
    <div class="actions__cluster">
      <button
        class="icon-btn icon-btn--primary"
        type="button"
        aria-label="Refresh IP information"
        title="Refresh"
        :disabled="isLoading"
        @click="onRefreshClick"
      >
        <RefreshCw
          :key="spinToken"
          :size="17"
          :stroke-width="2"
          :class="{ 'is-spinning': isLoading, 'is-spinning-once': spinOnce }"
        />
      </button>

      <button
        class="icon-btn"
        type="button"
        aria-label="Copy IP information"
        :title="copied ? 'Copied' : canCopy ? 'Copy' : 'Nothing to copy yet'"
        :disabled="!canCopy"
        @click="emit('copy')"
      >
        <Check v-if="copied" :size="17" :stroke-width="2.2" />
        <Copy v-else :size="17" :stroke-width="1.9" />
      </button>
    </div>

    <p v-if="message" class="actions__message" :title="message" role="status">{{ message }}</p>

    <button
      class="mode-btn"
      :class="{ 'is-open': menuOpen }"
      type="button"
      data-mode-menu-trigger
      aria-haspopup="menu"
      :aria-expanded="menuOpen"
      :title="`Detection mode: ${modeLabel}`"
      @click="emit('toggle-mode-menu')"
    >
      <SlidersHorizontal :size="16" :stroke-width="1.9" class="mode-btn__icon" />
      <span class="mode-btn__label">{{ modeLabel }}</span>
      <ChevronUp :size="14" :stroke-width="2" class="mode-btn__chevron" />
    </button>
  </div>
</template>

<style scoped>
.actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

/* Failure explanations sit between the buttons: the card must say *why* it is
   showing stale numbers instead of silently keeping them (spec §2.3). */
.actions__message {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  color: #a3372c;
  font-size: 10.5px;
  font-weight: 550;
  line-height: 1.3;
  text-align: right;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.actions__cluster {
  display: flex;
  align-items: center;
  gap: 8px;
}

.icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 14px;
  color: var(--ip-text-secondary);
  background: transparent;
  transition:
    background-color var(--ip-duration-fast) var(--ip-ease),
    color var(--ip-duration-fast) var(--ip-ease),
    box-shadow var(--ip-duration-fast) var(--ip-ease);
}

.icon-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.62);
  color: var(--ip-text);
  box-shadow: 0 1px 2px rgba(31, 34, 23, 0.07), inset 0 0 0 1px rgba(255, 255, 255, 0.6);
}

.icon-btn:active:not(:disabled) {
  background: rgba(255, 255, 255, 0.4);
}

.icon-btn:disabled {
  opacity: 0.38;
}

/* The refresh control is the single dark anchor of the card. */
.icon-btn--primary {
  color: var(--ip-ink-on-dark);
  background: #2a2c26;
  box-shadow:
    0 6px 14px -6px rgba(31, 34, 23, 0.5),
    inset 0 1px 0 rgba(255, 255, 255, 0.14);
  transition:
    background-color var(--ip-duration-fast) var(--ip-ease),
    box-shadow var(--ip-duration-fast) var(--ip-ease),
    transform 200ms var(--ip-ease);
}

.icon-btn--primary:hover:not(:disabled) {
  color: var(--ip-ink-on-dark);
  background: #34372f;
  box-shadow:
    0 9px 18px -7px rgba(31, 34, 23, 0.55),
    inset 0 1px 0 rgba(255, 255, 255, 0.18);
}

.icon-btn--primary:active:not(:disabled) {
  background: #1f211c;
  transform: scale(0.93);
  transition-duration: 90ms;
}

.icon-btn--primary:disabled {
  opacity: 0.5;
}

.is-spinning {
  animation: spin 1s linear infinite;
}

.is-spinning-once {
  animation: spin 520ms var(--ip-ease) 1;
}

@media (prefers-reduced-motion: reduce) {
  .is-spinning,
  .is-spinning-once {
    animation: none;
  }

  .icon-btn--primary:active:not(:disabled) {
    transform: none;
  }
}

/* ---- mode trigger ---- */

.mode-btn {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  height: 36px;
  max-width: 190px;
  padding: 0 9px 0 12px;
  border: 1px solid var(--ip-divider);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.42);
  color: var(--ip-text-secondary);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.01em;
  transition:
    background-color var(--ip-duration-fast) var(--ip-ease),
    color var(--ip-duration-fast) var(--ip-ease),
    border-color var(--ip-duration-fast) var(--ip-ease);
}

.mode-btn:hover {
  background: rgba(255, 255, 255, 0.66);
  color: var(--ip-text);
}

.mode-btn.is-open {
  border-color: var(--ip-accent-line);
  background: var(--ip-accent-tint);
  color: var(--ip-accent-ink);
}

.mode-btn__icon {
  flex: none;
  opacity: 0.86;
}

/* ---- compact card (< 460px) -------------------------------------------
   The whole card shrinks towards a 320 × 210 minimum, so the footer has to
   give back height before the address does. */
@media (max-width: 460px) {
  .actions {
    gap: 10px;
  }

  .icon-btn {
    width: 30px;
    height: 30px;
    border-radius: 11px;
  }

  .mode-btn {
    gap: 6px;
    height: 30px;
    max-width: 148px;
    padding: 0 7px 0 9px;
    border-radius: 11px;
    font-size: 11px;
  }
}

.mode-btn__label {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.mode-btn__chevron {
  flex: none;
  opacity: 0.62;
  transition: transform var(--ip-duration) var(--ip-ease);
}

.mode-btn.is-open .mode-btn__chevron {
  transform: rotate(180deg);
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
