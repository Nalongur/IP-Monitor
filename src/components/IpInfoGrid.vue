<script setup lang="ts">
import type { IpInfoItem } from '../types/ip-monitor'

defineProps<{ items: readonly IpInfoItem[] }>()
</script>

<template>
  <dl class="grid">
    <div v-for="item in items" :key="item.label" class="grid__cell">
      <dt class="grid__label">{{ item.label }}</dt>
      <dd v-if="item.rows" class="grid__value grid__value--stack">
        <span v-for="row in item.rows" :key="row.tag ?? 'one'" class="place">
          <span v-if="row.tag" class="place__tag">{{ row.tag }}</span>
          <img v-if="row.flag" class="place__flag" :src="row.flag" alt="" aria-hidden="true" />
          <span class="place__text" :class="{ 'is-empty': item.empty }">{{ row.text }}</span>
        </span>
      </dd>
      <dd v-else class="grid__value" :class="{ 'is-empty': item.empty }">
        {{ item.value }}
      </dd>
    </div>
  </dl>
</template>

<style scoped>
.grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1.12fr);
  column-gap: clamp(18px, 4.2vw, 32px);
  row-gap: clamp(12px, 4.4vh, 20px);
  flex: 1;
  align-content: space-evenly;
  padding-top: 18px;
  border-top: 1px solid var(--ip-divider);
}

.grid__cell {
  min-width: 0;
}

.grid__label {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.1em;
  line-height: 1.2;
  text-transform: uppercase;
  color: var(--ip-text-secondary);
}

.grid__value {
  margin-top: 6px;
  overflow: hidden;
  color: var(--ip-text);
  font-size: 15.5px;
  font-weight: 550;
  line-height: 1.35;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.grid__value.is-empty {
  color: var(--ip-text-secondary);
  font-weight: 500;
}

/* ---- location cell ---- */

.grid__value--stack {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 3px;
}

.place {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.place__tag {
  flex: none;
  width: 16px;
  color: var(--ip-text-tertiary);
  font-size: 9.5px;
  font-weight: 600;
  letter-spacing: 0.06em;
  line-height: 1;
}

/*
 * 4:3 flag, 18×13. The inset ring keeps white flags (JP, PL) from dissolving
 * into the card, which is exactly what happens without it at this size.
 */
.place__flag {
  flex: none;
  width: 18px;
  height: 13px;
  border-radius: 2px;
  object-fit: cover;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
  transform: translateY(-1px);
}

.place__text {
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 15.5px;
  font-weight: 550;
  line-height: 1.3;
}

.place__text.is-empty {
  color: var(--ip-text-secondary);
  font-weight: 500;
}

@media (max-width: 460px) {
  .grid {
    column-gap: 14px;
    row-gap: 8px;
    padding-top: 10px;
  }

  .grid__label {
    font-size: 9px;
  }

  .grid__value,
  .place__text {
    margin-top: 2px;
    font-size: 12px;
    line-height: 1.3;
  }

  .grid__value--stack {
    gap: 2px;
    margin-top: 1px;
  }

  .place {
    gap: 5px;
  }

  .place__tag {
    width: 13px;
    font-size: 8px;
  }

  .place__flag {
    width: 14px;
    height: 10px;
  }
}
</style>
