# planflow-kit

## ChatWorkspace timeline plugin

`createFastPlanTimelinePlugin` is an opt-in integration for `chat-renderer`'s
`ChatWorkspace`. Existing consumers are unchanged unless they register the
plugin and create `embedded` entries whose renderer is `fastplan`.

```ts
import { createFastPlanTimelinePlugin } from 'planflow-kit'
import 'planflow-kit/style.css'

const plugins = createFastPlanTimelinePlugin({
  apiBase: '/fastPlan',
  wsBase: 'ws://localhost:2800/fastPlan',
  executionMode: 'manual',
  confirmationPolicy: 'strict',
})
```

```vue
<ChatWorkspace :plugins="plugins" :adapter="adapter" />
```

The timeline entry owns the live FastPlan command and session state. The
plugin internally performs FastPlan session creation, streaming/WebSocket
communication, uploads, downloads, confirmations, and execution controls.
The host should only supply proxy/base URL configuration and optional mapping
or persistence metadata for its own conversation system. Do not duplicate
FastPlan transport calls in the host adapter.

The minimum entry shape is:

```ts
{
  kind: 'embedded',
  id: 'fastplan:local-id',
  renderer: 'fastplan',
  status: 'active',
  data: {
    connectionMode: 'live',
    executionMode: 'manual',
    confirmationPolicy: 'strict',
    command: { id: 'command-id', type: 'send', prompt: 'Plan this task' },
  },
}
```

将 FastPlan 的单会话、规划、执行和聊天 UI 嵌入任意 Vue 3 项目。宿主只管理自己的路由、导航和会话列表。

```vue
<script setup lang="ts">
import { FastPlanConversation } from 'planflow-kit'
import 'planflow-kit/style.css'
</script>

<template>
  <FastPlanConversation
    :session-id="sessionId"
    api-base="https://fastplan.example.com"
    ws-base="wss://fastplan.example.com"
    :model="{ model: 'qwen-plus', baseUrl: 'https://model.example.com/v1' }"
    execution-mode="auto"
    theme="light"
    :theme-tokens="{ primary: '#5b6af0', radius: '16px' }"
    @human-request="showNotification"
    @execution-completed="handleResult"
  >
    <template #top-bar="{ state }">{{ state.sessionId }}</template>
    <template #result-card="{ result, files }">...</template>
  </FastPlanConversation>
</template>
```

低层接入可使用 `createFastPlanExecutionController`。它统一提供 `sendChat`、`confirmPlan`、`cancelPlan`、`cancelExecution`、`pauseExecution`、`resumeExecution`、`retryNode`、`skipNode`、`respondHumanCheckpoint`、`updateNodeOutput`、`updateNextNodeConfig`、`executeTemplate` 和 `saveAsTemplate`。

所有服务地址均由构造参数或组件属性传入；包内不读取宿主的 `import.meta.env`。

## 界面语言

所有内置通用文案默认显示中文。传入 `locale="en-US"` 可切换为内置英文；不支持的语言会回退到中文。`labels` 用于覆盖聊天区域的通用文案，`planningLabels` 用于覆盖规划、确认和任务列表文案，因此宿主也可以传入任意其他语言的部分语言包。

```vue
<FastPlanConversation
  :session-id="sessionId"
  locale="en-US"
  :labels="{ copy: 'Copy', cancel: 'Cancel' }"
  :planning-labels="{ taskList: 'Task list', confirmationRequired: 'Your confirmation is required' }"
/>
```

时间线接入时，在 `createFastPlanTimelinePlugin` 的选项中传入同名的 `locale`、`labels` 和 `planningLabels` 即可。语言包只需提供需要替换的键，未提供的键仍使用选定语言的内置文本。

## 公共契约

- 必填属性：`sessionId`。
- 服务配置：`apiBase`、`wsBase`、`uploadBase`、`adminBase`、`downloadBase`、`model`。
- 执行配置：`executionMode`、`confirmationPolicy`、`retainConnectionOnSessionSwitch`、`autoConnect`。`confirmationPolicy` 默认 `legacy` 保持兼容；设为 `strict` 时，`manual` 只确认整份规划，`step` 会在规划开始前及每个节点完成后等待确认。
- 输入区配置：`composerDisabled`、`composerReadonly`、`composerActions`、`composerSelectors`、`composerSelectorPlacement`、`composerAttachments`、`composerMaxHeight`、`composerAutoResize`、`sendByCtrlEnter`。选择器位置支持 `composer`（默认）、`header` 和 `hidden`。
- 消息操作配置：`showActions`、`actions`、`resolveActions`。
- 上下文配置：`showContext`、`contextItems`、`contextPlacement`；带 `options` 的条目由 FastPlan 自有选择器渲染。
- 嵌入布局：`layout`、`appearance`、`composerMode`、`composerTarget`、`showTaskList`、`interactionPlacement`、`errorPlacement`、`scrollMode`。`composerMode="internal"` 为默认值，继续使用组件内部的 `chat-renderer` Composer；统一会话宿主可设置 `composerMode="host"` 并通过 `composerTarget` 提供自身 `composer-before` 容器，FastPlan 将任务列表和确认交互贡献到该容器而不创建第二个输入框。目标暂不可用时会安全回退为 inline 展示。`showTaskList` 默认开启，多规划时间线宿主可只对当前条目开启以隐藏被替换的旧任务列表。`errorPlacement` 默认 `top`，宿主可显式设为 `inline` 将错误放到当前任务区底部。
- 响应式密度：`density` 支持 `auto`、`comfortable` 和 `compact`，默认 `auto`。自动模式使用 `ResizeObserver` 按实际规划内容宽度切换；`compactBreakpoint` 默认 `700`，恢复桌面布局时保留 20px 缓冲。宿主 Composer 使用独立宽度判断，因此 Teleport 后的任务列表不会误用时间线宽度。
- 事件：`state-change`、`loading-change`、`message-sent`、`plan-completed`、`execution-completed`、`human-request`、`composer-action`、`composer-files`、`composer-selector-change`、`attachment-remove`、`attachment-action`、`message-action`、`context-action`、`context-option-select`、`navigation-change`、`error`。
- 插槽：`top-bar`、`message-card`、`result-card`、`historical-planning-card`、`attachment-input`、`composer`、`composer-inside-top`、`composer-selectors`、`composer-attachment`、`composer-input`、`composer-action`、`composer-action-icon`、`composer-toolbar-start`、`composer-toolbar-end`、`composer-submit-icon`、`composer-abort-icon`、`context-item`、`action-bar`、`action`、`error`。

TypeScript 项目可以直接导入 `FastPlanConversationProps`、`FastPlanConversationEventMap`、`FastPlanConversationSlots` 和 `FastPlanConversationExpose`。组件实例也会暴露完整的规划执行命令控制器。

`FastPlanConversation` 默认复用 `chat-renderer` 的 `ChatComposer`。也可以直接从本包导入 `ChatComposer`、`ChatConversation` 及相关输入区类型，不需要切换包入口。

内置输入框维持“选择器、附件、输入控件、底部操作栏”四区域结构。区域插槽共享 `ChatComposerRegionScope`；`top-bar` 插槽额外获得 `selectors` 和 `select`，用于把受控选择器真正编排进宿主标题栏。FastPlan 会把规划预览、执行中和暂停状态分别映射为确认、暂停/停止、继续/停止操作；宿主仍可通过区域插槽覆盖显示方式。

模型回答底部的操作栏直接复用 `chat-renderer` 的 `ChatMessageItem` 和 `ChatActionIcon`。`FastPlanConversation` 不维护第二套按钮实现，只透传操作配置、`message-action` 事件以及 `action-bar`、`action` 插槽；这些组件和相关类型也可以从本包直接导入。

规划协议归属 `planflow-runtime`，规划面板和 composer 任务列表归属 `planflow-planning-ui`。它们通过 chat-renderer 的 `message-after`、`message-after-content` 和 composer 插槽注入，`chat-renderer` 本身不包含 FastPlan Plan 类型或组件。

主题支持 `light`、`dark` 和 `auto`。`themeTokens` 可统一覆盖聊天、规划和结果区域，支持 `background`、`foreground`、`muted`、`border`、`primary`、`userBackground`、`assistantBackground`、`danger`、`codeBackground`、`surface`、`subtleBackground`、`radius`、`fontSize`、`gap` 和 `bubbleMaxWidth`。也可以直接传入以 `--` 开头的自定义 CSS 变量。

运行时负责单个会话的消息、TODO、规划历史、暂停/失败/人工确认状态和执行快照。宿主仍可独立管理会话列表、路由与项目导航。
