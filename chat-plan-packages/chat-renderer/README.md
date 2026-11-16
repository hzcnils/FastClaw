# chat-renderer

## 通用参数补全表单

`ChatParameterForm` 根据 `fields` 渲染输入项，不依赖 Element Plus、供应商数据库或调用模式。
普通对话和任务规划可共用此组件；宿主在 `submit` 中保存配置，成功后继续待发送请求。

```vue
<ChatParameterForm
  v-model="values"
  :fields="[
    { key: 'baseUrl', label: 'Base URL', type: 'url', required: true },
    { key: 'apiKey', label: 'API Key', type: 'password', required: true },
  ]"
  title="补全供应商参数"
  :submitting="saving"
  :error="saveError"
  @submit="saveAndContinue"
  @cancel="cancelPendingRequest"
/>
```

字段支持 `key`、`label`、`type`（`text` / `password` / `url`）、`placeholder` 和 `required`。
`submit` 返回去除首尾空白的字段值；必填项为空或正在保存时不能提交。
可传入 `description`、`submitLabel`、`cancelLabel` 自定义文案，样式使用包的主题变量。
表单采用浅黄色提示卡片、左侧标签与右侧输入框的布局；必填字段保留校验，不显示星号。
表单值应保留在宿主的临时状态中，通过自定义消息渲染器接入；不要把 API Key 放进持久化消息或规划条目。

## Host selector renderer

`ChatWorkspace` forwards the optional `composer-selectors` slot to
`ChatComposer`. Hosts can render framework-specific controls such as Element
Plus while the package keeps its dependency-free default selector:

```vue
<ChatWorkspace :composer-selectors="selectors">
  <template #composer-selectors="{ selectors }">
    <!-- Render host-native selectors here. -->
  </template>
</ChatWorkspace>
```

If the slot is omitted, the existing `ChatComposerSelector` behavior and
events are unchanged.

一个与业务后端解耦的 Vue 3 聊天消息渲染组件包，适用于浏览器 SPA、Tauri 和 Electron。组件包不依赖 Pinia、Vue Router、vue-i18n 或 Tailwind，所有业务操作通过事件交给宿主应用处理。

## 界面语言

组件默认显示中文。`ChatWorkspace`、`ChatConversation`、`ChatComposer`、`ChatMessagePanel` 和 `ChatTimeline` 均支持 `locale`：传入 `en-US` 使用内置英文，未传或传入其他值时使用中文。`labels` 是部分语言包，可覆盖任意 `ChatRendererMessages` 键，适用于第三方语言：

```vue
<ChatWorkspace
  locale="en-US"
  :labels="{ copy: 'Copy', cancel: 'Cancel' }"
/>
```

包同时导出 `defaultMessages`、`englishMessages`、`chatRendererMessages` 与 `resolveChatRendererMessages`，供宿主复用或构建自己的语言包。

## 功能概览

- 用户、助手、系统和工具消息
- Markdown、JSON、KaTeX、Mermaid 和 Highlight.js 代码高亮（基于 `md-editor-v3`）
- 思考过程、图片、附件、工具调用、通知和自定义内容块
- 流式状态、初始加载、卡死恢复、历史分页和滚动锚点
- 明暗主题、CSS Variables、移动端和键盘操作
- 操作栏总开关、逐项配置、动态 resolver 和自定义插槽
- 内置模型调用错误分类、友好提示和可扩展错误操作事件
- 通过 DOMPurify 清洗 Markdown 生成的最终 HTML

## 模型调用错误

将消息状态设为 `error` 后，组件会隐藏标准消息操作栏，并根据结构化错误或正文中的 HTTP 状态码展示内置提示：

```ts
const failedMessage: ChatMessage = {
  id: 'assistant-error',
  role: 'assistant',
  createdAt: Date.now(),
  status: 'error',
  error: { raw: 'HTTP状态码:403\ninsufficient_user_quota' },
  parts: [],
}
```

额度或余额不足只展示提示，不附带充值操作。宿主可以在 `ChatMessage.error` 中覆盖 `category`、`message` 和 `actions`。包同时导出 `resolveChatError`，可用于服务端适配层、日志或自定义 UI。

自动重试期间可传入临时进度，组件会隐藏普通流式正文和操作栏，仅显示简洁状态：

```ts
message.status = 'streaming'
message.retry = { attempt: 1, total: 5 } // 正在重试中…1/5
```

## 安装与构建

本仓库中的源码目录为 `frontend/packages/chat-renderer`。

```bash
cd frontend/packages/chat-renderer
npm run build
```

构建产物位于 `dist`。发布到 npm 或内部 registry 后：

```bash
npm install chat-renderer
```

```ts
import { ChatMessagePanel } from 'chat-renderer'
import 'chat-renderer/style.css'
```

Vue `^3.5.3` 是 peer dependency。`md-editor-v3`、Highlight.js、KaTeX、Mermaid 和 DOMPurify 均由包声明为运行时依赖；扩展通过模块实例加载，不需要 CDN 或 `/static/md-editor-v3` 静态资源目录。

## 快速开始

```vue
<script setup lang="ts">
import { ref } from 'vue'
import {
  ChatMessagePanel,
  type ChatMessage,
  type MessageActionEvent,
} from 'chat-renderer'
import 'chat-renderer/style.css'

const messages = ref<ChatMessage[]>([
  {
    protocolVersion: 1,
    id: 'user-1',
    role: 'user',
    createdAt: Date.now(),
    parts: [{ type: 'text', text: '请解释这段代码', format: 'plain' }],
  },
  {
    protocolVersion: 1,
    id: 'assistant-1',
    role: 'assistant',
    createdAt: Date.now(),
    model: { id: 'demo-model', name: 'Demo Model' },
    status: 'complete',
    parts: [{ type: 'text', text: '这是一个 **Markdown** 回答。', format: 'markdown' }],
  },
])

function handleAction(event: MessageActionEvent) {
  // 在这里调用宿主应用的编辑、重新生成、点赞或删除 API。
  console.log(event.action, event.messageId, event.message)
}
</script>

<template>
  <ChatMessagePanel
    :messages="messages"
    :actions="{
      copy: true,
      regenerate: true,
      delete: { danger: true, requireConfirm: true },
    }"
    theme="auto"
    @message-action="handleAction"
  />
</template>
```

容器必须具有可计算的高度，例如：

```css
.chat-page {
  height: 100vh;
  min-height: 0;
}
```

## 导出内容

```ts
export {
  ChatMessagePanel,
  ChatMessageItem,
  ChatMarkdownRenderer,
  ChatToolGroup,
  CHAT_PROTOCOL_VERSION,
  defaultMessages,
  groupMessagesIntoTurns,
  groupConsecutiveTools,
  resolveMessageActions,
  safeUrl,
  textFromMessage,
}
```

包同时导出 `types.ts` 中的全部 TypeScript 类型。

## 完整会话页面

除消息渲染组件外，包内还提供以下页面级组件：

```ts
import {
  ChatConversation,
  ChatContextBar,
  ChatComposer,
} from 'chat-renderer'
```

- `ChatConversation`：组合上下文信息栏、消息区域、规划区域和底部输入区。
- `ChatContextBar`：展示文档、知识库、助手、模型和配置等当前会话上下文。
- `ChatComposer`：通用输入框、附件预览、操作按钮、发送和停止交互。

这些组件同样不调用接口。选择模型、上传文件、语音识别、知识库检索、发送消息和停止生成均通过事件交给宿主。

### ChatConversation 快速开始

```vue
<script setup lang="ts">
import { ref } from 'vue'
import {
  ChatConversation,
  type ChatComposerSubmitEvent,
  type ChatContextItem,
} from 'chat-renderer'

const input = ref('')
const contextItems: ChatContextItem[] = [
  { id: 'docs', kind: 'document', label: '文档', badge: 3 },
  { id: 'kb', kind: 'knowledge', label: '知识库', value: '产品规范' },
  { id: 'assistant', kind: 'assistant', label: '助手', value: '编码助手' },
  { id: 'model', kind: 'model', label: '模型', value: 'Claude Sonnet' },
]

function submit(event: ChatComposerSubmitEvent) {
  // 调用宿主自己的 API / WebSocket
  console.log(event.text, event.attachments)
}
</script>

<template>
  <ChatConversation
    v-model="input"
    :messages="messages"
    :context-items="contextItems"
    context-placement="top"
    :composer-actions="[
      { id: 'attach-image', label: '图片', icon: '▧' },
      { id: 'attach-file', label: '文件', icon: '＋' },
      { id: 'voice', label: '语音', icon: '●', position: 'end' },
    ]"
    @submit="submit"
    @abort="abortGeneration"
    @context-action="handleContextAction"
    @composer-action="handleComposerAction"
  >
    <template #initialization-placeholder>
      <ProjectInitializationPlaceholder />
    </template>
    <template #assistant-loading>
      <ProjectAssistantLoading />
    </template>
    <template #planning-panel>
      <PlanningTree :nodes="planNodes" />
    </template>
  </ChatConversation>
</template>
```

### ChatConversation 参数

| 参数 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `messages` | `ChatMessage[]` | `[]` | 公共消息协议数组。 |
| `modelValue` | `string` | `''` | 输入内容，支持 `v-model`。 |
| `contextItems` | `ChatContextItem[]` | `[]` | 文档、知识库、助手、模型等上下文入口。 |
| `contextPlacement` | `'top' \| 'composer'` | `'top'` | 信息栏显示在页面顶部或输入框上方。第二种布局已经定义，可后续完善视觉。 |
| `showContext` | `boolean` | `true` | 是否显示上下文信息栏。 |
| `showComposer` | `boolean` | `true` | 是否显示底部输入区域。 |
| `initializing` | `boolean` | `false` | 显示宿主提供的 `initialization-placeholder` 插槽。包内没有默认项目占位内容。 |
| `assistantLoading` | `boolean` | `false` | 显示宿主提供的 `assistant-loading` 插槽。 |
| `composerLoading` | `boolean` | `false` | 输入区切换为停止状态。 |
| `composerDisabled` | `boolean` | `false` | 禁用输入和发送。 |
| `composerReadonly` | `boolean` | `false` | 输入框只读。 |
| `composerPlaceholder` | `string` | `'Type a message…'` | 输入提示文字。 |
| `composerActions` | `ChatComposerAction[]` | `[]` | 输入框底部按钮。 |
| `composerSelectors` | `ChatComposerSelectorConfig[]` | `[]` | 输入框顶部的受控选择器；空数组不渲染该区域。 |
| `composerAttachments` | `ChatComposerAttachment[]` | `[]` | 待发送附件、图片、任务或自定义对象。 |
| `composerMaxHeight` | `number` | `200` | 默认文本输入控件的最大高度（像素）。 |
| `composerAutoResize` | `boolean` | `true` | 是否随内容自动调整默认文本输入控件高度。 |
| `sendByCtrlEnter` | `boolean` | `false` | `false` 为 Enter 发送、Shift+Enter 换行；`true` 为 Ctrl/Cmd+Enter 发送。 |
| `loadingState` | `LoadingState` | `{}` | 消息面板加载状态。 |
| `paginationState` | `PaginationState` | `{}` | 历史消息分页状态。 |
| `showActions` | `boolean` | `true` | 消息操作栏总开关。 |
| `actions` | `MessageActionsConfig` | `{}` | 消息动作默认配置。 |
| `resolveActions` | `ActionResolver` | - | 消息动作动态解析器。 |
| `adapters` | `ChatRendererAdapters` | - | 浏览器能力适配器。 |
| `toolRenderers` | `ToolRendererRegistry` | `{}` | 工具专用渲染器。 |
| `labels` | `Partial<ChatRendererMessages>` | `{}` | 消息区域文案。 |
| `locale` | `string` | - | 内容语言。 |
| `theme` | `'light' \| 'dark' \| 'auto'` | `'auto'` | 会话页面主题。 |
| `allowHtml` | `boolean` | `false` | 是否允许清洗后的 Markdown HTML。 |
| `maxRenderChars` | `number` | `15000` | Markdown 完整渲染阈值。 |
| `markdownAutoFoldThreshold` | `number` | `10000` | `md-editor-v3` 代码块自动折叠的行数阈值。 |
| `markdownShowCodeRowNumber` | `boolean` | `true` | 是否显示 Markdown 代码块行号。 |
| `autoScroll` | `boolean` | `true` | 是否启用消息面板自动滚动。 |

### ChatConversation 事件

| 事件 | 参数 | 说明 |
| --- | --- | --- |
| `update:modelValue` | `string` | 输入内容变化。 |
| `submit` | `ChatComposerSubmitEvent` | 请求发送；宿主负责校验、持久化和调用模型。 |
| `abort` | `MouseEvent` | 请求停止生成。 |
| `composer-action` | `ChatComposerActionEvent` | 输入栏按钮触发。 |
| `composer-files` | `ChatComposerFileSelectionEvent` | 带文件选择能力的输入栏按钮选中文件。 |
| `composer-selector-change` | `ChatComposerSelectorChangeEvent` | 受控选择器请求变更；宿主负责回写新值。 |
| `context-action` | `ChatContextActionEvent` | 文档、知识库、助手、模型等入口触发。 |
| `attachment-remove` | `attachment, index` | 请求移除待发送附件。 |
| `attachment-action` | `ChatComposerAttachmentActionEvent` | 附件重试、预览等动作触发。 |
| `message-action` | `MessageActionEvent` | 消息操作栏触发。 |
| `load-older` | - | 请求加载更早消息。 |
| `load-all` | - | 请求加载全部历史。 |
| `recover` | - | 请求恢复卡住的会话。 |

### ChatConversation 插槽

| 插槽 | 用途 |
| --- | --- |
| `context-start` / `context-end` | 在上下文栏两端注入宿主控件。 |
| `context-item` | 完全替换单个上下文入口。 |
| `before-messages` / `after-messages` | 消息区域前后扩展。 |
| `messages` | 用宿主消息列表完全替换默认 `ChatMessagePanel`。迁移旧项目时推荐使用。 |
| `initialization-placeholder` | 宿主初始化占位。公共包不内置具体实现。 |
| `assistant-loading` | 宿主模型加载/思考占位。公共包不内置具体实现。 |
| `message-before-content` | 模型头之后、消息正文之前的消息级扩展；作用域包含 `{ message, turn, messageIndex, isFirstAssistantInTurn }`。 |
| `message-content` | 用宿主 renderer 替换整段消息正文；作用域包含 `{ message, parts, turn, messageIndex }`。 |
| `message-text` | 只替换 text part；作用域包含 `{ part, message, partIndex, turn, messageIndex }`。 |
| `message-reasoning` | 只替换 reasoning part；作用域同上。 |
| `message-after-content` | 消息正文之后、操作栏之前的消息级扩展。 |
| `message-after` | 整条消息组件之后的业务扩展；保持现有外层语义。 |
| `planning-panel` | 任务规划树、Todo、阶段进度等输入区上方内容。 |
| `composer-before` / `composer-after` | 输入区前后扩展，例如权限确认、任务选择器。 |
| `composer` | 完全替换默认通用输入区，适合保留旧项目复杂 Composer。 |
| `composer-inside-top` | 输入框四区域之前的兼容扩展位。 |
| `composer-selectors` | 替换顶部选择器区域。 |
| `composer-attachment` | 替换单个附件卡片。 |
| `composer-input` | 替换输入控件区域。 |
| `composer-action` / `composer-action-icon` | 替换单个操作或其图标。 |
| `composer-toolbar-start` / `composer-toolbar-end` | 向底部操作栏左右区域注入内容。 |
| `composer-submit-icon` / `composer-abort-icon` | 替换发送或停止图标。 |
| `overlays` | 对话级弹窗、菜单或其他浮层。 |

默认输入框稳定包含 `selectors`、`attachments`、`input`、`toolbar` 四个语义区域，并分别标记 `data-composer-region`。所有区域级插槽都会收到统一的 `ChatComposerRegionScope`：输入值、附件、选择器、操作项、禁用/只读/loading 状态，以及 `update`、`submit`、`abort`、`triggerAction` 方法。原有插槽额外参数（例如单个 `attachment` 或 `action`）继续保留。

### ChatContextItem

```ts
interface ChatContextItem {
  id: string
  kind: 'document' | 'knowledge' | 'assistant' | 'model' | 'settings' | 'custom'
  label: string
  value?: string
  icon?: string
  badge?: string | number
  visible?: boolean
  disabled?: boolean
  active?: boolean
  metadata?: Record<string, unknown>
}
```

`visible` 只控制展示，`disabled` 只控制前端交互，均不能替代宿主权限检查。

### ChatComposerAction

```ts
interface ChatComposerAction {
  id: string
  label: string
  tooltip?: string
  icon?: string
  position?: 'start' | 'end'
  visible?: boolean
  disabled?: boolean
  active?: boolean
  order?: number
  accept?: string
  multiple?: boolean
  payload?: unknown
}
```

内置输入区不会因为 `id` 为 `attach-file`、`voice` 或 `knowledge` 就访问浏览器或接口；所有按钮统一发出 `composer-action`。

### ChatComposerAttachment

附件支持 `image`、`file`、`task` 和 `custom` 四种类型，并具有 `uploading`、`processing`、`complete`、`error` 等展示状态。组件不创建或释放 Object URL；若宿主传入 `previewUrl`，仍由宿主负责生命周期。

### 对大模型对话与任务规划的复用建议

| 能力 | 推荐程度 | 建议边界 |
| --- | --- | --- |
| 会话布局、消息滚动、输入区、操作事件 | 高 | 放入公共包。 |
| 文档/知识库/助手/模型入口的展示协议 | 高 | 使用 `ChatContextItem`，数据和选择逻辑由宿主提供。 |
| 规划树、Todo、步骤状态 | 高 | 使用 `planning-panel` 插槽；树节点协议建议独立成可选子包，避免污染核心聊天协议。 |
| 文件/图片附件预览 | 高 | 公共包负责展示和移除事件，读取、转换、上传归宿主。 |
| Slash Command、@ 文件提及、模型搜索面板 | 中 | 可以通过输入插槽复用；数据源和执行逻辑不进入核心包。 |
| 语音输入、OCR、文件转 Markdown | 中低 | 依赖平台服务，建议宿主实现并通过 `composer-action` 回填文本或附件。 |
| 权限申请、工具授权 | 中低 | 展示可插槽化，但授权必须由宿主和服务端完成。 |
| Pinia 会话状态、WebSocket、API、Toast、路由 | 不推荐 | 始终留在宿主。 |
| 初始化占位、助手加载占位 | 不推荐内置 | 使用明确插槽注入，避免项目文案和动画进入公共包。 |

## ChatMessagePanel 参数

| 参数 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `messages` | `ChatMessage[]` | 必填 | 按展示顺序传入的消息列表。消息应具有稳定且唯一的 `id`。 |
| `loadingState` | `LoadingState` | `{}` | 初始加载、生成中、卡死状态及状态文案。 |
| `paginationState` | `PaginationState` | `{}` | 是否还有历史消息、分页加载状态和消息总数。 |
| `showActions` | `boolean` | `true` | 操作栏总开关。为 `false` 时不展示任何内置动作。 |
| `assistantActionsReady` | `boolean` | `true` | 为 `false` 时暂时隐藏 assistant 操作栏，适合流式生成完成后再开放操作。 |
| `actions` | `MessageActionsConfig` | `{}` | 各动作的启用、显示、禁用、排序和确认配置。 |
| `resolveActions` | `ActionResolver` | `undefined` | 根据消息和上下文执行最后一层动作筛选或覆盖。 |
| `adapters` | `ChatRendererAdapters` | `undefined` | 剪贴板、图片预览、下载、链接和日期格式化能力。 |
| `toolRenderers` | `Record<string, Component>` | `{}` | 以工具名称为 key 的自定义 Vue 渲染器。 |
| `labels` | `Partial<ChatRendererMessages>` | `{}` | 覆盖默认英文文案。 |
| `locale` | `string` | `undefined` | 设置面板根节点的 `lang` 属性。 |
| `theme` | `'light' \| 'dark' \| 'auto'` | `'auto'` | 主题；`auto` 在挂载时读取系统配色。 |
| `allowHtml` | `boolean` | `false` | 是否允许 Markdown 中的原始 HTML；开启后仍会执行 DOMPurify 清洗。 |
| `maxRenderChars` | `number` | `15000` | 单个 Markdown 内容的最大渲染字符数，超过后截断并展示“查看完整内容”。 |
| `markdownAutoFoldThreshold` | `number` | `10000` | 代码块达到该行数后自动折叠；宿主可覆盖。 |
| `markdownShowCodeRowNumber` | `boolean` | `true` | 是否显示 Markdown 代码块行号；宿主可覆盖。 |
| `autoScroll` | `boolean` | `true` | 用户位于底部时，新消息或流式增量是否自动滚到底部。 |

### LoadingState

```ts
interface LoadingState {
  initial?: boolean
  generating?: boolean
  stalled?: boolean
  label?: string
}
```

- `initial` 仅在消息为空时显示整页加载态。
- `generating` 在消息列表底部显示生成状态，并限制部分危险操作。
- `stalled` 显示卡死提示；点击恢复按钮触发 `recover`，组件不会自行终止请求。

### PaginationState

```ts
interface PaginationState {
  hasMore?: boolean
  loading?: boolean
  loadingAll?: boolean
  total?: number
}
```

当滚动位置距离顶部小于 80px，且 `hasMore=true`、`loading=false` 时，组件会触发 `load-older`。宿主加载早期消息并插入数组头部后，组件会维持当前视觉滚动位置。

## 消息协议

当前协议版本：

```ts
CHAT_PROTOCOL_VERSION === 1
```

### ChatMessage

```ts
interface ChatMessage {
  protocolVersion?: 1
  id: string
  role: 'user' | 'assistant' | 'system' | 'tool'
  createdAt: string | number | Date
  model?: { id?: string; name?: string; avatar?: string }
  status?: 'pending' | 'streaming' | 'complete' | 'error'
  parts: ChatPart[]
  feedback?: -1 | 0 | 1
  turnId?: string
  metadata?: Record<string, unknown>
}
```

注意事项：

- `id` 必须稳定。以 `temporary-` 开头的 ID 会被视为临时 ID，依赖持久化标识的动作会隐藏。
- 每条用户消息默认开始一个新轮次；也可通过 `turnId` 明确关联消息。
- 连续两条及以上已经完成的 `tool` 消息会折叠成工具组；运行中的工具始终直接展示。
- 一轮有多条助手消息时，默认只在该轮最后一条消息下展示操作栏。

### ChatPart

#### 文本

```ts
{ type: 'text', text: string, format?: 'plain' | 'markdown' | 'json' }
```

- 用户消息始终按纯文本显示，避免将用户输入解释成 HTML。
- `markdown` 支持 Markdown、公式和代码高亮。
- `json` 使用预格式化代码块展示；调用方应传入 JSON 字符串。

#### 思考过程

```ts
{
  type: 'reasoning'
  text: string
  label?: string
  status?: 'streaming' | 'complete'
}
```

默认以可折叠区域展示，`streaming` 状态带有活动指示。

#### 图片

```ts
{ type: 'image', src: string, alt?: string, name?: string }
```

支持 `http`、`https`、`blob`、站内相对地址，以及 PNG/JPEG/GIF/WebP Base64。Base64 URL 最长约 14,000,000 个字符；SVG Data URL 和危险协议会被拒绝。

#### 附件

```ts
{
  type: 'attachment'
  name: string
  url?: string
  size?: number
  mimeType?: string
}
```

`size` 使用字节。若提供安全 URL，默认渲染为链接；可通过 `attachment` 插槽完全替换。

#### 工具调用

```ts
{
  type: 'tool-call'
  id: string
  name: string
  input?: unknown
  output?: unknown
  status?: 'pending' | 'running' | 'complete' | 'error'
  error?: string
}
```

没有注册专用渲染器时，组件使用通用折叠 JSON 卡片。可以通过 `toolRenderers` 或 `tool` 插槽提供专用展示。

#### 通知

```ts
{
  type: 'notice'
  text: string
  tone?: 'neutral' | 'info' | 'success' | 'warning' | 'error'
}
```

#### 自定义内容

```ts
{ type: 'custom', kind: string, data: unknown }
```

通过 `custom` 插槽按照 `kind` 渲染。未提供插槽时显示安全的降级提示。自定义工具和自定义内容被错误边界隔离，单个渲染器异常不会主动替换整张消息列表。

## 操作栏

### 默认动作

- 用户消息：`copy`、`edit`、`resend`
- 助手消息：`copy`、`regenerate`、`like`、`dislike`
- 系统和工具消息：无默认动作

用户消息的操作始终全部直接展示，不收入省略号菜单。助手操作栏挂载前以前五个动作为稳定回退；挂载后根据容器宽度动态计算直接展示数量，其余动作自动收入省略号菜单。

### 动作名称

```ts
type MessageActionName =
  | 'copy' | 'edit' | 'resend' | 'regenerate' | 'quote' | 'tts'
  | 'translate' | 'favorite' | 'edit-as-doc' | 'switch-model' | 'trace-source'
  | 'export' | 'like' | 'dislike'
  | 'branch' | 'delete' | 'custom'
```

### MessageActionConfig

```ts
interface MessageActionConfig {
  enabled?: boolean
  visible?: boolean
  disabled?: boolean
  label?: string
  icon?: string
  tooltip?: string
  order?: number
  danger?: boolean
  requireConfirm?: boolean
  actionId?: string
  payload?: unknown
  menuItems?: MessageActionMenuItem[]
  customMenu?: boolean
  dividerBefore?: boolean
}

interface MessageActionMenuItem {
  id: string
  label: string
  disabled?: boolean
  danger?: boolean
  payload?: unknown
}
```

| 字段 | 说明 |
| --- | --- |
| `enabled` | 是否启用动作；`false` 等同于移除。 |
| `visible` | 是否显示动作。 |
| `disabled` | 保留按钮但禁止点击。适合表达 pending 或无权限状态。 |
| `label` | 动作显示文案；优先级低于 `labels` 中的同名文案。 |
| `icon` | 自定义文字图标；省略时使用公共包内置 SVG。 |
| `tooltip` | 按钮提示和无障碍名称。 |
| `order` | 排序值，数值越小越靠前。 |
| `danger` | 使用危险操作配色。 |
| `requireConfirm` | 第一次点击进入确认状态，确认后才派发事件。 |
| `actionId` | 事件中的动作名称，可用于定义多个 `custom` 动作。 |
| `payload` | 原样放入 `MessageActionEvent.payload`。 |
| `menuItems` | 可选二级菜单；选择后事件携带菜单项标识和菜单项 payload。 |
| `customMenu` | 为 `true` 时使用 `action-menu` 作用域插槽渲染该动作的菜单内容；未提供插槽时仍可回退到 `menuItems`。 |
| `dividerBefore` | 在该动作前显示分隔线。也可以在 `resolveActions` 名称数组中使用 `MESSAGE_ACTION_DIVIDER` 标记下一项。 |

配置示例：

```ts
const actions = {
  copy: true,
  regenerate: { order: 10, tooltip: '重新生成此回答' },
  export: { order: 20 },
  delete: { order: 100, danger: true, requireConfirm: true },
  custom: {
    actionId: 'send-to-document',
    label: '发送到文档',
    payload: { target: 'editor' },
  },
}
```

### 动作解析顺序

1. `showActions=false` 时全部隐藏。
2. 合并角色默认动作与 `actions` 配置。
3. 应用内置状态规则。
4. 最后调用 `resolveActions`。

内置规则包括：

- 生成中隐藏 `edit`、`delete`、`regenerate` 和 `export`。
- `regenerate` 仅允许最后一条已完成的助手回答。
- 临时或空 ID 隐藏依赖持久化标识的动作。

### resolveActions

```ts
import { MESSAGE_ACTION_DIVIDER } from 'chat-renderer'

const resolveActions: ActionResolver = (message, context) => {
  if (message.role === 'user') return ['copy', 'edit']
  if (!context.hasStableId) return ['copy']
  return ['copy', 'regenerate', MESSAGE_ACTION_DIVIDER, 'like', 'dislike']
}
```

```ts
interface MessageActionContext {
  index: number
  turnId?: string
  isLastMessage: boolean
  isLastCompletedAssistantTurn: boolean
  isGenerating: boolean
  hasStableId: boolean
}
```

resolver 返回动作名称数组时会筛选已经解析的动作；返回完整的 `ResolvedMessageAction[]` 时可以替换最终结果。

## 事件

| 事件 | 参数 | 触发时机 |
| --- | --- | --- |
| `message-action` | `MessageActionEvent` | 用户确认并执行一个消息动作。复制动作完成内置复制后也会触发。 |
| `load-older` | 无 | 点击加载更早消息，或滚动到顶部附近。 |
| `load-all` | 无 | 点击加载全部历史消息。 |
| `recover` | 无 | 点击卡死状态中的恢复按钮。 |
| `link-click` | `(href, MouseEvent)` | 点击通过安全校验的 Markdown 或附件链接。 |
| `image-preview` | `(images, activeIndex)` | 点击消息图片。 |
| `show-full-content` | `ChatMessage` | 点击超长内容的“查看完整内容”。 |
| `container-ready` | `HTMLElement` | 面板滚动容器完成挂载。 |

```ts
interface MessageActionEvent {
  action: string
  messageId: string
  turnId?: string
  message: ChatMessage
  selectedText?: string
  payload?: unknown
  menuItemId?: string
  menuItem?: MessageActionMenuItem
}
```

组件只发出事件，不会自行调用重新生成、删除、点赞或导出接口。按钮的显示与服务端权限是两回事，宿主必须再次校验权限。

## 插槽

| 插槽 | 作用域参数 | 说明 |
| --- | --- | --- |
| `empty` | 无 | 空消息状态。 |
| `loading` | `{ state }` | 初始加载状态。 |
| `generating` | `{ state }` | 底部生成状态。 |
| `stalled` | `{ state }` | 卡死状态。 |
| `avatar` | `{ message }` | 用户或模型头像。 |
| `header` | `{ message }` | 消息作者和时间区域。 |
| `content` | `{ message, parts, turn, messageIndex }` | 替换整段正文，优先级高于全部分片插槽。 |
| `text` | `{ part, message, partIndex, turn, messageIndex }` | 替换单个 text part。 |
| `reasoning` | `{ part, message, partIndex, turn, messageIndex }` | 替换单个 reasoning part。 |
| `attachment` | `{ part, message }` | 附件卡片。 |
| `tool` | `{ part, message }` | 工具调用内容。 |
| `custom` | `{ part, message }` | 自定义内容块。 |
| `before-content` | `{ message, turn, messageIndex, isFirstAssistantInTurn }` | 模型头之后、消息正文之前的扩展内容。`ChatConversation` 中对应 `message-before-content`。 |
| `after-content` | `{ message }` | 消息正文之后、操作栏之前的扩展内容。 |
| `message-after` | `{ message, turn, messageIndex }` | 整条消息之后的业务扩展内容。 |
| `action-bar` | `{ message, actions, trigger }` | 替换整条操作栏。 |
| `action` | `{ action, message, trigger }` | 替换单个动作按钮。 |
| `action-menu` | `{ action, message, placement, triggerItem, close }` | 为标记了 `customMenu` 的动作渲染自定义菜单内容；`placement` 为 `direct` 或 `overflow`。 |

同一个 `action-menu` 插槽可以通过 `action.name` 或 `action.actionId` 渲染多个不同菜单。简单菜单继续使用 `menuItems`，只有复杂菜单需要设置 `customMenu: true`。调用 `triggerItem(item)` 会沿用公共 `MessageActionEvent` 协议并自动关闭菜单。

```vue
<ChatMessagePanel :messages="messages">
  <template #tool="{ part }">
    <MySearchResult v-if="part.name === 'Search'" :result="part.output" />
    <pre v-else>{{ part.output }}</pre>
  </template>
</ChatMessagePanel>
```

也可以注册组件表：

```ts
const toolRenderers = {
  Search: MySearchResult,
  ReadFile: MyFileResult,
}
```

注册组件会收到 `{ part, message }` props。

## Adapters

```ts
interface ChatRendererAdapters {
  clipboard?: (text: string) => boolean | Promise<boolean>
  imagePreview?: (images: ImagePart[], activeIndex: number) => void
  download?: (data: Blob | string, filename: string) => void | Promise<void>
  link?: (href: string, event: MouseEvent) => void
  formatDate?: (value: ChatMessage['createdAt'], message: ChatMessage) => string
}
```

- 未提供 `clipboard` 时优先使用浏览器 `navigator.clipboard`。
- `imagePreview` 用于连接宿主的灯箱组件；无适配器时仍会触发 `image-preview`。
- `link` 可阻止默认跳转并交由 Tauri、Electron 或宿主路由处理。
- `formatDate` 用于注入语言、时区和日期格式。
- `download` 预留给自定义导出动作；当前核心组件不会主动下载文件。

## 暴露的方法

通过组件 ref 可以访问：

```ts
interface ChatMessagePanelExpose {
  scrollToBottom: (behavior?: ScrollBehavior) => void
  element: HTMLElement | null
}
```

```vue
<script setup lang="ts">
import { ref } from 'vue'
import { ChatMessagePanel } from 'chat-renderer'

const panel = ref<InstanceType<typeof ChatMessagePanel> | null>(null)
const goBottom = () => panel.value?.scrollToBottom('smooth')
</script>

<template><ChatMessagePanel ref="panel" :messages="messages" /></template>
```

## 文案配置

`labels` 支持覆盖以下键：

```text
empty, loading, generating, stalled, recover, loadOlder, loadAll,
copy, copied, edit, resend, regenerate, quote, tts, translate,
favorite, export, like, dislike, branch, delete, confirm, cancel,
edit-as-doc, switch-model, trace-source,
reasoning, toolInput, toolOutput, unknownContent, showFullContent,
newMessages, scrollToBottom, more
```

```vue
<ChatMessagePanel
  :messages="messages"
  locale="zh-CN"
  :labels="{
    empty: '暂无消息',
    loading: '正在加载…',
    copy: '复制',
    copied: '已复制',
    more: '更多',
  }"
/>
```

## 主题和 CSS Variables

样式仅作用于 `.jcr-*` 类。可在面板自身或外层主题类覆盖：

```css
.brand-chat .jcr-panel {
  --jcr-bg: #ffffff;
  --jcr-fg: #1f2937;
  --jcr-muted: #6b7280;
  --jcr-border: #e5e7eb;
  --jcr-primary: #7c3aed;
  --jcr-user-bg: #f3e8ff;
  --jcr-assistant-bg: transparent;
  --jcr-danger: #dc2626;
  --jcr-code-bg: #f6f8fa;
  --jcr-radius: 14px;
  --jcr-font-size: 14px;
  --jcr-gap: 16px;
  --jcr-bubble-max-width: 78%;
}
```

`theme="dark"` 会在根节点设置 `data-theme="dark"` 并应用默认深色变量。宿主仍可覆盖任何变量。

## 安全策略

- `allowHtml` 默认关闭。
- 无论 `allowHtml` 是否开启，最终 HTML 都经过 DOMPurify。
- 禁止 `script`、`style`、`iframe`、`object`、`embed`、`form`、`srcdoc` 和内联 `style`。
- 链接只允许 `http`、`https`、`mailto`、`tel`、站内相对地址和锚点。
- 图片不允许 SVG Data URL 和危险协议。
- 外链默认带有 `noopener noreferrer nofollow`。
- 超长 Markdown 按 `maxRenderChars` 截断，避免长内容同步解析造成明显卡顿。

这些前端规则不代替服务端输入校验、权限验证或内容安全策略。

## 直接使用子组件

### ChatMessageItem

```ts
{
  message: ChatMessage
  actions?: ResolvedMessageAction[]
  messages: ChatRendererMessages
  adapters?: ChatRendererAdapters
  toolRenderers?: ToolRendererRegistry
  allowHtml?: boolean
  maxRenderChars?: number
  markdownAutoFoldThreshold?: number // 默认 10000
  markdownShowCodeRowNumber?: boolean // 默认 true
  theme?: 'light' | 'dark'
  showModelHeader?: boolean
  showAvatar?: boolean
}
```

事件：`message-action`、`link-click`、`image-preview`、`show-full-content`。

正文渲染优先级为 `content` 整段覆盖 → `text/reasoning/tool/custom` 分片覆盖 → 公共默认 renderer。即使 `message.parts` 为空，消息外壳、模型头以及 `before-content` / `after-content` 仍会渲染。当一个 turn 的首个模型输出是工具调用时，模型头会前置到首个工具或工具组之前，并且仍只渲染一次。

### ChatMarkdownRenderer

```ts
{
  content?: string
  allowHtml?: boolean       // 默认 false
  maxChars?: number         // 默认 15000
  autoFoldThreshold?: number // 默认 10000
  showCodeRowNumber?: boolean // 默认 true
  theme?: 'light' | 'dark'  // 默认 light
  streaming?: boolean       // 流式阶段暂停 KaTeX/Mermaid，默认 false
}
```

事件：`link-click`、`show-full-content`。插槽：`show-full-label`。默认 renderer 内置 `MdPreview`，代码高亮、KaTeX 和 Mermaid 均使用包依赖的模块实例，不发起 CDN 请求。

## 当前项目的旧协议适配

宿主通过以下适配器将原 `ChatMessage` 转换为公共协议：

```ts
import {
  adaptLegacyMessage,
  adaptLegacyMessages,
  legacyMessageFromAction,
} from '@/components/chat/adapters/chatRendererAdapter'
```

适配器会将原消息、图片、附件、思考、工具调用、MoA 引用和变更汇总转换为 `parts`，并将原始消息保存在：

```ts
message.metadata.legacyMessage
```

该字段仅用于当前项目迁移，不应成为其他项目依赖的公共协议。

## ChatWorkspace 协议（阶段 0）

`chat-renderer` 额外导出 `CHAT_WORKSPACE_PROTOCOL_VERSION`、`ChatTimelineEntry`、
`ChatWorkspaceAdapter`、`ChatStreamEvent`、`ChatWorkspaceActionEvent` 和
`ChatWorkspacePlugins`，用于后续构建普通问答与嵌入内容共存的工作台。

- `ChatTimelineEntry` 使用 `message`、`turn`、`embedded` 三种判别类型；多模型回答进入
  `turn.alternatives`，FastPlan 使用 `renderer: 'fastplan'` 的 `embedded` entry。
- `ChatWorkspaceAdapter` 仅定义普通宿主问答和宿主业务动作，不负责 FastPlan 请求。
- FastPlan 的会话、流式通信、上传、下载和执行控制继续由 `planflow-runtime` 内部完成。
- 插件注册表只接收 Vue 组件，不要求宿主字段、接口客户端或全局样式。

该阶段仅发布协议类型，尚未改变 `ChatConversation` 的 `messages` 输入和默认行为。

## ChatWorkspace 基础工作台（阶段 1）

阶段 1 新增了 opt-in 的 `ChatWorkspace`，旧的 `ChatMessagePanel`、`ChatConversation(messages)` 和 `CHAT_PROTOCOL_VERSION` 保持兼容。

```vue
<script setup lang="ts">
import { ref } from 'vue'
import { ChatWorkspace, type ChatTimelineEntry, type ChatWorkspaceAdapter } from 'chat-renderer'

const input = ref('')
const timeline = ref<ChatTimelineEntry[]>([])
const adapter: ChatWorkspaceAdapter = {
  async *send(input) {
    yield { type: 'text-delta', delta: `echo: ${input.text}` }
    yield { type: 'message-complete' }
  },
}
</script>

<template>
  <ChatWorkspace v-model="input" v-model:timeline="timeline" :adapter="adapter" />
</template>
```

- `ChatTimeline` 按顺序渲染 `message`、`turn` 和 `embedded` entry。
- `ChatModelVariants` 支持 tabs 与 split 两种多答案布局。
- `ChatQuestionNavigator` 从用户消息或 `metadata.navigationLabel` 生成导航。
- `showScrollToBottom` 可启用公共时间线的圆形回到底部按钮；`scroll-to-bottom-icon` 插槽允许宿主复用自己的图标，按钮文案通过 `labels.scrollToBottom` 覆盖。
- `ChatAuxiliaryPanel` 使用 `plugins.auxiliaryPanels` 注册业务辅助面板。
- `plugins.timelineRenderers`、`plugins.partRenderers` 和 `plugins.auxiliaryPanels` 都只接收 Vue 组件，不绑定宿主接口。
- 提供 `adapter` 时，Workspace 消费规范化流事件并维护草稿时间线；不提供时只发送 `workspace-action`，不会请求任何后端。
- FastPlan 不通过 `ChatWorkspaceAdapter` 发送；后续由 `planflow-kit` 的 embedded timeline plugin 挂载，网络、上传和下载仍归 FastPlan 公共 runtime。

本地无后端演示：

```bash
npm run demo
npm run demo:build
```

## 开发验证

```bash
# 组件包
cd frontend/packages/chat-renderer
npm test
npm run build

# 当前前端项目
cd frontend
npm run typecheck
npm test
npm run build
```
