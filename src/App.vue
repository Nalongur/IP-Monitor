<script setup lang="ts">
import EdgeTab from './components/EdgeTab.vue'
import IpCard from './components/IpCard.vue'
import { useEdgeMode } from './composables/useEdgeMode'

// Folding is decided in Rust (it owns the monitor geometry); the frontend only
// mirrors the state. The card stays mounted so its last detection survives a
// fold/unfold cycle instead of re-running a network request every time.
const { parked, side, expand, readTab, moveTab, saveTab } = useEdgeMode()
</script>

<template>
  <main class="stage" :class="{ 'stage--parked': parked }">
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
  .stage__card,
  .stage__tab {
    animation: reveal-face 260ms cubic-bezier(0.2, 0.65, 0.3, 1);
  }
}

@keyframes reveal-face {
  from { opacity: 0.88; }
  to { opacity: 1; }
}
</style>
