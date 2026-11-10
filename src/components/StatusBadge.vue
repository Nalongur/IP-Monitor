<script setup lang="ts">
import { computed } from 'vue'
import type { ConnectionStatus } from '../types/ip-monitor'

const props = defineProps<{ status: ConnectionStatus }>()

const LABELS: Record<ConnectionStatus, string> = {
  idle: 'Idle',
  checking: 'Checking',
  connected: 'Connected',
  partial: 'Partial',
  error: 'Error',
  stale: 'Stale',
}

const label = computed(() => LABELS[props.status])
</script>

<template>
  <span class="pill" :class="`pill--${status}`" role="status">
    <span class="pill__dot" aria-hidden="true" />
    <span class="pill__label">{{ label }}</span>
  </span>
</template>

<style scoped>
.pill {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  height: 26px;
  padding: 0 11px 0 9px;
  border: 1px solid rgba(60, 60, 55, 0.13);
  border-radius: var(--ip-radius-pill);
  background: rgba(255, 255, 255, 0.52);
  color: var(--ip-text-secondary);
  font-size: 11.5px;
  font-weight: 600;
  letter-spacing: 0.015em;
  line-height: 1;
  white-space: nowrap;
}

.pill__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ip-status-idle);
  box-shadow: 0 0 0 3px rgba(155, 157, 149, 0.14);
}

/* ---- per-status tints (spec §8.2) ---- */

.pill--checking {
  border-color: rgba(192, 138, 46, 0.3);
  background: rgba(192, 138, 46, 0.09);
  color: #8a6317;
}

.pill--checking .pill__dot {
  background: var(--ip-status-checking);
  box-shadow: 0 0 0 3px rgba(192, 138, 46, 0.16);
  animation: pill-pulse 1.15s ease-in-out infinite;
}

.pill--connected {
  border-color: var(--ip-accent-line);
  background: var(--ip-accent-tint);
  color: var(--ip-accent-ink);
}

.pill--connected .pill__dot {
  background: var(--ip-status-connected);
  box-shadow: 0 0 0 3px var(--ip-accent-soft);
}

.pill--partial {
  border-color: rgba(192, 138, 46, 0.3);
  background: rgba(192, 138, 46, 0.09);
  color: #8a6317;
}

.pill--partial .pill__dot {
  background: var(--ip-status-partial);
  box-shadow: 0 0 0 3px rgba(192, 138, 46, 0.16);
}

.pill--error {
  border-color: rgba(180, 70, 47, 0.28);
  background: rgba(180, 70, 47, 0.08);
  color: #9d3a26;
}

.pill--error .pill__dot {
  background: var(--ip-status-error);
  box-shadow: 0 0 0 3px rgba(180, 70, 47, 0.16);
}

.pill--stale .pill__dot {
  background: var(--ip-status-stale);
  box-shadow: 0 0 0 3px rgba(141, 143, 134, 0.14);
}

@keyframes pill-pulse {
  0%,
  100% {
    box-shadow: 0 0 0 3px rgba(192, 138, 46, 0.16);
  }
  50% {
    box-shadow: 0 0 0 5px rgba(192, 138, 46, 0.05);
  }
}

/* Compact card: the pill shrinks with the header so the address keeps its row. */
@media (max-width: 460px) {
  .pill {
    height: 22px;
    padding: 0 8px 0 7px;
    font-size: 10.5px;
  }
}
</style>

