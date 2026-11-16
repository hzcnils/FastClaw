import { readFileSync } from 'node:fs'

const outputUrl = new URL('../dist/chat-renderer.js', import.meta.url)
const output = readFileSync(outputUrl, 'utf8')

const requiredMarkers = [
  'highlight.js/lib/common',
  'from "katex"',
  'from "mermaid"',
  'editorExtensions',
  'enableZoom',
]

const missing = requiredMarkers.filter(marker => !output.includes(marker))
if (missing.length) {
  throw new Error(`chat-renderer production bundle is missing local Markdown extension configuration: ${missing.join(', ')}`)
}

console.log('Verified local Highlight.js, KaTeX, and Mermaid configuration in dist/chat-renderer.js')
