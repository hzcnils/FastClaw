import { defineConfig, type Plugin } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'
import { readFileSync, writeFileSync, existsSync } from 'fs'
import tailwindcss from '@tailwindcss/vite'
import Icons from 'unplugin-icons/vite'

/**
 * 选中图片时浮动批注按钮不出现：库内 an() 只查 span[data-pm-*]，
 * 图片锚点在 img[data-pm-start] 上。补上通用 [data-pm-start] 回退。
 */
function patchDocxFloatingCommentAnchor(): Plugin {
  const MARK = '/* fastclaw:floating-comment-img-anchor */'
  const TARGET = resolve(__dirname, 'node_modules/@eigenpal/docx-editor-vue/dist/index.js')
  const OLD = `function an(e, t, n) {
	let r = n ? Array.from(e.querySelectorAll(\`.layout-page-\${n} span[data-pm-start][data-pm-end]\`)) : Se(e);
	for (let e of r) {
		let n = Number(e.dataset.pmStart), r = Number(e.dataset.pmEnd);
		if (!isNaN(n) && !isNaN(r) && t >= n && t <= r) return e;
	}
	return null;
}`
  const NEW = `function an(e, t, n) {
	${MARK}
	let r = n ? Array.from(e.querySelectorAll(\`.layout-page-\${n} span[data-pm-start][data-pm-end]\`)) : Se(e);
	for (let e of r) {
		let n = Number(e.dataset.pmStart), r = Number(e.dataset.pmEnd);
		if (!isNaN(n) && !isNaN(r) && t >= n && t <= r) return e;
	}
	let a = e.querySelector(\`[data-pm-start="\${t}"]\`);
	if (a) return a;
	for (let e of Array.from(e.querySelectorAll("[data-pm-start][data-pm-end]"))) {
		let n = Number(e.dataset.pmStart), r = Number(e.dataset.pmEnd);
		if (!isNaN(n) && !isNaN(r) && t >= n && t <= r) return e;
	}
	return null;
}`
  const apply = () => {
    if (!existsSync(TARGET)) return
    const code = readFileSync(TARGET, 'utf8')
    if (code.includes(MARK)) return
    if (!code.includes(OLD)) return
    writeFileSync(TARGET, code.replace(OLD, NEW), 'utf8')
  }
  return {
    name: 'patch-docx-floating-comment-anchor',
    enforce: 'pre',
    buildStart: apply,
    configureServer() {
      apply()
    },
  }
}

/**
 * CommentCard 收起后再展开会残留 Delete 菜单（内部 a 未重置）。
 * 在 expanded=false 时清掉菜单开关。
 */
function patchDocxCommentMenuReset(): Plugin {
  const MARK = '/* fastclaw:comment-menu-reset */'
  const TARGET = resolve(
    __dirname,
    'node_modules/@eigenpal/docx-editor-vue/dist/usePortalClass-HM-UHRlE.js',
  )
  const OLD = `		let r = t, i = n, a = S(!1);
		function u() {
			a.value = !1, i("delete", r.comment.id);
		}`
  const NEW = `		let r = t, i = n, a = S(!1);
		${MARK}
		A(() => t.expanded, (e) => {
			e || (a.value = !1);
		});
		function u() {
			a.value = !1, i("delete", r.comment.id);
		}`
  const apply = () => {
    if (!existsSync(TARGET)) return
    const code = readFileSync(TARGET, 'utf8')
    if (code.includes(MARK)) return
    if (!code.includes(OLD)) return
    writeFileSync(TARGET, code.replace(OLD, NEW), 'utf8')
  }
  return {
    name: 'patch-docx-comment-menu-reset',
    enforce: 'pre',
    buildStart: apply,
    configureServer() {
      apply()
    },
  }
}

/**
 * 插入→表格尺寸网格：原实现依赖 mouseenter 写入行列，未悬停成功时点击无反应。
 * 改为 mousedown 直接按单元格 r/c 插入（与工具栏 TableGridPicker 一致）。
 */
function patchDocxTableGridInlineClick(): Plugin {
  const MARK = '/* fastclaw:table-grid-inline-click */'
  const TARGET = resolve(
    __dirname,
    'node_modules/@eigenpal/docx-editor-vue/dist/usePortalClass-HM-UHRlE.js',
  )
  const OLD = `		function d() {
			o.value > 0 && s.value > 0 && r("insert", o.value, s.value);
		}
		return (t, n) => (y(), c("div", null, [l("div", {
			class: "docx-table-grid-inline__grid",
			style: g({ gridTemplateColumns: \`repeat(\${e.gridColumns}, 18px)\` }),
			role: "grid",
			"aria-label": "Table size selector",
			onMouseleave: n[0] ||= (e) => {
				o.value = 0, s.value = 0;
			}
		}, [(y(!0), c(i, null, C(u.value, (e) => (y(), c("div", {
			key: \`\${e.r}-\${e.c}\`,
			class: h(["docx-table-grid-inline__cell", { "docx-table-grid-inline__cell--active": e.r <= o.value && e.c <= s.value }]),
			role: "gridcell",
			"aria-selected": e.r <= o.value && e.c <= s.value,
			onMouseenter: (t) => {
				o.value = e.r, s.value = e.c;
			},
			onClick: P(d, ["prevent"])
		}, null, 42, ai))), 128))], 36), l("div", oi, E(o.value > 0 && s.value > 0 ? \`\${s.value} × \${o.value}\` : "Select size"), 1)]));`
  const NEW = `		function d(e, t) {
			e > 0 && t > 0 && r("insert", e, t);
		}
		${MARK}
		return (t, n) => (y(), c("div", null, [l("div", {
			class: "docx-table-grid-inline__grid",
			style: g({ gridTemplateColumns: \`repeat(\${e.gridColumns}, 18px)\` }),
			role: "grid",
			"aria-label": "Table size selector",
			onMouseleave: n[0] ||= (e) => {
				o.value = 0, s.value = 0;
			}
		}, [(y(!0), c(i, null, C(u.value, (e) => (y(), c("div", {
			key: \`\${e.r}-\${e.c}\`,
			class: h(["docx-table-grid-inline__cell", { "docx-table-grid-inline__cell--active": e.r <= o.value && e.c <= s.value }]),
			role: "gridcell",
			"aria-selected": e.r <= o.value && e.c <= s.value,
			onMouseenter: (t) => {
				o.value = e.r, s.value = e.c;
			},
			onMousedown: P((t) => {
				d(e.r, e.c);
			}, ["prevent", "stop"])
		}, null, 42, ai))), 128))], 36), l("div", oi, E(o.value > 0 && s.value > 0 ? \`\${s.value} × \${o.value}\` : "Select size"), 1)]));`
  const apply = () => {
    if (!existsSync(TARGET)) return
    const code = readFileSync(TARGET, 'utf8')
    if (code.includes(MARK)) return
    if (!code.includes(OLD)) return
    writeFileSync(TARGET, code.replace(OLD, NEW), 'utf8')
  }
  return {
    name: 'patch-docx-table-grid-inline-click',
    enforce: 'pre',
    buildStart: apply,
    configureServer() {
      apply()
    },
  }
}

/** 依赖爬取卡住时 Vite 默认会挂起 index.html，Tauri WebView 一直白屏；直接吐出入口 HTML */
function serveIndexWithoutCrawl(): Plugin {
  const html = `<!DOCTYPE html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>FastClaw</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/@vite/client"></script>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>`
  return {
    name: 'serve-index-without-crawl',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const url = req.url?.split('?')[0]
        if (url !== '/' && url !== '/index.html') {
          next()
          return
        }
        res.statusCode = 200
        res.setHeader('Content-Type', 'text/html; charset=utf-8')
        res.setHeader('Cache-Control', 'no-cache')
        res.end(html)
      })
    },
  }
}

/** Rust 目录由 `tauri dev` 监视；Vite 再盯会把 SQLite/日志当成前端改动并整页刷新 */
function ignoreTauriDirWatch(): Plugin {
  return {
    name: 'ignore-tauri-dir-watch',
    apply: 'serve',
    configureServer(server) {
      const tauriDir = resolve(__dirname, 'src-tauri')
      const unwatch = () => {
        server.watcher.unwatch(tauriDir)
        server.watcher.unwatch('**/src-tauri/**')
      }
      unwatch()
      server.httpServer?.once('listening', unwatch)
    },
  }
}

/** CJS 包缺 ESM default：@eigenpal 依赖链补 export default */
function cjsDefaultExportInterop(): Plugin {
  const targets = [
    /\/docxtemplater\/js\/docxtemplater\.js$/,
    /\/pizzip\/(js\/)?index\.js$/,
    /\/xml-js\/lib\/.+\.js$/,
  ]
  return {
    name: 'cjs-default-export-interop',
    enforce: 'pre',
    transform(code, id) {
      const norm = id.split('?')[0].replace(/\\/g, '/')
      if (!targets.some((re) => re.test(norm))) return
      if (/\bexport\s+default\b/.test(code)) return
      // 含 require 的文件必须交给 Vite/Rollup commonjs 转换。
      // 若这里强行补 export default，会被当成 ESM，require() 会残留到生产包。
      if (/\brequire\s*\(/.test(code)) return
      // 仅有 module.exports、无 require 的小文件（如 xml-js/array-helper）：
      // 不能直接 `export default module.exports`，浏览器没有 module → module is not defined。
      // 用 IIFE 注入 module/exports 后再导出。
      if (code.includes('module.exports') || /(?:^|[\n;])\s*exports\s*[=.]/.test(code)) {
        return {
          code: [
            'const __cjsModule = { exports: {} };',
            '(function (module, exports) {',
            code,
            '})(__cjsModule, __cjsModule.exports);',
            'export default __cjsModule.exports;',
          ].join('\n'),
          map: null,
        }
      }
    },
  }
}

/** 把 xml-js / docxtemplater / pizzip 包装成带 default 的 ESM（浏览器侧，不用 createRequire） */
function cjsPackageDefaultShims(): Plugin {
  const pkgs: Record<string, string> = {
    'xml-js': 'xml-js/lib/index.js',
    docxtemplater: 'docxtemplater/js/docxtemplater.js',
    pizzip: 'pizzip/js/index.js',
  }
  return {
    name: 'cjs-package-default-shims',
    enforce: 'pre',
    resolveId(id) {
      if (pkgs[id]) return `\0cjs-shim:${id}`
    },
    load(id) {
      if (!id.startsWith('\0cjs-shim:')) return
      const name = id.slice('\0cjs-shim:'.length)
      const rel = pkgs[name]
      if (!rel) return
      // xml-js 的 index.js 是相对 require 聚合入口；生产打包时容易残留 require。
      // 改为分别导入叶子模块（仍由 Vite commonjs 转换），再组装 API。
      if (name === 'xml-js') {
        const base = resolve(__dirname, 'node_modules/xml-js/lib').replace(/\\/g, '/')
        return `
          import * as __xml2js from ${JSON.stringify(`${base}/xml2js.js`)};
          import * as __xml2json from ${JSON.stringify(`${base}/xml2json.js`)};
          import * as __js2xml from ${JSON.stringify(`${base}/js2xml.js`)};
          import * as __json2xml from ${JSON.stringify(`${base}/json2xml.js`)};
          function __pick(ns) {
            const m = ns && (ns.default ?? ns);
            return m && typeof m === 'object' && 'default' in m && m.default != null ? m.default : m;
          }
          export const xml2js = __pick(__xml2js);
          export const xml2json = __pick(__xml2json);
          export const js2xml = __pick(__js2xml);
          export const json2xml = __pick(__json2xml);
          const api = { xml2js, xml2json, js2xml, json2xml };
          export default api;
        `
      }
      const abs = resolve(__dirname, 'node_modules', rel).replace(/\\/g, '/')
      return `
        import * as __ns from ${JSON.stringify(abs)};
        const __mod = __ns.default ?? __ns;
        const api = __mod && __mod.default ? __mod.default : __mod;
        export default api;
      `
    },
  }
}

export default defineConfig(({ command }) => ({
  plugins: [
    serveIndexWithoutCrawl(),
    ignoreTauriDirWatch(),
    patchDocxFloatingCommentAnchor(),
    patchDocxCommentMenuReset(),
    patchDocxTableGridInlineClick(),
    cjsPackageDefaultShims(),
    cjsDefaultExportInterop(),
    vue(),
    tailwindcss(),
    Icons({ compiler: 'vue3' }),
  ],
  optimizeDeps: {
    // include @eigenpal 整条链；CJS 子依赖另由 cjsPackageDefaultShims 兜底
    // floating-comment-img-anchor：强制重预构建以吃上 an() 补丁
    include: [
      '@eigenpal/docx-editor-vue',
      '@eigenpal/docx-editor-core',
      '@eigenpal/docx-editor-i18n',
      'docxtemplater',
      'pizzip',
      'jszip',
      'xml-js',
      'dompurify',
    ],
    exclude: [],
    needsInterop: ['docxtemplater', 'pizzip', 'xml-js', 'jszip'],
    holdUntilCrawlEnd: false,
  },
  resolve: {
    alias: [
      { find: '@', replacement: resolve(__dirname, 'src') },
      { find: 'tauri-download-center-core', replacement: resolve(__dirname, 'download-packages/packages/tauri-download-center-core/src/index.ts') },
      { find: 'tauri-download-center-vue', replacement: resolve(__dirname, 'download-packages/packages/tauri-download-center-vue/src/index.ts') },
      ...(command === 'serve' ? [
        { find: /^chat-renderer$/, replacement: resolve(__dirname, 'chat-plan-packages/chat-renderer/src/index.ts') },
        { find: /^chat-renderer\/style\.css$/, replacement: resolve(__dirname, 'chat-plan-packages/chat-renderer/src/style.css') },
        { find: /^planflow-runtime$/, replacement: resolve(__dirname, 'chat-plan-packages/fastplan-runtime/src/index.ts') },
        { find: /^planflow-planning-ui$/, replacement: resolve(__dirname, 'chat-plan-packages/fastplan-planning-ui/src/index.js') },
        { find: /^planflow-planning-ui\/style\.css$/, replacement: resolve(__dirname, 'chat-plan-packages/fastplan-planning-ui/src/style.css') },
        { find: /^planflow-planning-ui\/taskDisplay$/, replacement: resolve(__dirname, 'chat-plan-packages/fastplan-planning-ui/src/utils/taskDisplay.js') },
        { find: /^planflow-kit$/, replacement: resolve(__dirname, 'chat-plan-packages/fastplan/src/index.ts') },
        { find: /^planflow-kit\/style\.css$/, replacement: resolve(__dirname, 'chat-plan-packages/fastplan/src/style.css') },
      ] : []),
    ],
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: [],
  },
  publicDir: 'public',
  clearScreen: false,
  server: {
    port: 2420,
    strictPort: true,
    preTransformRequests: false,
    watch: {
      // 整棵 src-tauri 都不要盯：开发库 WAL、日志、sidecar 复制都会误触发 WebView 整页刷新
      ignored: ['**/src-tauri/**', '**/data/**', '**/.tmp/**'],
    },
    proxy: {
      '/FastPlan/': {
        target: 'http://127.0.0.1:5179',
        changeOrigin: true,
      },
      '/api/': {
        target: 'http://127.0.0.1:5179',
        changeOrigin: true
      },
      '/fastkg': {
        // 经 FastClaw 动态代理访问，避免 FastKG 端口自动避让后仍直连旧默认端口。
        target: 'http://127.0.0.1:5179',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/fastkg/, '/api/v1'),
        headers: {
          'X-FastKG-Service-Key': 'fastkg-service-key',
        },
      },
      '/fastciApi/': {
        target: 'http://127.0.0.1:5179',
        changeOrigin: true,
      },
    }
  },
  envPrefix: ['VITE_', 'TAURI_'],
  css: {
    preprocessorOptions: {
      scss: {
        api: 'modern-compiler',
        silenceDeprecations: ['legacy-js-api'],
      },
    },
  },
  build: {
    target: ['es2021', 'chrome100', 'safari13'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    commonjsOptions: {
      include: [/node_modules/],
      transformMixedEsModules: true,
    },
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        screenshot: resolve(__dirname, 'screenshot.html'),
      },
    },
  },
}))
