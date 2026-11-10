import { createApp } from 'vue'
import App from './App.vue'
import './styles/tokens.css'
import './styles/base.css'
import './styles/acrylic.css'
import { initAppSettings } from './composables/useAppSettings'

/**
 * Settings are read before mounting: the stored accent must be on `<html>`
 * before the first paint, otherwise the card flashes the default palette.
 */
async function bootstrap(): Promise<void> {
  await initAppSettings()
  createApp(App).mount('#app')
}

void bootstrap()
