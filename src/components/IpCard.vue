<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Globe } from '@lucide/vue'
import StatusBadge from './StatusBadge.vue'
import WindowPinButton from './WindowPinButton.vue'
import ReservedMenuButton from './ReservedMenuButton.vue'
import IpInfoGrid from './IpInfoGrid.vue'
import FooterActions from './FooterActions.vue'
import DetectionModeMenu from './DetectionModeMenu.vue'
import SettingsPopover from './SettingsPopover.vue'
import CustomProxyPanel from './CustomProxyPanel.vue'
import { useIpMonitor } from '../composables/useIpMonitor'
import { flagUrl } from '../lib/flags'
import { useAppSettings } from '../composables/useAppSettings'
import type { CustomProxyView, DetectionMode, IpInfoItem } from '../types/ip-monitor'

const {
  result,
  isLoading,
  mode,
  modeMenuOpen,
  status,
  ipv4IsEmpty,
  ipv4Text,
  ipv6Text,
  locationText,
  locationRows,
  networkText,
  asnText,
  modeLabel,
  modeOptions,
  canCopy,
  copied,
  message,
  error,
  refresh,
  copySnapshot,
  selectMode,
  toggleModeMenu,
  closeModeMenu,
} = useIpMonitor()

const { theme, selectTheme } = useAppSettings()

/** Only one popover is open at a time; the mode popover lives in the composable. */
const appearanceOpen = ref(false)

/**
 * The custom-proxy form opens from the mode menu when nothing is configured
 * yet. `customProxy` mirrors what the backend stored, minus the password: the
 * card only needs it to decide whether a form is still required.
 */
const proxyPanelOpen = ref(false)
const customProxy = ref<CustomProxyView | null>(null)

async function loadCustomProxy(): Promise<void> {
  if (!isTauri()) return
  try {
    customProxy.value = await invoke<CustomProxyView | null>('get_custom_proxy')
  } catch {
    customProxy.value = null
  }
}

/**
 * Long addresses buy their extra width back by getting smaller instead of being
 * cut off. A full IPv6 is 39 characters — roughly 2.6× a dotted IPv4 — so at the
 * default 44px it would otherwise end in an ellipsis, which is useless in a tool
 * whose whole job is showing the address.
 */
/** Country flag for the headline address; null when nothing real is shown. */
const heroFlag = computed(() => flagUrl(result.value?.ipv4?.countryCode))

const ipScale = computed(() => {
  const length = ipv4Text.value.length
  if (length <= 20) return '1'
  if (length <= 28) return '0.8'
  return '0.62'
})

const infoItems = computed<IpInfoItem[]>(() => {
  const info = result.value?.ipv4 ?? null
  const rows = locationRows.value
  return [
    {
      label: 'Location',
      value: locationText.value,
      empty: !info?.cityName && !info?.countryName && !info?.countryCode,
      rows: rows.length > 1 || rows[0]?.flag ? rows : undefined,
    },
    { label: 'Network', value: networkText.value, empty: !info?.ispName },
    { label: 'ASN', value: asnText.value, empty: info?.asNumber == null && !info?.asNetwork },
    {
      label: 'IPv6',
      value: ipv6Text.value,
      empty: result.value?.ipv6Status !== 'success' || !result.value?.ipv6,
    },
  ]
})

/**
 * Frameless window drag. Tauri performs the native move loop, which keeps
 * dragging smooth and pointer-capture correct; buttons are excluded so the
 * mode menu stays clickable.
 */
function onCardPointerDown(event: PointerEvent): void {
  if (event.button !== 0) return
  if (
    event.target instanceof Element &&
    event.target.closest('button, [data-mode-menu-root], [data-appearance-root], [data-custom-proxy-root]')
  ) {
    return
  }
  if (!isTauri()) return
  void getCurrentWindow().startDragging()
}

function onDocumentPointerDown(event: PointerEvent): void {
  const target = event.target
  if (!(target instanceof Element)) return
  if (
    target.closest(
      '[data-mode-menu-root], [data-mode-menu-trigger], [data-appearance-root], [data-appearance-trigger], [data-custom-proxy-root]',
    )
  ) {
    return
  }
  closeModeMenu()
  closeAppearance()
  closeProxyPanel()
}

function onDocumentEscape(event: KeyboardEvent): void {
  if (event.key !== 'Escape') return
  if (proxyPanelOpen.value) {
    closeProxyPanel(true)
    return
  }
  if (appearanceOpen.value) {
    closeAppearance()
    focusAppearanceTrigger()
  }
  closeModeMenu()
  focusModeTrigger()
}

function focusAppearanceTrigger(): void {
  document
    .querySelector<HTMLElement>('[data-appearance-trigger]')
    ?.focus({ preventScroll: true })
}

function toggleAppearance(): void {
  appearanceOpen.value = !appearanceOpen.value
  if (appearanceOpen.value) closeModeMenu()
}

function closeAppearance(restoreFocus = false): void {
  if (!appearanceOpen.value) return
  appearanceOpen.value = false
  if (restoreFocus) focusAppearanceTrigger()
}

function closeProxyPanel(restoreFocus = false): void {
  if (!proxyPanelOpen.value) return
  proxyPanelOpen.value = false
  if (restoreFocus) focusModeTrigger()
}

function onToggleModeMenu(): void {
  closeAppearance()
  closeProxyPanel()
  toggleModeMenu()
}

function focusModeTrigger(): void {
  document.querySelector<HTMLElement>('[data-mode-menu-trigger]')?.focus({ preventScroll: true })
}

let listening = false

function bindMenuDismiss(): void {
  if (listening) return
  document.addEventListener('pointerdown', onDocumentPointerDown, true)
  document.addEventListener('keydown', onDocumentEscape)
  listening = true
}

function unbindMenuDismiss(): void {
  if (!listening) return
  document.removeEventListener('pointerdown', onDocumentPointerDown, true)
  document.removeEventListener('keydown', onDocumentEscape)
  listening = false
}

watch([modeMenuOpen, appearanceOpen, proxyPanelOpen], ([mode, appearance, proxy]) => {
  if (mode || appearance || proxy) bindMenuDismiss()
  else unbindMenuDismiss()
})

onBeforeUnmount(unbindMenuDismiss)

/**
 * First detection runs on mount so the card has real data before the user
 * touches anything (spec §8.1). Outside Tauri (the browser preview used while
 * designing the card) there is no backend, so the request is skipped and the
 * card keeps its honest empty state.
 */
onMounted(() => {
  if (!isTauri()) return
  void refresh()
  void loadCustomProxy()
})

function onSelectMode(next: DetectionMode): void {
  closeModeMenu()
  // Picking Custom Proxy without a saved endpoint would only produce an error;
  // open the form instead and let "保存并检测" be the thing that switches mode.
  // Picking it again while already in that mode is how the saved endpoint gets
  // edited — otherwise a wrong port would be impossible to correct from the card.
  if (next === 'custom_proxy' && (!customProxy.value || mode.value === 'custom_proxy')) {
    proxyPanelOpen.value = true
    void loadCustomProxy()
    return
  }
  selectMode(next)
  // The selected item unmounts with the menu; keep keyboard focus predictable.
  focusModeTrigger()
}

function onProxySaved(view: CustomProxyView): void {
  customProxy.value = view
  closeProxyPanel()
  // Re-test on every save: a corrected port or password has to produce a new
  // result, not leave the previous failure on screen.
  void selectMode('custom_proxy', true)
  focusModeTrigger()
}

function onMenuClose(): void {
  closeModeMenu()
  // Only keyboard dismissal reaches this path; return focus to the trigger.
  focusModeTrigger()
}
</script>

<template>
  <section class="card ip-material ip-material--card" aria-label="IP Monitor" @pointerdown="onCardPointerDown">
    <header class="card__head">
      <div class="brand">
        <span class="brand__mark" aria-hidden="true">
          <Globe :size="19" :stroke-width="1.7" />
        </span>
        <h1 class="brand__title">IP Monitor</h1>
      </div>

      <div class="card__head-right">
        <StatusBadge :status="status" />
        <WindowPinButton />
        <ReservedMenuButton
          data-appearance-trigger
          :expanded="appearanceOpen"
          @open="toggleAppearance"
        />
      </div>
    </header>

    <div class="ip-block">
      <p class="ip-block__value" :class="{ 'is-empty': ipv4IsEmpty }" :style="{ '--ip-scale': ipScale }">
        <span class="ip-block__text">{{ ipv4Text }}</span>
        <!-- The flag belongs to the address it was resolved for, so it travels
             with the value and disappears together with real data. -->
        <img
          v-if="heroFlag && !ipv4IsEmpty"
          class="ip-block__flag"
          :src="heroFlag"
          alt=""
          aria-hidden="true"
        />
      </p>
    </div>

    <IpInfoGrid :items="infoItems" />

    <footer class="card__foot">
      <FooterActions
        :mode-label="modeLabel"
        :can-copy="canCopy"
        :is-loading="isLoading"
        :menu-open="modeMenuOpen"
        :copied="copied"
        :message="message"
        @refresh="refresh"
        @copy="copySnapshot"
        @toggle-mode-menu="onToggleModeMenu"
      />
    </footer>

    <Transition name="menu">
      <DetectionModeMenu
        v-if="modeMenuOpen"
        class="card__mode-menu"
        data-mode-menu-root
        :options="modeOptions"
        :model-value="mode"
        @select="onSelectMode"
        @close="onMenuClose"
      />
    </Transition>

    <Transition name="menu">
      <CustomProxyPanel
        v-if="proxyPanelOpen"
        class="card__proxy"
        :initial="customProxy"
        @saved="onProxySaved"
        @close="closeProxyPanel(true)"
      />
    </Transition>

    <Transition name="menu">
      <SettingsPopover
        v-if="appearanceOpen"
        class="card__appearance"
        :theme="theme"
        @select="selectTheme"
        @close="closeAppearance(true)"
      />
    </Transition>
  </section>
</template>

<style scoped>
.card {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  padding: 24px var(--ip-gutter) 22px;
}

/* ---- header ---- */

.card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
}

.brand {
  display: flex;
  align-items: center;
  gap: 11px;
  min-width: 0;
}

.brand__mark {
  display: grid;
  flex: none;
  place-items: center;
  width: 32px;
  height: 32px;
  border-radius: 10px;
  color: var(--ip-accent);
  background: #2a2c26;
  box-shadow:
    0 6px 14px -8px rgba(31, 34, 23, 0.55),
    inset 0 1px 0 rgba(255, 255, 255, 0.16);
}

.brand__title {
  color: var(--ip-text);
  font-family: var(--ip-font-display);
  font-size: 16px;
  font-weight: 600;
  letter-spacing: -0.01em;
  line-height: 1;
}

.card__head-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* ---- primary value ---- */

.ip-block {
  display: flex;
  /* Grows faster than the info grid so the 44px address stops hogging the
     vertical space once the card is at its default 440px height. Basis 0 keeps
     the 1.15 : 1 ratio exact instead of "whichever content is taller wins". */
  flex: 1.15;
  flex-direction: column;
  justify-content: center;
  min-height: 0;
  padding: 6px 0 2px;
}

.ip-block__value {
  display: flex;
  align-items: center;
  gap: 0.34em;
  overflow: hidden;
  color: var(--ip-text);
  font-family: var(--ip-font-display);
  /* 760px → 44px, 520px (the floor) → 30px; `--ip-scale` then trims it further
     for long addresses. The ellipsis below is a last resort, not the plan. */
  font-size: calc(clamp(28px, 5.79vw, 44px) * var(--ip-scale, 1));
  font-weight: 560;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.035em;
  line-height: 1.05;
  white-space: nowrap;
}

.ip-block__text {
  overflow: hidden;
  text-overflow: ellipsis;
}

/*
 * Sized in m on purpose: the flag is part of the address line, so it shrinks
 * with it when the window narrows or the address is a 39-character IPv6.
 */
.ip-block__flag {
  flex: none;
  width: 0.86em;
  height: 0.62em;
  border-radius: 3px;
  object-fit: cover;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
  transform: translateY(0.02em);
}

.ip-block__value.is-empty {
  color: var(--ip-text-secondary);
  font-weight: 480;
  letter-spacing: -0.025em;
  opacity: 0.92;
}

/* ---- footer ---- */

.card__foot {
  padding-top: 17px;
  border-top: 1px solid var(--ip-divider);
}

/* ---- mode popover ---- */

.card__mode-menu {
  position: absolute;
  right: var(--ip-gutter);
  bottom: 66px;
  z-index: 6;
}

.card__appearance {
  position: absolute;
  top: 56px;
  right: calc(var(--ip-gutter) - 4px);
  max-height: calc(100% - 64px);
  overflow-y: auto;
  z-index: 7;
}

.card__proxy {
  position: absolute;
  right: var(--ip-gutter);
  bottom: 66px;
  z-index: 8;
}

.menu-enter-active,
.menu-leave-active {
  transition:
    opacity 140ms var(--ip-ease),
    transform 160ms var(--ip-ease);
}

.menu-enter-from,
.menu-leave-to {
  opacity: 0;
  transform: translateY(6px) scale(0.97);
}

/* ---- compact card (< 460px) -------------------------------------------
   Down to 320 × 210 the card has to shed padding, the brand label and the
   header controls before the address loses its size. */
@media (max-width: 460px) {
  .card {
    padding: 16px var(--ip-gutter) 14px;
  }

  .card__head,
  .brand {
    gap: 8px;
  }

  .brand__mark {
    width: 26px;
    height: 26px;
    border-radius: 8px;
  }

  .brand__title {
    font-size: 13.5px;
  }

  .card__head-right {
    gap: 4px;
  }

  .ip-block {
    padding: 2px 0 10px;
  }

  .ip-block__value {
    font-size: calc(clamp(22px, 6.6vw, 34px) * var(--ip-scale, 1));
  }

  .card__foot {
    padding-top: 10px;
  }

  .card__mode-menu {
    bottom: 52px;
  }

  .card__appearance {
    top: 44px;
    max-height: calc(100% - 52px);
  }

  .card__proxy {
    bottom: 52px;
  }
}
</style>
