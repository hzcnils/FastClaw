import { resolve } from 'node:path'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  css: { postcss: { plugins: [] } },
  build: {
    lib: {
      entry: resolve(__dirname, 'src/index.ts'),
      name: 'ChatRenderer',
      fileName: () => 'chat-renderer.js',
      formats: ['es'],
    },
    rollupOptions: {
      external: [
        'vue',
        'md-editor-v3',
        'md-editor-v3/lib/style.css',
        'highlight.js/lib/common',
        'highlight.js/styles/atom-one-dark.css',
        'katex',
        'katex/dist/katex.min.css',
        'mermaid',
      ],
      output: {
        globals: { vue: 'Vue' },
        assetFileNames: assetInfo => assetInfo.name?.endsWith('.css')
          ? 'chat-renderer.css'
          : 'assets/[name]-[hash][extname]',
      },
    },
    cssCodeSplit: false,
  },
})
