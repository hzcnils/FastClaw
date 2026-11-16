<script setup lang="ts">
import { MdPreview, config } from 'md-editor-v3'
import 'md-editor-v3/lib/style.css'
import { useThemeStore } from '@/stores/theme'

const themeStore = useThemeStore()
const props = withDefaults(defineProps<{
  content: string
  previewTheme?: string
  codeTheme?: string
  noKatex?: boolean
  noMermaid?: boolean
  customClass?: string
}>(), {
  content: '',
  previewTheme: 'github',
  codeTheme: 'atom',
  noKatex: false,
  noMermaid: false,
  customClass: ''
})
let path = '/static/md-editor-v3'
config({
  editorConfig: {
    languageUserDefined: {
      'zh-CN': {
        copyCode: {
          text: '复制',
          successTips: '已复制！',
          failTips: '复制失败！'
        }
      }
    }
  },
  editorExtensions: {
    /**
     * 1. 代码高亮 (Highlight.js)
     * js: 核心库
     * css: 将你下载的css文件映射为组件可识别的主题名
     */
    highlight: {
      js: `${path}/highlight.min.js`,
      css: {
        'github': {
          'dark': `${path}/github-dark.min.css`,
          'light': `${path}/github.min.css`,
        },
        'atom-one': {
          'dark': `${path}/atom-one-dark.min.css`,
          'light': `${path}/atom-one-light.min.css`,
        },
        'a11y': {
          'dark': `${path}/a11y-dark.min.css`,
          'light': `${path}/a11y-light.min.css`,
        },
        'kimbie': {
          'dark': `${path}/kimbie-dark.min.css`,
          'light': `${path}/kimbie-light.min.css`,
        },
        'paraiso': {
          'dark': `${path}/paraiso-dark.min.css`,
          'light': `${path}/paraiso-light.min.css`,
        },
        'qtcreator': {
          'dark': `${path}/qtcreator-dark.min.css`,
          'light': `${path}/qtcreator-light.min.css`,
        },
        'stackoverflow': {
          'dark': `${path}/stackoverflow-dark.min.css`,
          'light': `${path}/stackoverflow-light.min.css`,
        },
        'gradient': {
          'dark': `${path}/gradient-dark.min.css`,
          'light': `${path}/gradient-light.min.css`,
        },
      }
    },
    /**
     * 2. 图片裁剪 (Cropper.js)
     * 用于编辑器内的图片裁剪功能
     */
    cropper: {
      js: `${path}/cropper.min.js`,
      css: `${path}/cropper.min.css`,
    },
    /**
     * 3. 数学公式 (KaTeX)
     */
    katex: {
      js: `${path}/katex.min.js`,
      css: `${path}/katex.min.css`,
    },
    /**
     * 4. 流程图/时序图 (Mermaid)
     */
    mermaid: {
      js: `${path}/mermaid.min.js`,
    },
    /**
     * 5. 全屏功能 (Screenfull)
     */
    screenfull: {
      js: `${path}/screenfull.js`,
    },
    /**
     * 6. 代码格式化 (Prettier)
     * standalone.js 通常是 prettier 的 standalone 版本
     * markdown.js 是解析 markdown 的插件
     */
    prettier: {
      standaloneJs: `${path}/standalone.js`,
      parserMarkdownJs: `${path}/markdown.js`,
    },
  },
});
</script>

<template>
  <MdPreview :theme="themeStore.draftPreset.id == 'simple-light' ? 'light' : 'dark'" :modelValue="content" :previewTheme="previewTheme" :codeTheme="codeTheme" :noKatex="noKatex"
  :noMermaid="noMermaid" :noImgZoomIn="false" :auto-fold-threshold="10000" :class="customClass" />
</template>

<style lang="scss" scoped>
.md-editor {
  --md-bk-color: transparent !important;
  --md-text-color: var(--text-primary) !important;
  background-color: transparent !important;
  padding: 15px;
  border-radius: 8px;
}

/* 表格所有行背景统一，取消奇偶交替色 */
.md-editor table tr:nth-child(even) {
  background-color: transparent !important;
}

:deep(.md-editor-preview .md-editor-code .md-editor-code-head) {
  z-index: 2000;
  // background-color: transparent!important;
  border-bottom: 1px solid var(--border-color) !important;  /* 加这行 */
}
</style>

<style lang="scss">
/* AI回答文字大小跟随系统字体，默认14px时: h1=18px, h2=16px, 正文=14px */
/*普通文本字体大小 */
.md-editor-preview {
  font-size: 1rem !important;
}
.md-editor-preview h1 {
  font-size: 1.286rem !important;
}
/*一级标题字体大小 */
.md-editor-preview h2 {
  font-size: 1.286rem !important;
  margin: 15px 15px 15px  0px!important;
  padding-bottom: 15px !important;   /* 文字到分割线的间距 */
}
/*二级标题字体大小 */
.md-editor-preview h3 {
  font-size: 1.143rem !important;
  margin:15px 0px 15px  0px!important; /*上 → 右 → 下 → 左*/


}


.md-editor-preview p  { margin: 0px 0 !important; padding-bottom: 7px !important; 
padding-top: 0px !important; 
 }   /* 段落间距 */

.md-editor-preview ul,
.md-editor-preview ol { margin: 8px 0 !important; }    /* 列表间距 */
.md-editor-preview li { margin: 4px 0 !important; }    /* 列表项间距 */

/* 图片（含 AI 生成图 / 参考图）：限制最大尺寸，缩略图展示，点击可放大 */
.md-editor-preview img {
  max-width: min(100%, 460px) !important;
  max-height: 340px !important;
  width: auto !important;
  height: auto !important;
  border-radius: 10px;
  object-fit: contain;
  cursor: zoom-in;
}

.md-editor .md-editor-preview,
.md-editor .md-editor-preview {
  --md-theme-border-color: var(--border-color) !important;
  --md-theme-table-td-border-color: var(--border-color) !important;
  --md-theme-heading-1-border: 1px solid var(--border-color) !important;
  --md-theme-heading-2-border: 1px solid var(--border-color)!important;
  --md-theme-table-stripe-color: transparent !important;
  --md-theme-table-tr-bg-color: transparent !important;
  --md-theme-code-before-bg-color: transparent !important;
  --md-theme-code-block-bg-color:var(--border-color) !important;
}


// /* 情景主题（除雾灰调）：AI回答区域样式 */
// html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .md-editor,
// html.light[data-theme-id="custom-scene"] .md-editor {
//   --md-text-color: #ffffff !important;
//   color: #ffffff !important;
// }
// html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .md-editor-preview,
// html.light[data-theme-id="custom-scene"] .md-editor-preview {
//   color: #ffffff !important;
// }
// html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .md-editor-preview *,
// html.light[data-theme-id="custom-scene"] .md-editor-preview * {
//   color: #ffffff !important;
//   background: transparent !important;
// }
// /* 情景主题（除雾灰调）：分割线+表格边框白色20% */
// html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .md-editor .md-editor-preview,
// html.light[data-theme-id="custom-scene"] .md-editor .md-editor-preview {
//   --md-theme-border-color: rgba(255,255,255,0.20) !important;
//   --md-theme-table-td-border-color: rgba(255,255,255,0.20) !important;
//   --md-theme-heading-1-border: 1px solid rgba(255,255,255,0.20) !important;
//   --md-theme-heading-2-border: 1px solid rgba(255,255,255,0.20) !important;
//   --md-theme-table-stripe-color: transparent !important;
//   --md-theme-table-tr-bg-color: transparent !important;
//   --md-theme-code-before-bg-color: transparent !important;
//   --md-theme-code-block-bg-color: rgba(0,0,0,0.10) !important;
// }
// /* 情景主题（除雾灰调）：表格全部透明 */
// html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .md-editor-preview table th,
// html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .md-editor-preview table td,
// html.light[data-theme-id="custom-scene"] .md-editor-preview table th,
// html.light[data-theme-id="custom-scene"] .md-editor-preview table td,
// html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .md-editor-preview table tr {
//   background: transparent !important;
// }


</style>