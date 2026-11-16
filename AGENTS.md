# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Project Overview

FastClaw is a Tauri v2 desktop AI assistant. The public edition exposes chat, knowledge management, and a model platform containing model, tool, Skills, MCP, WSP, and usage-dashboard pages. Chinese-language UI (Element Plus with zh-cn locale).

## Commands

```bash
# Frontend dev server (Vite on port 2420)
npm run dev

# Type-check and build frontend
npm run build

# Tauri dev (full desktop app, release mode)
npm run tauri:dev

# Tauri dev with optional features (FastPlan + FastKg, release mode)
npm run tauri:dev -- --features full

# Rust: type-check backend
cargo check --manifest-path src-tauri/Cargo.toml
```

The Vite dev server proxies `/api/` and `/FastPlan/` to `http://127.0.0.1:5179` (the embedded Rust HTTP server).

## Architecture

```
src/                     # Vue 3 frontend (TypeScript)
  main.ts                # Entry: createApp → Pinia → Router → ElementPlus → mount
  router.ts              # ~50 routes, all lazy-loaded via import()
  App.vue                # Shell layout: title bar + sidebar (TreeMenu) + <router-view> with keep-alive
  stores/                # Pinia stores (one per domain: chat, agents, planning, settings, etc.)
  components/            # Reusable components across views
    common/              # TreeMenu, TabBar, Modal, Drawer, ToastContainer, etc.
    chat/                # ChatInput, MessageBubble, ModelSelector, etc.
    planning/            # AgentTaskCard, ExecutionDagPanel, PlanPreview, etc.
  views/                 # Page-level components (one directory per feature area)
    agent/               # AgentList, SaveDialog, tinyflow (workflow editor), appconfig
    knowledge/           # KnowledgeBaseList, KnowledgeBaseDetail
    mcp/                 # McpManager, McpStore
    skills/              # SkillManager, SkillStore
  styles/global.scss     # Global SCSS with CSS custom properties (theme system)

src-tauri/               # Rust backend (Tauri v2)
  src/
    main.rs              # App bootstrap: DB init, FastPlan/FastKg integration, HTTP server (axum on :5179)
    commands/            # Tauri command handlers (one module per domain: chat, models, agents, etc.)
                         # Each is #[tauri::command] — frontend calls them via invoke()
    core/                # Business logic layer
      llm/               # LlmManager: model resolution, chat/completions, SSE streaming via Tauri events
      wsp/               # WSP service lifecycle manager (start/stop/health-check external Python/Node processes)
      plan/              # FastPlan integration (planning/execution)
      mcp/               # MCP client (stdio/SSE transport)
      skills/            # Skill registry and execution
    db/
      mod.rs             # Database struct: sqlx SqlitePool, WAL mode, migrations from ./migrations/
      repo/              # Data access layer — one repo file per domain (agents, knowledge, models, etc.)
    http_server.rs       # Embedded axum HTTP server (REST API for file upload, knowledge CRUD, search, Shimmy proxy)
    state.rs             # AppState: aggregates Database + LlmManager + WspManager + MCP + Skills + Plan + Kg
    migrations/          # SQL migration files (sqlx)
```

### Key architectural patterns

- **Frontend ↔ Backend communication**: The Vue frontend calls Rust via two channels:
  1. `invoke()` (Tauri IPC) for most operations — each command is registered in `main.rs::generate_handler![]`
  2. `http://127.0.0.1:5179/api/v1/...` (axum REST API) for knowledge base CRUD, file upload, search — the frontend uses `axios` in `src/types/knowledge.ts`

- **Streaming chat**: The backend's `chat_stream` method emits Tauri events (`chat-stream-token`, `chat-stream-done`, `chat-stream-error`) which the frontend listens to via `@tauri-apps/api/event.listen()`. The `invoke('send_message_stream')` returns immediately; actual tokens arrive as events.

- **Feature-gated modules**: `with-fastplan` and `with-fastkg` are compile-time Cargo features. Code behind `#[cfg(feature = "...")]` is conditionally compiled.

- **Environment injection**: On startup, `main.rs` reads LLM/embedding config from the SQLite database and injects it into env vars (`FASTCLAW_LLM_BASE_URL`, `FASTCLAW_LLM_MODEL`, `FASTCLAW_EMBEDDING_URL`, etc.) so downstream processes (FastPlan, FastKg) can pick them up.

- **Theme system**: CSS custom properties (e.g. `--bg-primary`, `--text-primary`) scoped to 6 themes (Blue, Green, Grey, Orange, Purple, Red). Each theme has a background image in `src/assets/theme/`. Theme state is managed by `src/stores/theme.ts` and persisted to localStorage.

- **TreeMenu**: The public sidebar is defined locally and exposes only chat, knowledge management, and the model-platform group. Existing model-platform child routes remain available.

- **WSP services**: External microservices managed as subprocesses. Each service has a port, protocol, and auto-start configuration; public navigation retains only the WSP administration page.

## Key dependencies

- **Frontend**: Vue 3, Pinia, Vue Router, Element Plus (zh-cn), Tailwind CSS 3, Vite 5, Tiptap, CodeMirror, ECharts, Mermaid, KaTeX, markdown-it, highlight.js
- **Backend**: Tauri 2, sqlx 0.8 (SQLite), axum 0.7, reqwest 0.12, tokio (full), keyring 3

## File naming notes

- `src/plan/` — appears to be an older/integrated copy of FastPlan frontend code; `src/components/planning/` is the actively used location
- `src-tauri/src/main copy.rs` — a backup, not compiled
- `src/types/knowledge.ts` and several knowledge-related Vue files are excluded from `vue-tsc` type checking (listed in `tsconfig.json` exclude)

## Publishing to Gitee / GitHub

Two public mirrors are configured as remotes:

- `github` → `git@github.com:hzcnils/FastClaw.git` (SSH; branch `main`)
- `gitee` → `https://gitee.com/hzcnils/fastclaw.git` (HTTPS + Git Credential Manager; branch `master`)

**Publishing rule: the public mirrors contain a single parentless `Initial commit` — never push the local development history.**

After every fix, commit locally and publish the current `HEAD` tree as a fresh orphan snapshot to both mirrors:

```bash
git add -A && git commit -m "<what changed>"

tree=$(git rev-parse HEAD^{tree})
snapshot=$(printf 'Initial commit\n' | git commit-tree "$tree")

git fetch github main
git fetch gitee master
git push github "$snapshot:refs/heads/main" --force-with-lease
git push gitee  "$snapshot:refs/heads/master" --force-with-lease
```

Notes:

- Never put tokens in the repository or in `.git/config`. Gitee authentication comes from Git Credential Manager (account `hzcnils`); GitHub uses the local SSH key (`~/.ssh/id_ed25519`).
- `.gitattributes` marks Vue/TS/JS/CSS/HTML as `-linguist-detectable` so GitHub Linguist reports **Rust** as the primary language. Do not remove those rules.
