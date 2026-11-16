import { resolve } from 'node:path'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  root: resolve(__dirname, 'demo'),
  plugins: [vue()],
  css: { postcss: { plugins: [] } },
  build: { outDir: resolve(__dirname, 'demo-dist'), emptyOutDir: true },
})
