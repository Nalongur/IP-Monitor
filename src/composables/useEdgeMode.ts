import { onMounted, onUnmounted, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

/** Which edge the tab is parked on. */
export type EdgeSide = 'left' | 'right' | 'top'

export interface EdgeSnapshot {
  parked: boolean
  side: EdgeSide | null
  revision: number
  center: number
}

/**
 * Window-level "folded into an edge tab" state.
 *
 * The decision to fold is made in Rust, because only it knows where the monitors
 * are; the frontend's job is to swap the card for the tab and to ask for the
 * card back when the tab is clicked.
 */
export function useEdgeMode() {
  const parked = ref(false)
  const side = ref<EdgeSide | null>(null)
  const motionSide = ref<EdgeSide | null>(null)
  let unlisten: UnlistenFn | null = null
  let active = true

  function apply(snapshot: EdgeSnapshot): void {
    if (snapshot.side) motionSide.value = snapshot.side
    parked.value = snapshot.parked
    side.value = snapshot.side
  }

  async function expand(): Promise<void> {
    if (isTauri()) await invoke('expand_edge')
  }

  async function readTab(): Promise<EdgeSnapshot | null> {
    if (!isTauri()) return null
    const snapshot = await invoke<EdgeSnapshot>('get_edge_state')
    return snapshot.parked ? snapshot : null
  }

  async function moveTab(tab: EdgeSnapshot, delta: number): Promise<void> {
    await invoke('move_edge_tab', { revision: tab.revision, center: tab.center, delta })
  }

  async function saveTab(tab: EdgeSnapshot): Promise<void> {
    await invoke('save_edge_tab', { revision: tab.revision })
  }

  onMounted(async () => {
    if (!isTauri()) return
    try {
      let receivedEvent = false
      const stop = await listen<EdgeSnapshot>('edge-state', ({ payload }) => {
        receivedEvent = true
        apply(payload)
      })
      if (!active) {
        stop()
        return
      }
      unlisten = stop
      const snapshot = await invoke<EdgeSnapshot>('get_edge_state')
      if (active && !receivedEvent) apply(snapshot)
    } catch (error) {
      console.error('edge: cannot subscribe to folded state', error)
    }
  })

  onUnmounted(() => {
    active = false
    unlisten?.()
  })

  return { parked, side, motionSide, expand, readTab, moveTab, saveTab }
}
