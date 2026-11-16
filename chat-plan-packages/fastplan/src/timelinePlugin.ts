import { defineComponent, h, type Component } from 'vue'
import type { ChatRendererMessages, ChatWorkspacePlugins } from 'chat-renderer'
import type { FastPlanPlanningMessages } from 'planflow-planning-ui'
import type {
  FastPlanConfirmationPolicy,
  FastPlanExecutionController,
  FastPlanExecutionControllerOptions,
  FastPlanExecutionMode,
  FastPlanModelConfig,
} from 'planflow-runtime'
import type { FastPlanAccess } from 'planflow-runtime'
import FastPlanTimelineRenderer from './FastPlanTimelineRenderer.vue'
import type { FastPlanInteractionCommand } from './interaction'
import type { FastPlanDensity, FastPlanThemeMode, FastPlanThemeTokens } from './types'

export interface FastPlanTimelineCommand {
  id: string
  type: 'send' | 'stop' | 'pause' | 'resume' | 'retry-node' | 'skip-node' | 'refresh' | 'ensure-live' | 'confirm'
  prompt?: string
  attachments?: Array<Record<string, unknown> & { file?: File }>
  nodeId?: string
  thinking?: boolean
  metadata?: Record<string, unknown>
}

export interface FastPlanTimelineEntryData {
  sessionId?: string
  markerMessageId?: string
  prompt?: string
  /** Host-owned text shown before preflight checks permit a FastPlan send. */
  pendingPrompt?: string
  /** Host-owned attachment snapshot shown alongside a pending prompt. */
  pendingAttachments?: Array<Record<string, unknown> & { file?: File }>
  /** A preflight failure associated with `pendingPrompt`. */
  preflightError?: string
  createdAt?: number | string | Date
  connectionMode?: 'live' | 'history' | 'manual'
  executionMode?: FastPlanExecutionMode
  confirmationPolicy?: FastPlanConfirmationPolicy
  model?: string | FastPlanModelConfig
  command?: FastPlanTimelineCommand
  /** Host dispatches this command; mounting a renderer must not execute it again. */
  commandOwner?: 'host'
  uploadProgress?: { id: string; progress: number }
  metadata?: Record<string, unknown>
  /** Render a lightweight history summary until the user explicitly expands it. */
  deferLoad?: boolean
}

export interface FastPlanTimelinePluginOptions {
  access?: FastPlanAccess
  /** View-only model/provider gate for internal submit/replan, never retained by a runtime. */
  beforeSend?: (selection: unknown, prompt: string) => Promise<boolean>
  apiBase?: string
  wsBase?: string
  parentToken?: string
  getConnection?: () => Promise<{ wsBase: string; apiBase: string; parentToken: string }>
  /** Resolve current credentials on send; never put them in timeline entry data. */
  getModelConfig?: () => Promise<FastPlanModelConfig>
  uploadBase?: string
  adminBase?: string
  downloadBase?: string
  executionMode?: FastPlanExecutionMode
  confirmationPolicy?: FastPlanConfirmationPolicy
  maxReconnectAttempts?: number
  baseReconnectDelayMs?: number
  retainConnectionOnSessionSwitch?: boolean
  theme?: FastPlanThemeMode
  themeTokens?: FastPlanThemeTokens
  /** Built-in locale used by embedded FastPlan conversations. Defaults to zh-CN. */
  locale?: string
  /** Overrides chat-renderer's generic language pack. */
  labels?: Partial<ChatRendererMessages>
  /** Overrides FastPlan planning controls' language pack. */
  planningLabels?: Partial<FastPlanPlanningMessages>
  model?: string | FastPlanModelConfig
  avatar?: string
  composerMode?: 'internal' | 'host'
  composerTarget?: string | HTMLElement | null
  /** Optional owner guard when one host renders multiple live FastPlan entries. */
  composerEntryId?: string | null
  density?: FastPlanDensity
  compactBreakpoint?: number
  /** Optional application-level owner for live session controllers. */
  sessionManager?: {
    getOrCreate: (
      sessionId: string,
      options?: Omit<FastPlanExecutionControllerOptions, 'sessionId' | 'autoConnect'>,
    ) => FastPlanExecutionController
    retain?: (sessionId: string, ownerId: string) => void
    release?: (sessionId: string, ownerId: string) => void
    /** Dispatch built-in controls using the application's current bound identity. */
    executeEntryControl?: (sessionId: string, entryId: string, command: FastPlanInteractionCommand) => Promise<boolean>
  }
}

export interface FastPlanTimelinePlugin extends ChatWorkspacePlugins {
  timelineRenderers: Record<'fastplan', Component>
}

/**
 * Build an opt-in ChatWorkspace plugin. Configuration is captured by the
 * renderer factory, so no globals or host environment variables are read.
 */
export function createFastPlanTimelinePlugin(
  options: FastPlanTimelinePluginOptions = {},
): FastPlanTimelinePlugin {
  const Renderer = defineComponent({
    name: 'ConfiguredFastPlanTimelineRenderer',
    inheritAttrs: false,
    props: {
      entry: { type: Object, required: true },
      data: { type: Object, required: true },
      status: { type: String, default: undefined },
    },
    emits: ['action'],
    setup(props, { emit, slots }) {
      return () => h(FastPlanTimelineRenderer as Component, {
        entry: props.entry,
        data: props.data,
        status: props.status,
        // Read and snapshot every option during render. Hosts commonly mutate a
        // reactive composerTarget after their composer slot has mounted; passing
        // the stable options object here would otherwise hide that nested change
        // from the configured renderer boundary.
        config: { ...options },
        onAction: (value: unknown) => emit('action', value),
      }, slots)
    },
  })
  return { timelineRenderers: { fastplan: Renderer } }
}
