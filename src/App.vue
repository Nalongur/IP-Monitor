<script setup lang="ts">
import EdgeTab from './components/EdgeTab.vue'
import IpCard from './components/IpCard.vue'
import { useEdgeMode } from './composables/useEdgeMode'

// Folding is decided in Rust (it owns the monitor geometry); the frontend only
// mirrors the state. The card stays mounted so its last detection survives a
// fold/unfold cycle instead of re-running a network request every time.
const { parked, side, motionSide, expand, readTab, moveTab, saveTab } = useEdgeMode()
</script>

<template>
  <main class="stage" :class="{ 'stage--parked': parked }" :data-motion-edge="motionSide">
    <EdgeTab class="stage__tab" :side="side" :read-tab="readTab" :move-tab="moveTab" :save-tab="saveTab" @expand="expand" />
    <div class="stage__card">
      <IpCard />
    </div>
  </main>
</template>

<style scoped>
/* Edge to edge: the window *is* the card. A margin here used to keep the outer
   drop shadow off the window border, but it also stopped the window from being
   draggable at its visible edge and turned the DWM backdrop into a frame around
   the card. The shadow now lives inside as an inset edge (see acrylic.css). */
.stage {
  display: flex;
  width: 100%;
  height: 100%;
}

.stage__card {
  display: flex;
  width: 100%;
  height: 100%;
}

/* A tiny viewport can arrive before the IPC event. Keep that fast path,
   while native parked state keeps the recovery button at any viewport size. */
.stage .stage__tab { display: none; }
.stage--parked { display: grid; place-items: center; }
.stage--parked .stage__tab { display: grid; }
.stage--parked .stage__card { display: none; }

@media (max-width: 100px) and (max-height: 100px) {
  .stage { display: grid; place-items: center; }
  .stage .stage__tab { display: grid; }
  .stage__card { display: none; }
}

@media (prefers-reduced-motion: no-preference) {
  .stage[data-motion-edge='left'] { --edge-x: -9px; }
  .stage[data-motion-edge='right'] { --edge-x: 9px; }
  .stage[data-motion-edge='top'] { --edge-y: -9px; }

  .stage[data-motion-edge] .stage__card {
    animation: reveal-card 210ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  .stage__tab {
    animation: reveal-tab 155ms cubic-bezier(0.22, 1, 0.36, 1);
  }
}

@keyframes reveal-card {
  from { opacity: 0.92; transform: translate(var(--edge-x, 0px), var(--edge-y, 0px)); }
  to { opacity: 1; transform: translate(0, 0); }
}

@keyframes reveal-tab {
  from { opacity: 0.82; scale: 0.88; }
  to { opacity: 1; scale: 1; }
}
</style>
