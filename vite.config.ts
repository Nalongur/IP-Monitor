import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Tauri serves the dev build from a fixed port; fail loudly instead of
// silently picking another one and leaving the desktop window blank.
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Atomic-editor temp dirs can be locked mid-rename; watching them
      // crashes chokidar with EBUSY on Windows.
      ignored: ['**/src-tauri/**', '**/.*.tmpdir/**', '**/*.tmp'],
    },
  },
  build: {
    // Windows 11 WebView2 is Chromium-based; 105 matches the Tauri 2 baseline.
    target: 'chrome105',
    emptyOutDir: true,
    // Flags are imported as URLs. Inlining small ones would fold ~1.9 MB of SVG
    // into the JS bundle as base64 and delay the first paint of a card that only
    // needs one 18×13 image.
    assetsInlineLimit: 0,
  },
})
