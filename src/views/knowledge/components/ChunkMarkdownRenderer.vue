<script setup lang="ts">
import { computed, watch, ref, nextTick, onMounted } from 'vue';
import MarkdownIt from 'markdown-it';
import mk from '@traptitech/markdown-it-katex';
// 按需引入 highlight.js core + 常用语言(此前全量引入 ~190 语言 ~900KB)。
// 未注册语言由下方 highlight() 的 `hljs.getLanguage(lang)` 判空自动降级为转义纯文本。
import hljs from 'highlight.js/lib/core';
import javascript from 'highlight.js/lib/languages/javascript';
import typescript from 'highlight.js/lib/languages/typescript';
import python from 'highlight.js/lib/languages/python';
import bash from 'highlight.js/lib/languages/bash';
import json from 'highlight.js/lib/languages/json';
import sql from 'highlight.js/lib/languages/sql';
import rust from 'highlight.js/lib/languages/rust';
import go from 'highlight.js/lib/languages/go';
import java from 'highlight.js/lib/languages/java';
import xml from 'highlight.js/lib/languages/xml';
import yaml from 'highlight.js/lib/languages/yaml';
import markdown from 'highlight.js/lib/languages/markdown';
import css from 'highlight.js/lib/languages/css';
import cpp from 'highlight.js/lib/languages/cpp';
[
  ['javascript', javascript], ['typescript', typescript], ['python', python],
  ['bash', bash], ['json', json], ['sql', sql], ['rust', rust], ['go', go],
  ['java', java], ['xml', xml], ['yaml', yaml], ['markdown', markdown],
  ['css', css], ['cpp', cpp],
].forEach(([name, lang]) => hljs.registerLanguage(name as string, lang as any));
import renderMathInElement from 'katex/dist/contrib/auto-render';
import 'katex/dist/katex.min.css';
import { sanitizeHtml } from '@/utils/display';

const props = defineProps<{
  content: string;
  class?: string;
  imageBaseUrl?: string;
}>();

const containerRef = ref<HTMLElement | null>(null);

const md: MarkdownIt = new MarkdownIt({
  html: true,
  breaks: true,
  linkify: true,
  typographer: false,
  highlight: function (str: string, lang: string): string {
    if (lang && hljs.getLanguage(lang)) {
      try { return `<pre class="hljs"><code>${hljs.highlight(str, { language: lang, ignoreIllegals: true }).value}</code></pre>`; }
      catch { /* fall through */ }
    }
    return `<pre class="hljs"><code>${md.utils.escapeHtml(str)}</code></pre>`;
  },
});

md.use(mk, { throwOnError: false, errorColor: '#ef4444' });

// 图片 — CDN 拼接 + 失败占位
md.renderer.rules.image = (tokens: any[], idx: number) => {
  const token = tokens[idx];
  let src = token.attrGet('src') || '';
  if (props.imageBaseUrl && src && !/^https?:\/\//i.test(src) && !/^\//.test(src)) {
    src = `${props.imageBaseUrl}/${src}`;
  }
  const alt = token.attrGet('alt') || '';
  const title = token.attrGet('title') ? ` title="${token.attrGet('title')}"` : '';
  return `<img src="${src}" alt="${alt}"${title} loading="lazy"
    onerror="this.outerHTML='<div style=\\'display:flex;align-items:center;justify-content:center;height:80px;background:#f3f4f6;border-radius:6px;color:#9ca3af;font-size:12px\\'>🖼 无法加载</div>'" />`;
};

const renderedHtml = computed(() => {
  if (!props.content) return '';
  try {
    let mdContent = props.content
      .replace(/\\\[/g, '\n$$\n').replace(/\\\]/g, '\n$$\n')
      .replace(/\\\(/g, '$').replace(/\\\)/g, '$');

    // 保护 HTML 标签，防止 inline $ 在 table 属性中被误解析
    const htmlTags: string[] = [];
    mdContent = mdContent.replace(/<(\/?[a-zA-Z][^>]*)>/g, (match) => {
      htmlTags.push(match);
      return `\x01T${htmlTags.length - 1}\x01`;
    });

    let html = md.render(mdContent);
    html = html.replace(/\x01T(\d+)\x01/g, (_m: string, idx: string) => htmlTags[parseInt(idx)] || '');

    // 净化不可信文档 HTML(markdown-it html:true 会透传原文 HTML)→ 防存储型 XSS。
    // KaTeX 在挂载后对 DOM 里的 $ 定界符渲染,不受此处净化影响。
    return sanitizeHtml(html);
  } catch { return '<p style="color:#ef4444">渲染错误</p>'; }
});

function renderMath() {
  if (!containerRef.value) return;
  try {
    renderMathInElement(containerRef.value, {
      delimiters: [
        { left: '$$', right: '$$', display: true },
        { left: '$', right: '$', display: false },
        { left: '\\(', right: '\\)', display: false },
        { left: '\\[', right: '\\]', display: true },
      ],
      throwOnError: false,
      ignoredTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code'],
    });
  } catch (e) { console.warn('KaTeX auto-render error:', e); }
}

watch(renderedHtml, () => nextTick(renderMath));
onMounted(() => nextTick(renderMath));
</script>

<template>
  <div ref="containerRef" class="md-body" :class="props.class" v-html="renderedHtml"></div>
</template>

<style>
/* ===================================================================
   MinerU 风格 Markdown — 白底黑字，干净排版
   =================================================================== */
.md-body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
  font-size: var(--font-size-md, 15px);
  line-height: 1.85;
  color: var(--text-primary, #1a1a1a);
  background: transparent;
  word-wrap: break-word;
}

/* ---- 标题 ---- */
.md-body h1 { font-size: 22px; font-weight: 700; margin: 0 0 16px; padding-bottom: 8px; border-bottom: 1px solid var(--border-color, #e5e7eb); color: var(--text-primary, #111827); }
.md-body h2 { font-size: 18px; font-weight: 600; margin: 28px 0 12px; color: var(--text-primary, #111827); }
.md-body h3 { font-size: 16px; font-weight: 600; margin: 22px 0 10px; color: var(--text-primary, #111827); }
.md-body h4 { font-size: 15px; font-weight: 600; margin: 18px 0 8px; color: var(--text-primary, #1f2937); }
.md-body h5, .md-body h6 { font-size: 14px; font-weight: 600; margin: 14px 0 6px; color: var(--text-secondary, #374151); }

/* ---- 段落 ---- */
.md-body p { margin: 0 0 12px; }

/* ---- 粗体/斜体/删除线 ---- */
.md-body strong { font-weight: 700; color: var(--text-primary, #111827); }
.md-body em { font-style: italic; }
.md-body del { text-decoration: line-through; color: var(--text-secondary, #9ca3af); }

/* ---- 链接 ---- */
.md-body a { color: #2563eb; text-decoration: underline; text-underline-offset: 2px; }
.md-body a:hover { color: #1d4ed8; }

/* ---- 列表 ---- */
.md-body ul, .md-body ol { padding-left: 24px; margin: 8px 0 12px; }
.md-body li { margin: 4px 0; }
.md-body ul { list-style-type: disc; }
.md-body ol { list-style-type: decimal; }

/* ---- 表格 ---- */
.md-body table { border-collapse: collapse; width: 100%; margin: 16px 0; font-size: 14px; }
.md-body thead { background: #f3f4f6; }
.md-body th, .md-body td { border: 1px solid #d1d5db; padding: 8px 12px; text-align: left; }
.md-body th { font-weight: 600; color: #111827; }
.md-body td { color: #1a1a1a; }
.md-body tbody tr:nth-child(even) td { background: #f9fafb; }

/* ---- 代码（分块查看场景避免大块 #1e1e1e 黑底；外层 chunk-viewer 再强制透明） ---- */
.md-body code { background: color-mix(in srgb, var(--text-primary, #111) 8%, transparent); padding: 2px 6px; border-radius: 3px; font-size: 13px; color: var(--color-danger, #e11d48); font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace; }
.md-body pre { background: rgba(255, 255, 255, 0.06); padding: 16px; border-radius: 6px; overflow-x: auto; margin: 16px 0; line-height: 1.6; border: 1px solid var(--border-color, #e5e7eb); }
.md-body pre code { background: none; color: var(--text-primary, #d4d4d4); font-size: 13px; padding: 0; border-radius: 0; }

/* ---- 图片 ---- */
.md-body img { max-width: 100%; height: auto; display: block; margin: 12px auto; border-radius: 4px; }

/* ---- 引用块 ---- */
.md-body blockquote { border-left: 4px solid #3b82f6; padding: 8px 16px; margin: 12px 0; background: #eff6ff; color: #1e40af; }
.md-body blockquote p { margin: 0.25em 0; }

/* ---- 分割线 ---- */
.md-body hr { border: none; border-top: 1px solid #e5e7eb; margin: 20px 0; }

/* ---- KaTeX ---- */
.md-body .katex-display { margin: 16px 0; overflow-x: auto; overflow-y: hidden; }
.md-body .katex { font-size: 1.05em; }

/* ---- MinerU image <details> blocks (natural_image / text_image) ---- */
.md-body details {
  margin: 4px 0 12px;
  font-size: 0.8rem;
  color: #6b7280;
}
.md-body details summary {
  cursor: pointer;
  user-select: none;
  padding: 2px 0;
  color: #9ca3af;
  font-size: 0.75rem;
  list-style: none;
}
.md-body details summary::-webkit-details-marker { display: none; }
.md-body details summary::before { content: '⊞ '; }
.md-body details[open] summary::before { content: '⊟ '; }
.md-body details p { margin: 2px 0 0; font-size: 0.8rem; color: #6b7280; font-style: italic; }

/* ---- 滚动条 ---- */
.md-body pre::-webkit-scrollbar { height: 6px; }
.md-body pre::-webkit-scrollbar-track { background: transparent; }
.md-body pre::-webkit-scrollbar-thumb { background: #d1d5db; border-radius: 3px; }
</style>
