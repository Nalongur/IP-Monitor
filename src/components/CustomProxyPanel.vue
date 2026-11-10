<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import type { CustomProxyConfig, CustomProxyView, ProxyProtocol } from '../types/ip-monitor'

const props = defineProps<{ initial?: CustomProxyView | null }>()

const emit = defineEmits<{
  saved: [view: CustomProxyView]
  close: []
}>()

const PROTOCOLS: { value: ProxyProtocol; label: string }[] = [
  { value: 'http', label: 'HTTP' },
  { value: 'https', label: 'HTTPS' },
  { value: 'socks5', label: 'SOCKS5' },
]

const protocol = ref<ProxyProtocol>(props.initial?.protocol ?? 'socks5')
const host = ref(props.initial?.host ?? '127.0.0.1')
const port = ref(String(props.initial?.port ?? 7890))
const username = ref(props.initial?.username ?? '')
const password = ref('')
const busy = ref(false)
const error = ref<string | null>(null)

const endpointHint = computed(() => {
  const trimmed = host.value.trim()
  if (!trimmed) return 'host:port'
  const bracketed = trimmed.includes(':') && !trimmed.startsWith('[') ? `[${trimmed}]` : trimmed
  return `${bracketed}:${port.value || '—'}`
})

function payload(): CustomProxyConfig | null {
  const portNumber = Number(port.value)
  if (!Number.isInteger(portNumber) || portNumber < 1 || portNumber > 65535) {
    error.value = '端口需要是 1–65535 的整数'
    return null
  }
  const user = username.value.trim()
  // An empty field means "keep the stored password" — the backend still has the
  // DPAPI blob, and the form never shows the old secret back.
  if (user && !password.value && !props.initial?.hasPassword) {
    error.value = '填写了用户名就要填一次密码'
    return null
  }
  return {
    protocol: protocol.value,
    host: host.value.trim(),
    port: portNumber,
    username: user ? user : null,
    // Empty means "keep the password already held for this session".
    password: password.value ? password.value : null,
  }
}

async function onSave(): Promise<void> {
  error.value = null
  const config = payload()
  if (!config) return
  if (!isTauri()) {
    error.value = '浏览器预览里没有后端，无法保存'
    return
  }
  busy.value = true
  try {
    const saved = await invoke<CustomProxyView>('save_custom_proxy', { config })
    emit('saved', saved)
  } catch (failure) {
    const shaped = failure as { message?: string } | null
    error.value = shaped?.message ?? (typeof failure === 'string' ? failure : '保存失败')
  } finally {
    busy.value = false
  }
}

onMounted(() => {
  // A stored password never comes back from the backend; the placeholder tells
  // the user that leaving the field empty keeps the session one.
  if (props.initial?.hasPassword) password.value = ''
})
</script>

<template>
  <section class="proxy ip-material ip-material--menu" role="dialog" aria-label="自定义代理" data-custom-proxy-root>
    <header class="proxy__head">
      <h2 class="proxy__title">自定义代理</h2>
      <button class="proxy__close" type="button" aria-label="关闭" @click="emit('close')">×</button>
    </header>

    <div class="proxy__row">
      <label class="proxy__field proxy__field--protocol">
        <span class="proxy__label">协议</span>
        <select v-model="protocol" class="proxy__control">
          <option v-for="item in PROTOCOLS" :key="item.value" :value="item.value">
            {{ item.label }}
          </option>
        </select>
      </label>
      <label class="proxy__field proxy__field--port">
        <span class="proxy__label">端口</span>
        <input v-model="port" class="proxy__control" inputmode="numeric" autocomplete="off" />
      </label>
    </div>

    <label class="proxy__field">
      <span class="proxy__label">主机</span>
      <input v-model="host" class="proxy__control" autocomplete="off" spellcheck="false" />
    </label>

    <div class="proxy__row">
      <label class="proxy__field">
        <span class="proxy__label">用户名（可选）</span>
        <input v-model="username" class="proxy__control" autocomplete="off" spellcheck="false" />
      </label>
      <label class="proxy__field">
        <span class="proxy__label">密码（可选）</span>
        <input
          v-model="password"
          class="proxy__control"
          type="password"
          autocomplete="off"
          :placeholder="initial?.hasPassword ? '留空即保持原密码' : ''"
        />
      </label>
    </div>

    <p class="proxy__hint">{{ endpointHint }} · 密码用 Windows DPAPI 加密保存</p>

    <p v-if="error" class="proxy__error" role="alert">{{ error }}</p>

    <div class="proxy__actions">
      <button class="proxy__button" type="button" :disabled="busy" @click="onSave">
        {{ busy ? '保存中…' : '保存并检测' }}
      </button>
    </div>
  </section>
</template>

<style scoped>
.proxy {
  display: flex;
  flex-direction: column;
  gap: 9px;
  width: 262px;
  padding: 14px 14px 12px;
}

.proxy__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.proxy__title {
  margin: 0;
  color: var(--ip-text);
  font-size: 12.5px;
  font-weight: 650;
  letter-spacing: 0.02em;
}

.proxy__close {
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--ip-text-secondary);
  font-size: 15px;
  line-height: 1;
  cursor: pointer;
}

.proxy__close:hover {
  background: var(--ip-surface-raised-hover);
  color: var(--ip-text);
}

.proxy__row {
  display: flex;
  gap: 8px;
}

.proxy__field {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.proxy__field--protocol {
  flex: 1.1;
}

.proxy__field--port {
  flex: 0.9;
}

.proxy__label {
  color: var(--ip-text-secondary);
  font-size: 9.5px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.proxy__control {
  box-sizing: border-box;
  min-width: 0;
  height: 28px;
  padding: 0 8px;
  border: 1px solid var(--ip-divider);
  border-radius: 9px;
  background: rgba(255, 255, 255, 0.6);
  color: var(--ip-text);
  font-family: inherit;
  font-size: 12px;
  font-weight: 550;
}

.proxy__control:focus-visible {
  border-color: var(--ip-accent-line);
  outline: 2px solid var(--ip-accent-tint);
  outline-offset: 0;
}

.proxy__hint {
  margin: 0;
  color: var(--ip-text-secondary);
  font-size: 10.5px;
  line-height: 1.35;
}

.proxy__error {
  margin: 0;
  color: #a3372c;
  font-size: 10.5px;
  line-height: 1.35;
}

.proxy__actions {
  display: flex;
  justify-content: flex-end;
}

.proxy__button {
  height: 28px;
  padding: 0 13px;
  border: 0;
  border-radius: 10px;
  background: #2a2c26;
  color: #f4f4ee;
  font-family: inherit;
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
}

.proxy__button:disabled {
  opacity: 0.6;
  cursor: default;
}
</style>
