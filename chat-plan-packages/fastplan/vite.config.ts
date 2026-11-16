import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'node:path'

export default defineConfig({
  plugins: [vue()],
  resolve: { alias: [
    { find: 'chat-renderer/style.css', replacement: resolve(__dirname, '../chat-renderer/dist/chat-renderer.css') },
    { find: 'planflow-planning-ui/style.css', replacement: resolve(__dirname, '../fastplan-planning-ui/dist/style.css') },
  ] },
  build: {
    lib: { entry: resolve(__dirname, 'src/index.ts'), name: 'PlanflowKit', fileName: 'planflow-kit', formats: ['es'] },
    rollupOptions: {
      external: ['vue', 'chat-renderer', 'planflow-runtime', 'planflow-planning-ui'],
      output: {
        assetFileNames: assetInfo => assetInfo.name?.endsWith('.css')
          ? 'style.css'
          : 'assets/[name]-[hash][extname]',
      },
    },
  },
})
