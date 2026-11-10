/**
 * Single-reader state for the card (spec §7.3).
 *
 * `refresh()` asks Rust for a detection in the current mode and applies the
 * answer verbatim: the card never invents an address, and a failed run leaves
 * the previous result on screen marked as out of date (spec §8.2 `Stale`)
 * instead of blanking the card or showing an error as if it were data.
 *
 * No Pinia: the card is the only reader, so a composable ref set is enough.
 */
import { computed, onScopeDispose, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { flagUrl, placeText } from '../lib/flags'
import type {
  ConnectionStatus,
  DetectionMode,
  DetectionModeOption,
  DetectionResult,
  FieldStatus,
  PlaceRow,
} from '../types/ip-monitor'

const MODE_OPTIONS: readonly DetectionModeOption[] = [
  { value: 'direct', label: 'Direct', hint: 'No proxy' },
  { value: 'system_proxy', label: 'System Proxy', hint: 'Use Windows settings' },
  { value: 'custom_proxy', label: 'Custom Proxy', hint: 'Manual endpoint' },
]

const MODE_LABELS: Record<DetectionMode, string> = {
  direct: 'Direct',
  system_proxy: 'System Proxy',
  custom_proxy: 'Custom Proxy',
}

/** How a family's failure reads on the card (spec §5.4: never call it a leak). */
const FAMILY_FAILURE: Record<Exclude<FieldStatus, 'success'>, string> = {
  unavailable: 'Not available',
  timeout: 'Timed out',
  error: 'Failed',
}

interface BackendError {
  kind?: string
  message?: string
}

function describeError(error: unknown): string {
  if (typeof error === 'string') return error
  const shaped = error as BackendError | null
  if (shaped?.message) return shaped.message
  if (error instanceof Error) return error.message
  return '检测失败，原因未知'
}

function formatStamp(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, '0')
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ` +
    `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  )
}

export function useIpMonitor(call: typeof invoke = invoke) {
  // Direct is the honest default: a cold start must not silently inherit
  // whatever proxy Windows happens to have configured. The mode is remembered
  // for the session only; every launch starts from Direct again.
  const mode = ref<DetectionMode>('direct')
  const result = ref<DetectionResult | null>(null)
  const isLoading = ref(false)
  const error = ref<string | null>(null)
  const modeMenuOpen = ref(false)
  const copied = ref(false)

  /** Result that belongs to a mode we are no longer in (spec §7.3). */
  const resultIsStale = computed(
    () => Boolean(result.value) && result.value!.mode !== mode.value,
  )

  /** Monotonic run id; a late answer from an older run is dropped (§7.3). */
  let runId = 0
  let copyTimer: ReturnType<typeof setTimeout> | null = null

  const lastUpdated = computed<Date | null>(() => {
    if (!result.value) return null
    const parsed = Date.parse(result.value.checkedAt)
    return Number.isNaN(parsed) ? null : new Date(parsed)
  })

  /** §8.2 mapping. Empty state is `idle`, never a fake success. */
  const status = computed<ConnectionStatus>(() => {
    if (isLoading.value) return 'checking'
    if (!result.value) return error.value ? 'error' : 'idle'
    if (resultIsStale.value) return 'stale'
    if (error.value) return 'stale'
    const { ipv4Status, ipv6Status } = result.value
    if (ipv4Status === 'success' && ipv6Status === 'success') return 'connected'
    if (ipv4Status === 'success' || ipv6Status === 'success') return 'partial'
    return 'error'
  })

  const ipv4IsEmpty = computed(() => !result.value?.ipv4?.ip)

  /**
   * The hero line. A family that never answered must not read as "no address":
   * "IPv4 timed out" tells the user the request was made and lost, while
   * "IPv4 unavailable" is what the API actually said.
   */
  const ipv4Text = computed(() => {
    const current = result.value
    if (current?.ipv4?.ip) return current.ipv4.ip
    const reason = current ? FAMILY_FAILURE[current.ipv4Status as Exclude<FieldStatus, 'success'>] : null
    return reason ? `IPv4 ${reason.toLowerCase()}` : 'IPv4 unavailable'
  })

  const ipv6Text = computed(() => {
    const current = result.value
    if (!current) return 'Not detected'
    if (current.ipv6Status === 'success' && current.ipv6) return current.ipv6
    return FAMILY_FAILURE[current.ipv6Status as Exclude<FieldStatus, 'success'>] ?? 'Not detected'
  })

  const locationText = computed(() => {
    const info = result.value?.ipv4
    if (!info) return 'Unknown'
    const parts = [info.cityName, info.countryName ?? info.countryCode].filter(
      (part): part is string => Boolean(part),
    )
    return parts.length > 0 ? parts.join(', ') : 'Unknown'
  })

  /**
   * The location cell, one line per address family.
   *
   * Both families are shown whenever both answered — even when they agree —
   * because a card that grows or shrinks a line depending on the data makes the
   * grid jump. A family with no country gets a line but no flag, so the text
   * still lines up.
   */
  const locationRows = computed<PlaceRow[]>(() => {
    const current = result.value
    const v4 = current?.ipv4 ?? null
    const v6 = current?.ipv6Info ?? null
    const hasV4 = Boolean(current && current.ipv4Status === 'success' && v4?.ip)
    const hasV6 = Boolean(current && current.ipv6Status === 'success' && v6?.ip)

    if (hasV4 && hasV6 && v4 && v6) {
      return [
        { tag: 'v4', flag: flagUrl(v4.countryCode), text: placeText(v4) },
        { tag: 'v6', flag: flagUrl(v6.countryCode), text: placeText(v6) },
      ]
    }
    const only = hasV4 ? v4 : hasV6 ? v6 : null
    return [{ tag: null, flag: flagUrl(only?.countryCode), text: placeText(only) }]
  })

  const networkText = computed(() => result.value?.ipv4?.ispName ?? 'Unknown')

  const asnText = computed(() => {
    const info = result.value?.ipv4
    if (!info) return 'Unknown'
    if (info.asNumber === null && !info.asNetwork) return 'Unknown'
    const number = info.asNumber === null ? null : `AS${info.asNumber}`
    return [number, info.asNetwork].filter(Boolean).join(' · ')
  })

  const modeLabel = computed(() => MODE_LABELS[mode.value])
  const modeOptions = MODE_OPTIONS

  /** Nothing truthful to copy until a real detection lands (spec §2.4). */
  const canCopy = computed(() => Boolean(result.value?.ipv4?.ip || result.value?.ipv6))

  /** Where the request really went — shown in the footer tooltip. */
  const routeLabel = computed(() => result.value?.route ?? '')

  /**
   * One line for the footer: a hard failure wins, otherwise the reason the
   * backend attached to an all-failed run (refused proxy, DNS, timeout).
   */
  const message = computed(() => {
    const reason = error.value ?? result.value?.detail ?? null
    if (!reason) return null
    // A stale card has to say *when* the answer it still shows was taken.
    const stamp = lastUpdated.value ? formatStamp(lastUpdated.value) : null
    return stamp ? `${reason} · ${stamp}` : reason
  })

  let requestsInFlight = 0
  let autoRefreshTimer: ReturnType<typeof setInterval> | undefined

  function startAutoRefresh(): void {
    if (autoRefreshTimer !== undefined) return
    autoRefreshTimer = setInterval(() => {
      if (!isLoading.value && requestsInFlight === 0) void refresh()
    }, 120_000)
  }

  onScopeDispose(() => {
    if (autoRefreshTimer !== undefined) clearInterval(autoRefreshTimer)
  })

  async function refresh(): Promise<void> {
    const currentRun = ++runId
    isLoading.value = true
    error.value = null
    requestsInFlight += 1

    try {
      const next = await call<DetectionResult>('get_ip_info', { mode: mode.value })
      if (currentRun !== runId) return

      const failedRun = next.ipv4Status !== 'success' && next.ipv6Status !== 'success'
      const previous = result.value
      const hasPreviousData = Boolean(previous?.ipv4?.ip || previous?.ipv6)

      if (failedRun && hasPreviousData) {
        // Keep the entire successful observation together: route, mode, data
        // and timestamp all describe the same request. Failure is separate.
        error.value = next.detail ?? 'Detection failed'
      } else {
        result.value = next
      }
    } catch (failure) {
      if (currentRun !== runId) return
      // Keep whatever we had: an old answer with a visible timestamp beats a
      // blank card (spec §8.2 Stale).
      error.value = describeError(failure)
    } finally {
      requestsInFlight -= 1
      if (currentRun === runId) isLoading.value = false
    }
  }

  /** Copy format is the one in spec §2.4, filled with real values only. */
  function snapshotText(): string | null {
    const current = result.value
    if (!current || (!current.ipv4?.ip && !current.ipv6)) return null
    const info = current.ipv4 ?? current.ipv6Info
    const stamp = lastUpdated.value ? formatStamp(lastUpdated.value) : 'Unknown'
    const lines = [
      'IP Monitor',
      `Detection Mode: ${MODE_LABELS[current.mode]}`,
      `Route: ${current.route ?? 'Unknown'}`,
      `IPv4: ${current.ipv4?.ip ?? 'Not detected'}`,
      `IPv6: ${current.ipv6Status === 'success' && current.ipv6 ? current.ipv6 : 'Not detected'}`,
      `Location: ${info ? placeText(info) : 'Unknown'}`,
      `ISP: ${info?.ispName ?? 'Not available'}`,
      `ASN: ${info?.asNumber == null ? 'Not available' : `AS${info.asNumber}`}`,
      `AS Network: ${info?.asNetwork ?? 'Not available'}`,
      `Last Updated: ${stamp}`,
    ]
    return lines.join('\n')
  }

  async function copySnapshot(): Promise<void> {
    const text = snapshotText()
    if (!text) return
    try {
      await navigator.clipboard.writeText(text)
      copied.value = true
      if (copyTimer) clearTimeout(copyTimer)
      copyTimer = setTimeout(() => {
        copied.value = false
      }, 1400)
    } catch (failure) {
      error.value = `复制失败：${describeError(failure)}`
    }
  }

  async function selectMode(next: DetectionMode, force = false): Promise<void> {
    modeMenuOpen.value = false
    // `force` exists for the custom-proxy form: saving a new endpoint while the
    // card is already in that mode has to re-run detection, otherwise the error
    // from the previous endpoint stays on screen and looks like the save failed.
    if (next === mode.value && !force) return
    mode.value = next
    // Invalidate the old request immediately, before the mode IPC can yield.
    const selection = ++runId
    isLoading.value = true
    try {
      await call('set_detection_mode', { mode: next })
    } catch {
      // The session state in Rust is a mirror; a failed write only means the
      // next run uses the default, and the run itself will say so.
    }
    if (selection === runId) await refresh()
  }

  function toggleModeMenu(): void {
    modeMenuOpen.value = !modeMenuOpen.value
  }

  function closeModeMenu(): void {
    modeMenuOpen.value = false
  }

  return {
    mode,
    result,
    isLoading,
    error,
    message,
    modeMenuOpen,
    copied,
    lastUpdated,
    status,
    resultIsStale,
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
    routeLabel,
    refresh,
    startAutoRefresh,
    copySnapshot,
    selectMode,
    toggleModeMenu,
    closeModeMenu,
  }
}
