/**
 * Window-level preferences shared by the card chrome.
 *
 * The source of truth stays in Rust (`src-tauri/src/settings.rs`): the pin
 * state has to be applied before the window is shown, so the frontend only
 * mirrors what the shell reports. Tray toggles arrive as `settings-changed`
 * events, which keeps the button and the tray checkbox from drifting apart.
 */
import { computed, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export type AccentTheme = 'lime' | 'aqua' | 'violet'

export interface AppSettings {
  pinned: boolean
  sizeLocked: boolean
  theme: AccentTheme
}

const DEFAULTS: AppSettings = { pinned: true, sizeLocked: false, theme: 'lime' }

const settings = ref<AppSettings>({ ...DEFAULTS })
let bridgeBound = false

function applyTheme(theme: AccentTheme): void {
  document.documentElement.dataset.theme = theme
}

/** Load once, before `createApp().mount()`, so the first frame has the right theme. */
export async function initAppSettings(): Promise<void> {
  if (!isTauri()) {
    applyTheme(DEFAULTS.theme)
    return
  }

  try {
    settings.value = await invoke<AppSettings>('get_settings')
  } catch (error) {
    console.error('settings: falling back to defaults', error)
    settings.value = { ...DEFAULTS }
  }
  applyTheme(settings.value.theme)

  if (bridgeBound) return
  try {
    await listen<AppSettings>('settings-changed', (event) => {
      settings.value = event.payload
      applyTheme(event.payload.theme)
    })
    bridgeBound = true
  } catch (error) {
    console.error('settings: cannot subscribe to changes', error)
  }
}

async function togglePin(): Promise<void> {
  const previous = settings.value
  const next = !previous.pinned
  // Optimistic: the click must feel instant; the shell's answer wins if it disagrees.
  settings.value = { ...previous, pinned: next }
  if (!isTauri()) return

  try {
    const effective = await invoke<boolean>('set_pinned', { pinned: next })
    settings.value = { ...settings.value, pinned: effective }
  } catch (error) {
    console.error('settings: cannot change pin state', error)
    settings.value = previous
  }
}

async function selectTheme(theme: AccentTheme): Promise<void> {
  if (settings.value.theme === theme) return
  settings.value = { ...settings.value, theme }
  applyTheme(theme)
  if (!isTauri()) return

  try {
    settings.value = await invoke<AppSettings>('set_theme', { theme })
  } catch (error) {
    console.error('settings: cannot change theme', error)
  }
}

async function setSizeLocked(locked: boolean): Promise<void> {
  const effective = isTauri() ? await invoke<boolean>('set_size_locked', { locked }) : locked
  settings.value = { ...settings.value, sizeLocked: effective }
}

export function useAppSettings() {
  return {
    settings,
    pinned: computed(() => settings.value.pinned),
    theme: computed(() => settings.value.theme),
    togglePin,
    sizeLocked: computed(() => settings.value.sizeLocked),
    setSizeLocked,
    selectTheme,
  }
}
