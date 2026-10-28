import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'

export default defineConfig({
  plugins: [vue()],
  build: {
    rollupOptions: {
      input: {
        quickbar: resolve(__dirname, 'quickbar.html'),
        panel: resolve(__dirname, 'panel.html'),
        settings: resolve(__dirname, 'settings.html'),
        history: resolve(__dirname, 'history.html'),
        glossary: resolve(__dirname, 'glossary.html'),
        snip: resolve(__dirname, 'snip.html'),
      },
    },
  },
  clearScreen: false,
  server: { port: 5173, strictPort: true },
})
