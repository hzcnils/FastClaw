import { resolve } from 'node:path'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  build: {
    lib: {
      entry: resolve(__dirname, 'src/index.js'),
      name: 'PlanflowPlanningUi',
      fileName: () => 'planflow-planning-ui.js',
      formats: ['es'],
    },
    rollupOptions: {
      external: ['vue', 'element-plus', 'chat-renderer', 'planflow-runtime'],
      output: {
        globals: { vue: 'Vue' },
        assetFileNames: assetInfo => assetInfo.name?.endsWith('.css')
          ? 'style.css'
          : 'assets/[name]-[hash][extname]',
      },
    },
    cssCodeSplit: false,
  },
})
