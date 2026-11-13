<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { Check } from '@lucide/vue'
import { useAppSettings, type AccentTheme } from '../composables/useAppSettings'

const props = defineProps<{ theme: AccentTheme }>()

const emit = defineEmits<{
  select: [theme: AccentTheme]
  close: []
}>()

/**
 * Swatch colours are literals, not tokens: each chip has to show the palette it
 * stands for, while the card around it stays on the *currently* selected one.
 */
const OPTIONS: ReadonlyArray<{ value: AccentTheme; label: string; color: string }> = [
  { value: 'lime', label: '荧光绿', color: '#d8ff35' },
  { value: 'aqua', label: '青色', color: '#3fe6f0' },
  { value: 'violet', label: '紫色', color: '#b08cff' },
]

/** Launch-at-login is owned by the OS (Run key), so it is queried, not cached. */
const autostart = ref(!isTauri())
const autostartPending = ref(isTauri())

onMounted(async () => {
  if (!isTauri()) return
  try {
    autostart.value = await invoke<boolean>('get_autostart')
  } catch (error) {
    console.error('settings: cannot read autostart state', error)
  } finally {
    autostartPending.value = false
  }
})

async function toggleAutostart(): Promise<void> {
  if (autostartPending.value) return
  const next = !autostart.value
  autostartPending.value = true
  autostart.value = next
  if (isTauri()) {
    try {
      autostart.value = await invoke<boolean>('set_autostart', { enabled: next })
    } catch (error) {
      console.error('settings: cannot change autostart', error)
      autostart.value = !next
    }
  }
  autostartPending.value = false
}

const { sizeLocked, setSizeLocked } = useAppSettings()
const sizeLockPending = ref(false)

async function toggleSizeLock(): Promise<void> {
  if (sizeLockPending.value) return
  sizeLockPending.value = true
  try {
    await setSizeLocked(!sizeLocked.value)
  } catch (error) {
    console.error('settings: cannot change window size lock', error)
  } finally {
    sizeLockPending.value = false
  }
}

void props
</script>

<template>
  <section
    class="appearance ip-material ip-material--menu"
    role="dialog"
    aria-label="外观与启动"
    data-appearance-root
  >
    <p class="appearance__label" id="appearance-accent-label">强调色</p>
    <div class="swatches" role="radiogroup" aria-labelledby="appearance-accent-label">
      <button
        v-for="option in OPTIONS"
        :key="option.value"
        class="swatch"
        :class="{ 'is-active': option.value === theme }"
        type="button"
        role="radio"
        :aria-checked="option.value === theme"
        :aria-label="option.label"
        :title="option.label"
        :style="{ '--swatch': option.color }"
        @click="emit('select', option.value)"
      >
        <Check v-if="option.value === theme" :size="13" :stroke-width="2.6" aria-hidden="true" />
      </button>
    </div>

    <div class="appearance__divider" role="presentation" />

    <div class="launch">
      <span class="launch__text">
        <span class="launch__title">开机自动启动</span>
        <span class="launch__hint">登录 Windows 后自动显示卡片</span>
      </span>
      <button
        class="switch"
        type="button"
        role="switch"
        :aria-checked="autostart"
        :disabled="autostartPending"
        aria-label="开机自动启动"
        @click="toggleAutostart"
      >
        <span class="switch__knob" aria-hidden="true" />
      </button>
    </div>

    <div class="launch launch--size-lock">
      <span class="launch__text">
        <span class="launch__title">锁定窗口大小</span>
        <span class="launch__hint">禁止拖动窗口边缘和四角来缩放</span>
      </span>
      <button
        class="switch"
        type="button"
        role="switch"
        :aria-checked="sizeLocked"
        :disabled="sizeLockPending"
        aria-label="锁定窗口大小"
        @click="toggleSizeLock"
      >
        <span class="switch__knob" aria-hidden="true" />
      </button>
    </div>
  </section>
</template>

<style scoped>
.appearance {
  width: 244px;
  padding: 15px 15px 14px;
  border-radius: var(--ip-radius-menu);
  color: var(--ip-text);
  font-family: var(--ip-font-sans);
}

.appearance__label {
  color: var(--ip-text-secondary);
  font-size: 10.5px;
  font-weight: 700;
  letter-spacing: 0.13em;
  text-transform: uppercase;
}

.swatches {
  display: flex;
  gap: 9px;
  margin-top: 10px;
}

.swatch {
  display: grid;
  flex: 1;
  height: 30px;
  place-items: center;
  border: 0;
  border-radius: 10px;
  background: var(--swatch);
  box-shadow: inset 0 0 0 1px rgba(36, 37, 34, 0.14);
  color: #23270d;
  cursor: pointer;
  transition:
    box-shadow var(--ip-duration-fast) var(--ip-ease),
    transform var(--ip-duration-fast) var(--ip-ease);
}

.swatch:hover {
  transform: translateY(-1px);
}

.swatch.is-active {
  box-shadow:
    inset 0 0 0 1px rgba(36, 37, 34, 0.18),
    0 0 0 2px var(--ip-surface-menu),
    0 0 0 3.5px rgba(36, 37, 34, 0.55);
}

.swatch:focus-visible {
  outline: 2px solid var(--ip-focus);
  outline-offset: 3px;
}

.appearance__divider {
  height: 1px;
  margin: 13px 0;
  background: var(--ip-divider);
}

.launch {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.launch--size-lock {
  margin-top: 11px;
}

.launch__text {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.launch__title {
  font-size: 12.5px;
  font-weight: 600;
  line-height: 1.2;
}

.launch__hint {
  color: var(--ip-text-secondary);
  font-size: 10.5px;
  line-height: 1.25;
}

.switch {
  position: relative;
  flex: none;
  width: 36px;
  height: 20px;
  padding: 0;
  border: 0;
  border-radius: 999px;
  background: rgba(60, 60, 55, 0.16);
  cursor: pointer;
  transition: background-color var(--ip-duration) var(--ip-ease);
}

.switch[aria-checked='true'] {
  background: var(--ip-accent);
}

.switch:disabled {
  opacity: 0.55;
}

.switch:focus-visible {
  outline: 2px solid var(--ip-focus);
  outline-offset: 2px;
}

.switch__knob {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #ffffff;
  box-shadow: 0 1px 2px rgba(31, 34, 23, 0.28);
  transition: transform var(--ip-duration) var(--ip-ease);
}

.switch[aria-checked='true'] .switch__knob {
  transform: translateX(16px);
}
</style>
