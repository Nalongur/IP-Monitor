<script setup lang="ts">
import { Ellipsis } from '@lucide/vue'

/**
 * Top-right entry point for the appearance popover (accent theme and
 * launch-at-login). The visual slot was reserved in Phase 1; the button now
 * opens that surface and nothing else, so it never competes with the
 * detection-mode control in the footer.
 */
withDefaults(defineProps<{ expanded?: boolean }>(), { expanded: false })

defineEmits<{ open: [] }>()
</script>

<template>
  <button
    class="reserved"
    type="button"
    aria-haspopup="dialog"
    aria-label="外观与启动"
    :aria-expanded="expanded"
    title="外观与启动"
    @click="$emit('open')"
  >
    <Ellipsis :size="18" :stroke-width="1.9" aria-hidden="true" />
  </button>
</template>

<style scoped>
.reserved {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  margin-right: -4px;
  border-radius: 10px;
  color: var(--ip-text-secondary);
  transition:
    background-color var(--ip-duration-fast) var(--ip-ease),
    color var(--ip-duration-fast) var(--ip-ease);
}

.reserved:hover {
  background: var(--ip-hover);
  color: var(--ip-text);
}

.reserved:active {
  background: var(--ip-pressed);
}
</style>
