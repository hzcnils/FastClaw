import hljs from 'highlight.js/lib/common'
import katex from 'katex'
import mermaid from 'mermaid'
import { config } from 'md-editor-v3'

let configured = false

/**
 * Register packaged Markdown extensions before MdPreview renders.
 * The explicit call keeps this module in production bundles even when the
 * package is marked side-effect-free for JavaScript tree shaking.
 */
export function ensureMarkdownExtensions() {
  if (configured) return

  // Package modules replace host-specific /static resource URLs and CDN access.
  config({
    // This package is the shared renderer for planning logs and node output.
    // Never install md-editor-v3's executable ECharts fence plugin here. Final
    // answers extract validated chart options before their text reaches this
    // renderer and render them with FastPlanEchartsBlock instead.
    markdownItPlugins: plugins => plugins.filter(plugin => plugin.type !== 'echarts'),
    editorExtensions: {
      highlight: { instance: hljs },
      katex: { instance: katex },
      mermaid: { instance: mermaid, enableZoom: true },
    },
    editorConfig: {
      languageUserDefined: {
        'zh-CN': {
          copyCode: {
            text: '复制',
            successTips: '已复制！',
            failTips: '复制失败！',
          },
        },
      },
    },
  })

  configured = true
}
