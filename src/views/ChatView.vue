<script setup lang="ts">
import { fastClawFastPlanAccess } from '@/integrations/chat-plan/fastplanAccess';
// FastClaw · 智能对话主页（chatGLM 风格重写版）
// 三栏布局 + 顶置选择器 + chatGLM 风格底部按钮
import {
  ref,
  shallowRef,
  reactive,
  computed,
  onMounted,
  onUnmounted,
  onBeforeUnmount,
  onDeactivated,
  nextTick,
  watch,
  onActivated,
  provide,
  inject,
  defineComponent,
  h,
} from "vue";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import { useRouter, useRoute } from "vue-router";
import { setForegroundConversation } from '@/utils/conversationActivity';
import ChatHistorySidebar from '@/components/chat/workspace/ChatHistorySidebar.vue';
import ComposerAttachmentCard from '@/components/chat/workspace/ComposerAttachmentCard.vue';
import ComposerContextHints from '@/components/chat/workspace/ComposerContextHints.vue';
import ComposerSelectors from '@/components/chat/workspace/ComposerSelectors.vue';
import ComposerAction from '@/components/chat/workspace/ComposerAction.vue';
import { useChatViewEffects } from '@/modules/chat/useChatViewEffects';
import { useChatViewIdentity } from '@/modules/chat/viewIdentity';
import { captureChatCommand, createChatContext, createChatContextBinding } from '@/modules/chat/contextBinding';
import { createMessageEditorBinding, type MessageEditStore } from '@/modules/chat/messageEditorBinding';
import { createInlineQuestionBinding } from '@/modules/chat/inlineQuestionBinding';
import { captureSendPreparation, type SendPreparation } from '@/modules/chat/sendPreparation';
import { createRemotePlanConfirmationBinding, createWorkspaceAbortBinding } from '@/modules/chat/planningControlBinding';
import { createComposerContextBindings } from '@/modules/chat/composerContextBindings';
import { createContextFilePicker } from '@/modules/chat/contextFilePicker';
import { captureBrowserScreenshot } from '@/modules/chat/browserScreenshot';
import { startNativeScreenshot } from '@/modules/chat/nativeScreenshot';
import { useChatTimelinePresentation } from '@/modules/chat/useChatTimelinePresentation';
import { useHistoryActivitySummary, type FavoriteHistoryItem as FavItem } from '@/modules/chat/historySidebarPresentation';
import { navigateToConversation, openConversationInTab } from '@/utils/chatNavigation';
import { cancelPendingConversation, findConversationTab } from '@/utils/chatTabRegistry';
import { useChatStore, type AnswerChatStore } from "@/stores/chat";
import { answerSource, captureAnswerTarget } from '@/modules/chat/answerTarget';
import { DEFAULT_TASK_PLAN_ENABLED, getChatTabState } from '@/modules/chat/tabState';
import { createConversationBranchBinding, type ConversationBranchStore } from '@/modules/chat/conversationBranchBinding';
import { createMessageDeletionBinding, type MessageDeletionStore } from '@/modules/chat/messageDeletionBinding';
import { messageDeletionService } from '@/modules/chat/messageDeletion';
import { saveDocumentEditState } from '@/modules/chat/documentEditRuntime';
import { uploadTabDocument, recognizeTabImage } from '@/modules/chat/attachmentTasks';
import { recognizeImageText } from '@/modules/chat/imageOcr';
import { captureReadingPosition, restoreReadingPosition } from '@/modules/chat/readingPosition';
import { useAppStore } from "@/stores/app";
import { useSettingsStore } from "@/stores/settings";
import { useAppConfigStore } from "@/stores/appConfig";
import { useThemeStore } from "@/stores/theme";
import { useKnowledgeBaseStore } from "@/stores/index";
import { useKnowledgeFileDrop, type KnowledgeSelectorHandle } from '@/composables/useKnowledgeFileDrop';
import { useAgentStore } from "@/stores/agents";
import { usePreviewPanelStore } from "@/stores/previewPanelStore";
import type { Agent } from "@/stores/agents";
import SearchClearButton from "@/components/common/SearchClearButton.vue";
import MessageBubble from "@/components/chat/MessageBubble.vue";
import ModelAnswerGroup from "@/components/chat/ModelAnswerGroup.vue";
import QuestionNav from "@/components/chat/QuestionNav.vue";
import { useTraceSourceActions } from "@/components/chat/TraceSource.vue";
import WebSearchSources from "@/components/chat/WebSearchSources.vue";
import PreviewPanel from "@/components/chat/PreviewPanel.vue";
import PromptPanel from "@/components/chat/legacy/PromptPanel.vue";
import CommandPalette from "@/components/chat/CommandPalette.vue";
import { useSlashCommands, type SlashCommand } from "@/composables/useSlashCommands";
import { invoke } from "@tauri-apps/api/core";
import {
  getProviderRuntimeConfig,
  updateProvider,
} from "@/modules/models/api";
import { isChatModelCatalogStale, loadChatModelCatalog } from "@/modules/models/chatCatalog";
import { writeFile } from "@tauri-apps/plugin-fs";
import LazyChatPanel from '@/components/chat/workspace/LazyChatPanel.vue';
import { loadMessageEditor, loadDocumentEditor } from '@/modules/chat/optionalPanels';
import DocVersionCard from "@/components/chat/DocVersionCard.vue";
import ScreenshotOverlay from "@/components/chat/ScreenshotOverlay.vue";
import PlanningWelcome from '@/components/chat/workspace/PlanningWelcome.vue';
import type FastPlanView from '@/components/chat/fastPlan/FastPlanView.vue';
import ModelSelector from "@/components/chat/ModelSelector.vue";
import { useFastPlanStore } from "@/components/chat/fastPlan/session";
import selectModel from "@/components/chat/selectModel.vue";
import KnowledgeBaseSelect from "@/components/chat/knowledgeBaseSelect.vue";
import { parseTags, toTagObjects } from "@/utils/modelUtils";
import type { Conversation, Message, ChatAttachment } from "@/types";
import type { CaptureMetadata } from "@/types/screenshot";
import { getHttpPort } from "@/api/knowledge";
import { FASTKG_AUTH_HEADERS, SERVICE_URL_DEFAULTS } from '@/config/constants';
import { ElMessage, ElMessageBox } from "element-plus";
import SvgIcon from "@/components/svgIcon/index.vue";
import {
  Bottom,
  DocumentDelete,
  Close,
  Promotion,
  VideoPause,
  Picture,
  Document as DocumentIcon,
  MagicStick,
  Cpu,
  Setting,
  ChatLineRound,
  Camera,
  ArrowRight,
  CirclePlus,
  Loading,
  CircleCheck,
  WarningFilled,
  View,
  Hide,
} from "@element-plus/icons-vue";
import logoSvg from "../assets/logo.svg?raw";
import agentDefaultIcon from "../assets/agent/card_icon.png";
import { appLogo } from "@/api/branding";
import pdfIcon from "@/assets/pdf.png";
import pngIcon from "@/assets/png.png";
import txtIcon from "@/assets/txt.png";
import wordIcon from "@/assets/word.png";
import xlsxIcon from "@/assets/xlsx.png";
import { FileUp } from "lucide-vue-next";
import WebSearchPopover from "@/components/chat/WebSearchPopover.vue";
import McpToolPopover from "@/components/chat/McpToolPopover.vue";
import { useWebSearchStore } from "@/stores/websearch";
import { useMcpChatStore } from "@/stores/mcpChat";
import { useChatToolsStore } from "@/stores/chatTools";
import { useMarkdown } from "@/composables/useMarkdown";
import { useAssistantStore } from "@/stores/assistant";
import { isHuizhiProvider } from '@/utils/provider'
import {
  ChatWorkspace,
  ChatTimeline,
  type ChatComposerAction,
  type ChatComposerAttachment,
  type ChatComposerFileSelectionEvent,
  type ChatComposerSubmitEvent,
  type ChatMessage as RendererChatMessage,
  type ChatWorkspaceActionEvent,
  type MessageActionEvent,
} from 'chat-renderer';
import 'chat-renderer/style.css';
import { FASTCLAW_MESSAGE_ACTIONS, createResolveFastClawMessageActions } from '@/integrations/chat-plan/actionAdapter';
import { isChatPlanPackagesEnabled } from '@/integrations/chat-plan/featureFlags';
import { FASTCLAW_CHAT_THEME_TOKENS } from '@/integrations/chat-plan/themeBridge';
import FastClawContextDivider from '@/integrations/chat-plan/FastClawContextDivider.vue';
import FastClawCustomPart from '@/integrations/chat-plan/FastClawCustomPart.vue';
import { createProviderCompletion, providerParameterFields } from '@/integrations/chat-plan/providerCompletion';
import FastClawMessageActionIcon from '@/integrations/chat-plan/FastClawMessageActionIcon.vue';
import { exportMarkdown, exportWord, exportPdf } from '@/utils/exportMessage';
import {
  createFastPlanTimelinePlugin,
  type FastPlanTimelineEntryData,
  type FastPlanTimelinePluginOptions,
} from 'planflow-kit';
import 'planflow-kit/style.css';
import type { ChatEmbeddedTimelineEntry } from 'chat-renderer';
import {
  adaptFastPlanTaskPlanExecution,
} from 'planflow-runtime';
import { FastPlanTaskList } from 'planflow-planning-ui';
import { resolveFastPlanServiceConfig } from '@/integrations/chat-plan/serviceConfig';
import { fastPlanSessionManager } from '@/integrations/chat-plan/sessionManager';
import { retainedComposerEntry } from '@/integrations/chat-plan/tabRuntimeBridge';
import { planningSubmissionService } from '@/integrations/chat-plan/planningSubmission';
import { planningPreparationService, type PlanningPreparationStore } from '@/integrations/chat-plan/planningPreparation';
import { deferredChatSubmissionService, type DeferredChatStore } from '@/modules/chat/deferredSubmission';
import { documentSubmissionService, type DocumentSubmissionStore } from '@/modules/chat/documentSubmission';
import { createDocumentEditorBinding } from '@/modules/chat/documentEditorBinding';
import { createMessageActionBinding } from '@/modules/chat/messageActionBinding';
import { createQualityFeedbackBinding } from '@/modules/chat/qualityFeedbackBinding';
import { createMessagePresentationBinding } from '@/modules/chat/messagePresentationBinding';
import { createSourceInteractionBinding, CHAT_SOURCE_LINKS } from '@/modules/chat/sourceInteractionBinding';
import { createMessageExportBinding, CHAT_MESSAGE_EXPORT } from '@/modules/chat/messageExportBinding';
import { persistManagedFastPlanState, type PlanningConversationStore } from '@/integrations/chat-plan/managedPersistence';
import { chatSwitchPerf } from '@/utils/chatSwitchPerf';
import {
  persistedEntriesToTimeline,
  readFastPlanPersistence,
  releaseFastPlanWriter,
} from '@/integrations/chat-plan/persistence';
import {
  remotePlanLiveBySession,
} from '@/integrations/chat-plan/remotePlanRuntime';
import { usePlanActivityStore } from '@/stores/planActivity';
import {
  useExecutionActivityStore,
  type ExecutionPhase,
} from '@/stores/executionActivity';
import { useFastPlanServiceStatusStore } from '@/stores/fastPlanServiceStatus';

const route = useRoute();
const viewIdentity = useChatViewIdentity();
const chat = useChatStore(viewIdentity.instanceId);
const answerChat = chat as unknown as AnswerChatStore & MessageEditStore & ConversationBranchStore & MessageDeletionStore;
const tabState = getChatTabState(viewIdentity.instanceId);
// R4-B1: retain fixed legacy business references; only the public workspace UI is leased.
const initialChatContext = createChatContext(viewIdentity, tabState, answerChat);
const chatContextBinding = createChatContextBinding<AnswerChatStore & MessageEditStore & ConversationBranchStore & MessageDeletionStore>(context => route.path === context.identity.path);
if (route.path === viewIdentity.path) chatContextBinding.activate(initialChatContext);
const restoringTabMemory = tabState.initialized;
tabState.initialized = true;
let viewDisposed = false;
const appStore = useAppStore();
const ws = useWebSearchStore();
const mcpChat = useMcpChatStore();
// MCP enable/selection affects send parameters, so hydrate its small config
// once globally; the heavier server list remains lazy in McpToolPopover.
void mcpChat.load();
const settingsStore = useSettingsStore();
const agentStore = useAgentStore();
const chatTools = useChatToolsStore();
const assistantStore = useAssistantStore();
const appConfigStore = useAppConfigStore();
const themeStore = useThemeStore();
const planActivity = usePlanActivityStore();
const executionActivity = useExecutionActivityStore();
const fastPlanServiceStatus = useFastPlanServiceStatusStore();
// 跟随 draft：主题选择弹窗内点选即可预览；取消会还原，确定后与 current 一致。
const workspaceThemeMode = computed<'light' | 'dark'>(() =>
  themeStore.draftPreset.id === 'simple-light' || themeStore.draftPreset.id === 'scene-graphite-cyan'
    ? 'light'
    : 'dark',
);

// 发布版无账户体系，用户头像固定使用本地默认资源。
import personAvatar from '@/assets/sex/person.png'
const userAvatarError = ref(false)
const userAvatarSrc = computed(() => personAvatar)
function onChatAvatarChanged() {
  userAvatarError.value = false
}
// 初始化激活角色列表，并监听后端激活事件
assistantStore.ensureActiveRoleLoaded();
assistantStore.ensureRolesLoaded();
void assistantStore.ensureRoleEventListener();
function onRoleChange(val: unknown) {
  if (val) assistantStore.activateRole(String(val));
  else assistantStore.deactivateRole();
}

const { t } = useI18n();
const resolveFastClawMessageActions = createResolveFastClawMessageActions({
  translatingLabel: () => t('chat.translating'),
});
const router = useRouter();
/** TabBar 提供：删除已打开会话时同步关掉对应 tab */
const closeTab = inject<(path: string) => Promise<void>>('closeTab', async () => {});
// 标题同步到 tab：当前会话标题变化（首次问答后由原文或模型概括得到）时更新对应 tab。
// myPath 来自路由外壳创建时的身份快照：useRoute() 返回的是全局共享路由对象，
// 所有被 keep-alive 缓存的 ChatView 实例共享同一 route，故 watcher 内不能直接读 route.path，
// 否则后台实例标题异步变化时会被错写到前台 tab。
const myPath = viewIdentity.path
const viewEffects = useChatViewEffects(() => route.path === myPath);
const planWriterId = tabState.planWriterId
chatSwitchPerf.registerChatView(myPath)
watch(
  () => chat.conversations.find((c) => c.id === chat.currentConversationId)?.title,
  (title) => {
    if (title && title !== '智能对话') {
      appStore.setTabTitle(myPath, title)
    }
  },
)
// 登记本实例为「空白新建对话」tab：当前会话为空（无 currentConversationId）时登记，
// 首次发消息后由该 watch 自动撤销。供 newCreateChat 复用，避免重复创建空 tab。
watch(
  () => chat.currentConversationId,
  (id) => {
    if (id) appStore.unregisterBlankChatTab(myPath)
    else appStore.registerBlankChatTab(myPath)
  },
  { immediate: true },
)
onUnmounted(() => {
  appStore.unregisterBlankChatTab(myPath)
  chatSwitchPerf.unregisterChatView(myPath)
})
const PRESET_CHIPS = ['视觉', '联网', '推理', '工具', '重排', '嵌入']

// ─── 引用 ───
const scrollContainerRef = ref<HTMLDivElement | null>(null);
const elScrollbarRef = ref<any>(null);
/** 新版 ChatWorkspace（chat-renderer）实例，用于滚到底 */
const chatWorkspaceRef = ref<{ scrollToBottom?: (behavior?: ScrollBehavior) => void } | null>(null);
const shouldAutoScroll = tabState.state.shouldAutoScroll;
// 问题导航分页：用于限制导航条高度不超过输入框
const inputAreaRef = ref<HTMLElement | null>(null);
const chatMainRef = ref<HTMLElement | null>(null);
const navMaxHeight = ref(0);
const navTop = ref(0);
let navResizeObserver: ResizeObserver | null = null;
/**
 * 手动拖高后的期望最小高度（px）；null = 默认自适应。
 * 附件、引用栏等动态内容所需空间超过该值时，输入区仍可继续向上扩展。
 * 仅存在于当前 ChatView 实例（keep-alive 切走再回来仍保留）；
 * 菜单新开 /chat/:iid 是新实例，始终从默认高度开始。
 * 不写 sessionStorage，避免新开对话继承旧高度。
 */
const inputAreaHeightPx = tabState.state.inputAreaHeightPx;
/** 首次拖动时锁定的最低高度（当前自然高度） */
const inputAreaMinHeightPx = tabState.state.inputAreaMinHeightPx;
const isResizingInputArea = ref(false);
let inputResizeStartY = 0;
let inputResizeStartH = 0;

/** chat-main 未就绪/隐藏时返回 null，避免把高度误钳到最小值 */
function getInputAreaMaxHeight(): number | null {
  const main = chatMainRef.value;
  if (!main) return null;
  const h = main.clientHeight;
  // 切换 keep-alive 页面时 clientHeight 可能短暂为 0
  if (h < 120) return null;
  return Math.floor(h / 2);
}

function clampInputAreaHeight(h: number) {
  const minH = inputAreaMinHeightPx.value || 100;
  let next = Math.max(minH, Math.round(h));
  const maxH = getInputAreaMaxHeight();
  if (maxH != null) next = Math.min(maxH, next);
  // max < min（窗口极矮）时仍不低于 min，避免被布局抖动压扁
  return Math.max(minH, next);
}

function clampResizedInputArea() {
  if (inputAreaHeightPx.value == null) return;
  // 布局未稳定时绝不改写高度（切换界面时曾被误压回最小值）
  if (getInputAreaMaxHeight() == null) return;
  inputAreaHeightPx.value = clampInputAreaHeight(inputAreaHeightPx.value);
}

let inputResizeCapture: { element: HTMLElement; pointerId: number } | null = null;
function onInputAreaResizeStart(e: PointerEvent) {
  if (!viewEffects.isActive()) return;
  const el = useChatPlanPackages.value
    ? chatMainRef.value?.querySelector<HTMLElement>('.jcr-conversation__footer')
    : inputAreaRef.value;
  if (!el) return;
  e.preventDefault();
  e.stopPropagation();
  const currentH = el.offsetHeight;
  if (!inputAreaMinHeightPx.value) {
    inputAreaMinHeightPx.value = currentH;
  }
  inputResizeStartY = e.clientY;
  inputResizeStartH = inputAreaHeightPx.value ?? currentH;
  isResizingInputArea.value = true;
  const captureElement = e.currentTarget as HTMLElement | null;
  captureElement?.setPointerCapture?.(e.pointerId);
  if (captureElement) inputResizeCapture = { element: captureElement, pointerId: e.pointerId };
  window.addEventListener("pointermove", onInputAreaResizeMove);
  window.addEventListener("pointerup", onInputAreaResizeEnd);
  window.addEventListener("pointercancel", onInputAreaResizeEnd);
}

function onInputAreaResizeMove(e: PointerEvent) {
  if (!isResizingInputArea.value) return;
  // 向上拖 → 增高
  const delta = inputResizeStartY - e.clientY;
  inputAreaHeightPx.value = clampInputAreaHeight(inputResizeStartH + delta);
}

function onInputAreaResizeEnd() {
  if (!isResizingInputArea.value) return;
  isResizingInputArea.value = false;
  if (inputResizeCapture) {
    const { element, pointerId } = inputResizeCapture;
    if (element.hasPointerCapture?.(pointerId)) element.releasePointerCapture(pointerId);
    inputResizeCapture = null;
  }
  window.removeEventListener("pointermove", onInputAreaResizeMove);
  window.removeEventListener("pointerup", onInputAreaResizeEnd);
  window.removeEventListener("pointercancel", onInputAreaResizeEnd);
  viewEffects.tick(() => recomputeNavMaxHeight());
}

const inputAreaStyle = computed(() => {
  if (inputAreaHeightPx.value == null) return undefined;
  return { minHeight: `${inputAreaHeightPx.value}px` };
});

const defaultUserAvatarCss = `url("${personAvatar}")`;
const workspaceInputAreaStyle = computed(() => {
  const style: Record<string, string> = {
    '--jcr-timeline-padding': '0px 32px',
    '--fastclaw-default-user-avatar': defaultUserAvatarCss,
  };
  if (inputAreaHeightPx.value != null) {
    style['--fastclaw-composer-height'] = `${inputAreaHeightPx.value}px`;
  }
  return style;
});
const chatMainStyle = computed(() => {
  const style: Record<string, string> = {
    '--fastclaw-default-user-avatar': defaultUserAvatarCss,
  };
  if (editorVisible.value) {
    style.flex = `0 0 ${chatPanelPx.value}px`;
    style.minWidth = `${CHAT_MIN_PX}px`;
  }
  return style;
});

/** 手动拉高后关闭 autosize，改由 CSS 填满剩余空间 */
const inputTextareaAutosize = computed(() => {
  if (inputAreaHeightPx.value != null) return false;
  return { minRows: 2, maxRows: 6 };
});

// 公共 Composer 使用 14px / 1.55 行高和上下各 11px 内边距；未手动拖拽时最多显示 6 行。
const workspaceComposerMaxHeightPx = Math.ceil(14 * 1.55 * 6 + 22);
const workspaceComposerAutoResize = computed(() => inputAreaHeightPx.value == null);

function recomputeNavMaxHeight() {
  if (!viewEffects.isActive()) return;
  clampResizedInputArea();
  const main = chatMainRef.value;
  const input = inputAreaRef.value;
  if (!main || !input) { navMaxHeight.value = 0; return; }
  // 页面隐藏时不要用 0 高度参与导航计算
  if (main.clientHeight < 120) return;
  const space = main.clientHeight - input.offsetHeight;
  navTop.value = Math.round(space * 0.1);
  navMaxHeight.value = Math.max(60, Math.round(space * 0.8));
}

function attachNavResizeObserver() {
  navResizeObserver?.disconnect();
  navResizeObserver = null;
  // 新版 ChatWorkspace 不渲染 QuestionNav，无需为隐藏功能触发布局读取。
  if (!viewEffects.isActive() || useChatPlanPackages.value || !chat.messages.length) return;
  recomputeNavMaxHeight();
  if (!chatMainRef.value || !inputAreaRef.value || typeof ResizeObserver === 'undefined') return;
  navResizeObserver = new ResizeObserver(viewEffects.guard(() => recomputeNavMaxHeight()));
  navResizeObserver.observe(chatMainRef.value);
  navResizeObserver.observe(inputAreaRef.value);
}
const textareaRef = ref<any>(null);
const contextFilePicker = createContextFilePicker(chatContextBinding);
onUnmounted(contextFilePicker.dispose);

// ─── 输入框状态 ───
const messageStr = tabState.state.messageStr;

// ─── Skill / MCP 选择（按钮弹出模式） ───
const selectedSkill = tabState.state.selectedSkill;

// ─── 斜杠命令 ───
const slash = useSlashCommands({
  input: messageStr,
  setInput: (v: string) => { messageStr.value = v },
  textareaRef,
  onExecuteCommand: async (_cmd: SlashCommand) => {
    // 不再将 skill 模板内容填入输入框——模板由后端 /skill_name 展开
    // 输入框只保留 /hello 这样的命令，发送后后端自动替换
  },
  onButtonSelect: (cmd: SlashCommand) => {
    const lease = chatContextBinding.captureUi();
    if (lease?.isCurrent()) lease.context.owner.state.selectedSkill.value = cmd;
  },
})

const fileList = tabState.state.fileList;
/** 附件栏中所有图片/截图的 preview_url，供 el-image 预览翻页 */
const imagePreviewList = computed<string[]>(() =>
  fileList.value
    .filter((f) => (f.type === 'image' || f.type === 'screenshot') && f.preview_url)
    .map((f) => f.preview_url as string),
);
/** 点击某张缩略图时，让 el-image 预览从它在列表中的下标打开 */
function imagePreviewIndex(f: ChatAttachment): number {
  return imagePreviewList.value.indexOf(f.preview_url as string);
}


/** Only resolve a file icon here; the card owns presentation, never the source File. */
function composerAttachmentIcon(attachment: ChatComposerAttachment): string | undefined {
  if (attachment.kind === 'image') return undefined;
  const id = String(attachment.metadata?.hostAttachmentId || attachment.id || '');
  const file = id ? fileList.value.find((f) => f.id === id) : undefined;
  return file ? getFileIcon(file) : undefined;
}


/** 保留原始 File 引用，供 OCR 直接读取（避免 blob URL fetch 的 CSP 问题） */
const fileRefs = tabState.files;
const fileKbIds = tabState.state.fileKbIds;

// File2Md OCR 下载并发控制（多图片同时上传时共享下载状态）

// ─── 键盘事件（含斜杠命令导航） ───
function handleChatKeydown(e: KeyboardEvent) {
  if (slash.handleKeydown(e)) return   // 斜杠菜单处理
  if (e.isComposing || (e as any).keyCode === 229) return   // 修复中文输入法误发
  const mode = settingsStore.settings.send_shortcut || 'enter'
  if (mode === 'alt_s') {
    if (e.altKey && e.key.toLowerCase() === 's') keydownEnterFn(e)
    return                                   // Alt+S 模式下 Enter 纯换行
  }
  if (e.key === 'Enter' && !e.shiftKey && !e.altKey && !e.ctrlKey) keydownEnterFn(e)
}
const conversationFileIds = tabState.state.conversationFileIds;  // 持久化 uploaded_file_id，跨轮次传递

// ─── 上传中自动发送标记 ───
const pendingAutoSend = tabState.state.pendingAutoSend;
const chatSubmissionError = tabState.state.chatSubmissionError;

// ─── 发送按钮防抖锁 ───
const sendBtnLock = tabState.state.sendBtnLock;

// ─── 追问引用 ───
const quotedText = tabState.state.quotedText;
watch([messageStr, quotedText], () => {
  // The queued send was confirmed for the previous draft, not a later edit.
  if (pendingAutoSend.value) pendingAutoSend.value = false;
}, { flush: 'sync' });

/** 问题气泡 medium-zoom 预览：滚轮放大/缩小（库本身不支持 wheel zoom） */
const MEDIUM_ZOOM_WHEEL_MIN = 0.4;
const MEDIUM_ZOOM_WHEEL_MAX = 4;
let mediumZoomScale = 1;
let mediumZoomBodyObserver: MutationObserver | null = null;

function mediumZoomOpenedImage(): HTMLElement | null {
  return document.querySelector('.medium-zoom-image--opened') as HTMLElement | null;
}

function applyMediumZoomScale(scale: number) {
  mediumZoomScale = Math.min(MEDIUM_ZOOM_WHEEL_MAX, Math.max(MEDIUM_ZOOM_WHEEL_MIN, scale));
  const img = mediumZoomOpenedImage();
  if (img) img.style.setProperty('--fc-medium-zoom-scale', String(mediumZoomScale));
  document.documentElement.style.setProperty('--fc-medium-zoom-scale', String(mediumZoomScale));
}

function onMediumZoomWheel(e: WheelEvent) {
  if (!document.body.classList.contains('medium-zoom--opened')) return;
  // 仅在预览打开时拦截滚轮，避免页面滚动触发 medium-zoom 关闭
  e.preventDefault();
  e.stopPropagation();
  const direction = e.deltaY < 0 ? 1 : -1;
  const factor = direction > 0 ? 1.12 : 1 / 1.12;
  applyMediumZoomScale(mediumZoomScale * factor);
}

function resetMediumZoomScale() {
  mediumZoomScale = 1;
  applyMediumZoomScale(1);
}

function bindMediumZoomWheelZoom() {
  if (mediumZoomBodyObserver || typeof window === 'undefined') return;
  let wasOpen = false;
  document.addEventListener('wheel', onMediumZoomWheel, { passive: false, capture: true });
  mediumZoomBodyObserver = new MutationObserver(() => {
    const open = document.body.classList.contains('medium-zoom--opened');
    if (open !== wasOpen) {
      resetMediumZoomScale();
      wasOpen = open;
    }
  });
  mediumZoomBodyObserver.observe(document.body, { attributes: true, attributeFilter: ['class'] });
}

function unbindMediumZoomWheelZoom() {
  if (!mediumZoomBodyObserver) return;
  document.removeEventListener('wheel', onMediumZoomWheel, true);
  mediumZoomBodyObserver?.disconnect();
  mediumZoomBodyObserver = null;
  resetMediumZoomScale();
}

// ─── 模型 / 知识库选择器 ───
interface ModelGroup {
  id: string;
  label: string;
  providerCode: string;
  sourceCategory: string;
  children: Array<{
    id: string;
    name: string;
    rawModelId: string;
    modelId: string;
    tags: Array<{
      value: string;
      label: string;
    }>;
    modelLabel: string;
    mxcs: string | null;
  }>;
}

/**
 * 根据供应商代码/分类获取图标名称，与 ModelsView.vue 保持一致。
 * 汇智网/私域网/本电脑使用固定图标；
 * 其他供应商使用 source_category 对应的图标（local→本地, online→互联网, huizhi→汇智网）。
 */
function getProviderIcon(providerCode: string, sourceCategory: string): string {
  const knownIcons: Record<string, string> = {
    HUIZHI: "CI",  // 汇智网
    PRIVATE: "lan", // 私域网
    LOCAL: "local",      // 本电脑
  };
  if (knownIcons[providerCode]) return knownIcons[providerCode];

  // 其他供应商：使用 source_category 映射
  const categoryIcons: Record<string, string> = {
    local: "local",
    online: "internet",
    huizhi: "CI",
  };
  return categoryIcons[sourceCategory] || "internet";
}

function groupLabel(g: { label: string; providerCode: string }): string {
  return g.label
}
const modelTree = ref<ModelGroup[]>([]);
const modelId = tabState.state.modelId;
let applyingRemoteModel = false;
let remoteModelSyncReady = false;
const searchModelStr = ref("");
const hideIncompleteModels = ref(false);
// 供应商配置（base_url / api_key），按 provider_code 索引
const providerConfigMap = ref<Record<string, {
  id: string;
  baseUrl: string;
  hasApiKey: boolean;
  apiKey?: string;
}>>({});
// 参数表单由 chat-renderer 提供；宿主只负责配置读取、保存和发送前等待。
const providerCompletion = createProviderCompletion({
  async load(selectedId) {
    const info = getProviderInfoForModel(selectedId);
    if (!info) throw new Error('所选模型不存在，请重新选择模型');
    // 汇智网不在这里补全供应商参数；私域网、本电脑需要补全。
    if (isHuizhiProvider(info.providerCode)) return null;
    // 每次发送读取最新值，避免设置页修改后使用旧的配置摘要。
    const runtime = await getProviderRuntimeConfig(info.providerCode);
    if (!runtime.id) throw new Error('供应商信息异常，无法读取配置');
    providerConfigMap.value[info.providerCode] = {
      id: runtime.id, baseUrl: runtime.base_url || '',
      hasApiKey: Boolean(runtime.api_key?.trim()), apiKey: runtime.api_key || '',
    };
    return { id: runtime.id, name: info.providerName, baseUrl: runtime.base_url || '', apiKey: runtime.api_key || '' };
  },
  async save(provider, values) {
    await updateProvider(provider.id, { base_url: values.baseUrl, api_key_ref: values.apiKey });
    for (const [code, config] of Object.entries(providerConfigMap.value)) {
      if (config.id === provider.id) providerConfigMap.value[code] = {
        ...config, baseUrl: values.baseUrl, hasApiKey: true, apiKey: values.apiKey,
      };
    }
  },
});
const vendorConfigPanel = providerCompletion.pending;
const showVendorApiKey = ref(false);
watch(vendorConfigPanel, (panel) => {
  if (!panel) showVendorApiKey.value = false;
});
let providerDraftTransition = false;
function vendorApiKeyEyeIcon(visible: boolean) {
  return visible
    ? h('svg', { viewBox: '0 0 24 24', width: '15', height: '15', fill: 'none', stroke: 'currentColor', strokeWidth: '2', strokeLinecap: 'round', strokeLinejoin: 'round' }, [
        h('path', { d: 'M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24' }),
        h('line', { x1: '1', y1: '1', x2: '23', y2: '23' }),
      ])
    : h('svg', { viewBox: '0 0 24 24', width: '15', height: '15', fill: 'none', stroke: 'currentColor', strokeWidth: '2', strokeLinecap: 'round', strokeLinejoin: 'round' }, [
        h('path', { d: 'M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z' }),
        h('circle', { cx: '12', cy: '12', r: '3' }),
      ]);
}
const VendorConfigRenderer = defineComponent({
  name: 'FastClawProviderCompletion',
  inheritAttrs: false,
  setup() {
    return () => {
      const panel = vendorConfigPanel.value;
      if (!panel) return null;
      const values = panel.values;
      const submitting = providerCompletion.saving.value;
      const incomplete = providerParameterFields.some((field) => field.required && !values[field.key]?.trim());
      return h('form', {
        class: 'jcr-parameter-form',
        'aria-busy': submitting ? 'true' : 'false',
        onSubmit: (event: Event) => {
          event.preventDefault();
          void providerCompletion.submit(panel.values);
        },
      }, [
        h('p', { class: 'jcr-parameter-form__description' }, [
          h('svg', {
            class: 'jcr-parameter-form__notice-icon',
            viewBox: '0 0 20 20',
            fill: 'none',
            stroke: 'currentColor',
            strokeWidth: '1.5',
            'aria-hidden': 'true',
          }, [
            h('circle', { cx: '10', cy: '10', r: '8' }),
            h('path', { d: 'M10 5.5v5M10 13.5v1' }),
          ]),
          h('span', `当前供应商「${panel.provider.name}」未配置完整参数（Base URL 或 API Key），请补充后再进行使用`),
        ]),
        ...providerParameterFields.map((field) => {
          const isSecret = field.type === 'password';
          const input = h('input', {
            type: isSecret && !showVendorApiKey.value ? 'password' : (isSecret ? 'text' : (field.type || 'text')),
            value: values[field.key] || '',
            placeholder: field.placeholder,
            required: field.required,
            disabled: submitting,
            autocomplete: 'off',
            spellcheck: false,
            onInput: (event: Event) => {
              panel.values = { ...panel.values, [field.key]: (event.target as HTMLInputElement).value };
            },
          });
          if (!isSecret) {
            return h('label', { key: field.key, class: 'jcr-parameter-form__field' }, [
              h('span', field.label),
              input,
            ]);
          }
          return h('label', { key: field.key, class: 'jcr-parameter-form__field jcr-parameter-form__field--secret' }, [
            h('span', field.label),
            h('div', { class: 'fastclaw-api-key-wrap' }, [
              input,
              h('button', {
                type: 'button',
                class: 'fastclaw-api-key-toggle',
                title: showVendorApiKey.value ? '隐藏' : '显示',
                'aria-label': showVendorApiKey.value ? '隐藏 API Key' : '显示 API Key',
                onClick: (event: Event) => {
                  event.preventDefault();
                  showVendorApiKey.value = !showVendorApiKey.value;
                },
              }, [vendorApiKeyEyeIcon(showVendorApiKey.value)]),
            ]),
          ]);
        }),
        providerCompletion.error.value
          ? h('p', { class: 'jcr-parameter-form__error', role: 'alert' }, providerCompletion.error.value)
          : null,
        h('div', { class: 'jcr-parameter-form__actions' }, [
          h('button', { type: 'submit', disabled: submitting || incomplete }, '确认并发送'),
          h('button', {
            type: 'button',
            disabled: submitting,
            onClick: () => providerCompletion.cancel(),
          }, t('common.cancel')),
        ]),
      ]);
    };
  },
});
async function ensureModelProviderReady(selectedId: string, content?: string, preparation?: SendPreparation): Promise<boolean> {
  if (!selectedId) {
    ElMessage.warning(t('chat.pleaseSelectModel'));
    return false;
  }
  const draft = messageStr.value;
  const quote = quotedText.value;
  const conversationId = chat.currentConversationId;
  const mode = dialogueCharacter.value.taskPlan;
  const isCurrentDraft = tabState.capture();
  let movedToTimeline = false;
  try {
    const ready = await providerCompletion.ensureReady(selectedId, content === undefined ? undefined : {
      message: {
        protocolVersion: 1, id: `provider-user:${crypto.randomUUID()}`, role: 'user',
        createdAt: Date.now(), status: 'complete',
        parts: [
          { type: 'text', text: content, format: 'markdown' },
          ...fileList.value.map(file => ({ type: 'attachment' as const, id: file.id, name: file.name, size: file.size })),
        ],
        metadata: { vendorConfig: true },
      },
      onRequired() {
        if (preparation) {
          providerDraftTransition = true;
          try { if (!preparation.moveDraft()) providerCompletion.cancel(); }
          finally { providerDraftTransition = false; }
          return;
        }
        // 先呈现已发送气泡；实际发送时由原流程持久化一次，避免重复消息。
        movedToTimeline = true;
        providerDraftTransition = true;
        try { messageStr.value = ''; quotedText.value = ''; }
        finally { providerDraftTransition = false; }
      },
    }, preparation?.signal);
    if (!preparation && !ready && isCurrentDraft() && movedToTimeline && chat.currentConversationId === conversationId
      && dialogueCharacter.value.taskPlan === mode && !messageStr.value) {
      messageStr.value = draft;
      if (!quotedText.value) quotedText.value = quote;
    }
    return ready;
  } catch (error) {
    if (!preparation || preparation.isCurrent()) ElMessage.error(typeof error === 'string' ? error : (error as Error)?.message || '读取供应商配置失败');
    return false;
  }
}
const lastUsableModelId = tabState.state.lastUsableModelId;

// 判断某模型是否具备指定能力标签（基于 modelLabel 逗号分隔标签）
function modelHasCapability(m: { modelLabel?: string }, cap: string): boolean {
  if (!cap) return true;
  return parseTags(m.modelLabel).includes(cap);
}
const showModelList = computed<ModelGroup[]>(() => {
  const kw = searchModelStr.value.trim().toLowerCase();
  const cap = imageCapabilityTag.value;
  let list = modelTree.value

  // 隐藏不可用模型：过滤掉供应商配置不完整的分组
  if (hideIncompleteModels.value) {
    list = list.filter((g) => !isVendorIncomplete(g.providerCode))
  }

  if (!kw && !cap) return list;
  return list
    .map((g) => ({
      id: g.id,
      label: g.label,
      providerCode: g.providerCode,
      sourceCategory: g.sourceCategory,
      children: g.children.filter(
        (m) =>
          (!kw || m.name.toLowerCase().includes(kw)) &&
          modelHasCapability(m, cap),
      ),
    }))
    .filter((g) => g.children.length > 0);
});

// 当前选中模型所属分组（用于获取图标）
const selectedModelGroup = computed(() => {
  if (!modelId.value) return null;
  for (const g of modelTree.value) {
    if (g.children.some((m) => m.id === modelId.value)) return g;
  }
  return null;
});

const selectedModelIcon = computed(() => {
  if (!selectedModelGroup.value) return "internet";
  return getProviderIcon(
    selectedModelGroup.value.providerCode,
    selectedModelGroup.value.sourceCategory,
  );
});

/** 根据模型 ID 查找其供应商信息 */
function getProviderInfoForModel(id: string): { providerCode: string; providerName: string } | null {
  for (const g of modelTree.value) {
    if (g.children.some((m) => m.id === id)) {
      return { providerCode: g.providerCode, providerName: g.label };
    }
  }
  return null;
}

/** 判断供应商是否未配置完整参数（baseUrl 或 apiKey 任一缺失） */
function isVendorIncomplete(providerCode: string): boolean {
  // 仅汇智网豁免参数补全，与发送前校验保持一致。
  if (isHuizhiProvider(providerCode)) return false
  const cfg = providerConfigMap.value[providerCode]
  if (!cfg) return false
  return !cfg.baseUrl.trim() || !cfg.hasApiKey
}

async function loadProviderRuntimeConfig(providerCode: string) {
  const current = providerConfigMap.value[providerCode]
  if (!current || current.apiKey !== undefined) return current
  try {
    const runtime = await getProviderRuntimeConfig(providerCode)
    const next = {
      ...current,
      id: runtime.id,
      baseUrl: runtime.base_url,
      hasApiKey: Boolean(runtime.api_key),
      apiKey: runtime.api_key || '',
    }
    providerConfigMap.value[providerCode] = next
    return next
  } catch (error) {
    console.warn('get_provider_runtime_config failed', error)
    return current
  }
}

let modelSelectionSequence = 0;
watch(modelId, () => { modelSelectionSequence++; }, { flush: 'sync' });
async function onChatModelChange(id: string | undefined) {
  const sequence = ++modelSelectionSequence;
  const lease = chatContextBinding.captureUi();
  if (!lease?.isCurrent()) return;
  const target = lease.context.owner.state;
  clearMxcsPanel();
  if (!id) return;

  target.lastUsableModelId.value = id;
}

// 选中第一个支持指定能力标签的模型，返回其 id（无则返回空串）
function pickModelForCapability(cap: string): string {
  for (const g of modelTree.value) {
    const m = g.children.find((c) => modelHasCapability(c, cap));
    if (m) return m.id;
  }
  return "";
}

// 当前所选模型是否具备指定能力
function currentModelHasCapability(cap: string): boolean {
  if (!cap) return true;
  return modelTree.value.some((g) =>
    g.children.some((c) => c.id === modelId.value && modelHasCapability(c, cap)),
  );
}


const knowledgeId = tabState.state.knowledgeId;
const dropSessionRevision = ref(0);
const kbStore = useKnowledgeBaseStore();

// 从混合选中 ID 中分离远端 KB ID 和本地 KB 路径
const remoteKbIds = computed(() =>
  knowledgeId.value.filter((id) =>
    kbStore.knowledgeBases.some((k) => k.id === id),
  ),
);
const localKbPaths = computed(() =>
  kbStore.getEnabledLocalPaths(knowledgeId.value),
);

const isScrolling = ref(false);
const isSelectVisible = ref(false);
const hoveredMxcsModel = ref<any>(null);
const mxcsPanelRect = ref<any>(null);
let dropdownScrollEl: Element | null = null;
let cancelDropdownBind: (() => void) | undefined;
let cancelScrollingTimer: (() => void) | undefined;
const modelPopperOwnerClass = `fastclaw-model-popper-${crypto.randomUUID()}`;
function getOwnedModelPopper() {
  return document.querySelector(`.${modelPopperOwnerClass}`);
}
const mxcsPanelStyle = computed(() => {
  if (!mxcsPanelRect.value) return {} as Record<string, string>;
  const { left, top, height } = mxcsPanelRect.value;
  return {
    position: 'fixed',
    left: `${left}px`,
    top: `${top}px`,
    height: `${height}px`,
    width: 'max-content',
    maxWidth: '480px',
    minWidth: '200px',
    zIndex: '1998',
    border: '1px solid var(--border-color)',
    pointerEvents: 'none',
  } as Record<string, string>;
});
const onMxcsHover = (item: any) => {
  if (!isSelectVisible.value || isScrolling.value || !item.mxcs) {
    hoveredMxcsModel.value = null;
    return
  };
  hoveredMxcsModel.value = item;
  const popperEl = getOwnedModelPopper();
  if (!viewEffects.isActive() || !popperEl) return;
  const popperRect = popperEl.getBoundingClientRect();
  if (popperRect.height > 0) {
    mxcsPanelRect.value = { left: popperRect.right - 1, top: popperRect.top, height: popperRect.height };
  }
};
// 下拉框滚动时锁定 hover 面板，防止内容跳变
const onDropdownScroll = () => {
  if (!viewEffects.isActive()) return;
  isScrolling.value = true;
  cancelScrollingTimer?.();
  cancelScrollingTimer = viewEffects.delay(() => { isScrolling.value = false; }, 300);
};
const onSelectVisibleChange = (visible: boolean) => {
  cancelDropdownBind?.();
  cancelScrollingTimer?.();
  dropdownScrollEl?.removeEventListener('scroll', onDropdownScroll);
  dropdownScrollEl = null;
  isScrolling.value = false;
  isSelectVisible.value = visible && viewEffects.isActive();
  if (isSelectVisible.value) {
    cancelDropdownBind = viewEffects.delay(() => {
      const popperEl = getOwnedModelPopper();
      if (!isSelectVisible.value || !popperEl?.isConnected) return;
      dropdownScrollEl = popperEl.querySelector('.el-scrollbar__wrap');
      dropdownScrollEl?.addEventListener('scroll', onDropdownScroll, { passive: true });
    }, 300);
  } else {
    hoveredMxcsModel.value = null;
    mxcsPanelRect.value = null;
  }
};
const clearMxcsPanel = () => {
  hoveredMxcsModel.value = null;
  mxcsPanelRect.value = null;
};

// ─── 对话角色（chatGLM 风格）───
const dialogueCharacter = tabState.state.dialogueCharacter;

// ─── 右侧面板：提示词 / 溯源 / 节点详情 ───
// 取远程 R1 重构后的 tabState 版本（右侧面板状态按 tab 隔离）。
// 注意：本地的右侧预览面板需要 'preview' 档，该档已同步补进 tabState 的联合类型，
// 否则下面 `rightPanel.value = "preview"` 会因不在联合类型内而报错。
const rightPanel = tabState.state.rightPanel;
const isHistoryCollapsed = tabState.state.isHistoryCollapsed;

// 右侧网页预览面板：显隐以 previewPanelStore.isOpen 为唯一事实来源，这里只同步 rightPanel 以复用 aside 布局
const previewStore = usePreviewPanelStore();
watch(
  () => previewStore.isOpen,
  (open) => {
    if (open) rightPanel.value = "preview";
    else if (rightPanel.value === "preview") rightPanel.value = "none";
  },
);

// ─── 编辑器分屏 ───
const editorVisible = tabState.state.editorVisible;
const editorContent = tabState.state.editorContent;
const editorTitle = tabState.state.editorTitle;
const chatPanelPx = tabState.state.chatPanelPx; // 聊天区像素宽度（分屏时生效）
const isDraggingSplit = ref(false);
const chatviewShellRef = ref<HTMLDivElement | null>(null);

// ─── 文档编辑模式：版本历史 ───
interface DocVersion {
  label: string;
  content: string;
  timestamp: string;
}
const docVersions = tabState.state.docVersions;
const activeVersionIdx = tabState.state.activeVersionIdx;

// doc-edit 消息标识：key = messageId，用于隐藏"转为文档编辑"按钮 + 渲染版本卡片
interface DocCardInfo { versionIdx: number; label: string; timestamp: string }
const docEditCardMap = tabState.state.docEditCardMap;
const docEditMessageIds = tabState.state.docEditMessageIds;

// ─── Doc-edit 状态持久化（localStorage，按对话 ID 分开存储）───
interface PersistedDocEditState {
  versions: DocVersion[];
  activeVersionIdx: number;
  cardMap: Record<string, DocCardInfo>;
  messageIds: Record<string, true>;
  title: string;
  visible: boolean;
  draftContent?: string;
}

function _docEditStorageKey(convId: string) {
  return `fastclaw:docEdit:${convId}`;
}

function saveDocEditState() {
  saveDocumentEditState(tabState, chat.currentConversationId);
}

function loadDocEditState(convId: string) {
  const raw = convId ? localStorage.getItem(_docEditStorageKey(convId)) : null;
  // 先清空
  docVersions.value = [];
  activeVersionIdx.value = 0;
  Object.keys(docEditCardMap).forEach((k) => delete docEditCardMap[k]);
  Object.keys(docEditMessageIds).forEach((k) => delete (docEditMessageIds as Record<string, true>)[k]);
  editorTitle.value = "";
  editorVisible.value = false;
  editorContent.value = "";
  if (!raw) return;
  try {
    const state: PersistedDocEditState = JSON.parse(raw);
    docVersions.value = state.versions || [];
    activeVersionIdx.value = state.activeVersionIdx ?? 0;
    Object.assign(docEditCardMap, state.cardMap || {});
    Object.assign(docEditMessageIds, state.messageIds || {});
    editorTitle.value = state.title || "";
    editorVisible.value = state.visible ?? false;
    // 恢复当前活跃版本内容
    const active = docVersions.value[activeVersionIdx.value];
    editorContent.value = typeof state.draftContent === 'string' ? state.draftContent : active?.content || '';
  } catch { /* 忽略损坏的存储 */ }
}

function onSplitDragStart(e: MouseEvent) {
  if (!viewEffects.isActive()) return;
  isDraggingSplit.value = true;
  document.body.classList.add("fastclaw-chat-split-dragging");
  document.addEventListener("mousemove", onSplitDragMove);
  document.addEventListener("mouseup", onSplitDragEnd);
  e.preventDefault();
}

const CONV_ASIDE_WIDTH = 234; // 左侧历史栏宽度（与 CSS 保持一致）
const DIVIDER_WIDTH = 3;
const EDITOR_MIN_PX = 550; // 编辑器最小宽度
const CHAT_MIN_PX = 280;   // 聊天区最小宽度

function getSidebarWidth() {
  return isHistoryCollapsed.value ? 0 : CONV_ASIDE_WIDTH;
}

function getDefaultChatPanelPx(): number {
  const shellW = chatviewShellRef.value?.getBoundingClientRect().width ?? 900;
  const available = shellW - getSidebarWidth() - DIVIDER_WIDTH;
  return Math.floor(available / 2);
}

function onSplitDragMove(e: MouseEvent) {
  if (!isDraggingSplit.value || !chatviewShellRef.value) return;
  const shellRect = chatviewShellRef.value.getBoundingClientRect();
  const sidebarW = getSidebarWidth();
  const available = shellRect.width - sidebarW - DIVIDER_WIDTH;
  // 鼠标相对于聊天区起始位置的偏移量
  const x = e.clientX - shellRect.left - sidebarW;
  // 限制：聊天区最小 280px，编辑器最小 550px
  const chatPx = Math.min(available - EDITOR_MIN_PX, Math.max(CHAT_MIN_PX, x));
  chatPanelPx.value = chatPx;
}

function onSplitDragEnd() {
  if (!isDraggingSplit.value) return;
  isDraggingSplit.value = false;
  document.body.classList.remove("fastclaw-chat-split-dragging");
  document.removeEventListener("mousemove", onSplitDragMove);
  document.removeEventListener("mouseup", onSplitDragEnd);
}

// ─── 截图（Tauri 端走 ScreenshotOverlay；Web 端占位）───
const captureResult = ref<CaptureMetadata | null>(null);
const inlineScreenshotEvents = shallowRef<{ confirm: (dataUrl: string) => Promise<void>; cancel: () => Promise<void> } | null>(null);
const windowListVisible = ref(false);
const screenshotGroupRef = ref<HTMLElement>();
const popoverStyle = ref<Record<string, string>>({});

// ─── 切换模型（多模型 children）───
// 优先在 jcr-action 按钮旁弹出 ModelSelector 下拉；找不到锚点时回退 el-dialog
const switchModelPickerVisible = ref(false);
const switchModelAnchorActive = ref(false);
const switchModelAnchorMessageId = ref<string | null>(null);
const switchModelAnchorStyle = ref<Record<string, string>>({});
const switchModelTarget = ref<ReturnType<typeof captureAnswerTarget> | null>(
  null,
);
const switchModelSelectorRef = ref<InstanceType<typeof ModelSelector> | null>(
  null,
);
const switchModelAnchorSelectorRef = ref<InstanceType<typeof ModelSelector> | null>(
  null,
);
const switchModelSelectedId = ref<string | undefined>(undefined);
/** 锚点下拉正在打开中，忽略卸载/重挂载触发的 visible-change(false) */
let switchModelAnchorOpening = false;
const switchModelPlacement = ref<'top-start' | 'bottom-start'>('bottom-start');
const switchModelFallbackPlacements = ref<string[]>([
  'bottom-start',
  'top-start',
  'bottom',
  'top',
]);
/** 下拉与按钮保持间距，避免遮挡 jcr-action；方向已按空间算好，关闭 flip */
const switchModelPopperOptions = {
  modifiers: [
    { name: 'offset', options: { offset: [0, 0] } },
    { name: 'flip', enabled: false },
    {
      name: 'preventOverflow',
      options: { padding: 8, boundary: 'viewport', altAxis: true },
    },
  ],
};

// ─── 编辑消息弹窗 ───
const messageEditor = createMessageEditorBinding(chatContextBinding, error => {
  ElMessage.error(error instanceof Error ? error.message : '编辑失败');
});
const messageEditorPanel = messageEditor.panel;
const inlineQuestion = createInlineQuestionBinding(chatContextBinding,
  context => context !== initialChatContext || inConversation.value || fastPlanBusy.value
    || context.owner.state.pendingAutoSend.value || context.owner.state.sendBtnLock.value,
  resendUserQuestion);
const inlineQuestionPanel = inlineQuestion.panel;

// ─── Tauri invoke ───
type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
let _invoke: InvokeFn | null = null;
async function getInvoke(): Promise<InvokeFn | null> {
  if (_invoke) return _invoke;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    _invoke = invoke as InvokeFn;
    return _invoke;
  } catch {
    return null;
  }
}
function isTauriEnv(): boolean {
  return !!(window as any).__TAURI_INTERNALS__ || !!(window as any).__TAURI__;
}

// ─── 消息分组（按 group_id 聚合多模型回答）───
interface MessageGroup {
  groupId: string | null;
  messages: Message[];
  startIndex: number;
}
const groupedMessages = computed<MessageGroup[]>(() => {
  const groups: MessageGroup[] = [];
  const msgs = chat.messages;
  for (let i = 0; i < msgs.length; i++) {
    const msg = msgs[i];
    const gid = msg.group_id || null;
    if (gid) {
      const last = groups[groups.length - 1];
      if (last && last.groupId === gid) {
        last.messages.push(msg);
        continue;
      }
    }
    groups.push({ groupId: gid, messages: [msg], startIndex: i });
  }
  return groups;
});

// 上下文清除分隔线：当某组消息全部被标记 context_cleared，且其后没有消息
// 或后一组未被清除时，在该组下方展示一条"上下文已清除"分隔线。
function isGroupContextCleared(group: MessageGroup): boolean {
  return (
    group.messages.length > 0 &&
    group.messages.every((m) => !!m.context_cleared)
  );
}
function showContextDivider(gIdx: number): boolean {
  const groups = groupedMessages.value;
  const g = groups[gIdx];
  if (!g || !isGroupContextCleared(g)) return false;
  const next = groups[gIdx + 1];
  return !next || !isGroupContextCleared(next);
}

const lastAiMsgId = computed(() => {
  for (let i = chat.messages.length - 1; i >= 0; i--) {
    if (chat.messages[i].role === "assistant") return chat.messages[i].id;
  }
  return null;
});

// 将 QuestionNav 的提问序号映射到 groupedMessages 的组索引
const questionGroupIndices = computed(() => {
  const indices: number[] = [];
  const seen = new Set<string | null>();
  groupedMessages.value.forEach((group, gi) => {
    if (group.messages.some((m) => m.role === "user")) {
      // 与 QuestionNav 保持一致的去重逻辑
      const gid = group.groupId;
      if (gid && seen.has(gid)) return;
      if (gid) seen.add(gid);
      indices.push(gi);
    }
  });
  return indices;
});

const highlightedIndex = tabState.state.highlightedIndex;
const { isLoading: inConversation } = storeToRefs(chat);

// ─── chat-plan-packages 普通会话边界（P2）───
// 默认启用；出现集成回归时可在控制台设置 localStorage['fastclaw.chatPlanPackages.enabled']='false' 后刷新回退。
const useChatPlanPackages = ref(isChatPlanPackagesEnabled());
const fastPlanComposerTargetId = `fastplan-composer-${viewIdentity.instanceId.replace(/[^a-zA-Z0-9_-]/g, '-')}`;
const fastPlanEntries = tabState.state.fastPlanEntries;
const activeFastPlanEntryId = tabState.state.activeFastPlanEntryId;
/** 按会话记录规划 busy，避免切到其他历史会话时按钮状态串台 */
const fastPlanBusyByConv = tabState.state.fastPlanBusyByConv;
/** entryId → conversationId，切走后仍能把后台 busy 事件记到原会话 */
const fastPlanEntryOwners = tabState.fastPlanEntryOwners;
const fastPlanBusy = computed(() => {
  const cid = chat.currentConversationId;
  const activity = executionActivity.activityByPath(myPath);
  if (fastPlanEntries.value.some(entry => (entry.data as FastPlanTimelineEntryData).commandOwner === 'host')) {
    return !!(activity?.kind === 'fastplan' && activity.protected);
  }
  return !!(activity?.kind === 'fastplan' && activity.protected)
    || !!(cid && fastPlanBusyByConv.value.get(cid));
});
function setFastPlanBusyFor(convId: string | null | undefined, busy: boolean) {
  if (!convId) return;
  if (Boolean(fastPlanBusyByConv.value.get(convId)) === busy) return;
  const next = new Map(fastPlanBusyByConv.value);
  if (busy) next.set(convId, true);
  else next.delete(convId);
  fastPlanBusyByConv.value = next;
}
function rememberFastPlanEntryOwner(entryId: string, convId: string | null | undefined) {
  if (entryId && convId) fastPlanEntryOwners.set(entryId, convId);
}
function resolveFastPlanBusyTarget(entryId: string): string | null {
  if (fastPlanEntries.value.some((entry) => entry.id === entryId)) {
    return chat.currentConversationId;
  }
  return fastPlanEntryOwners.get(entryId) ?? null;
}
/** 落盘目标：优先 entry 所属会话，避免切会话后的异步事件写到当前会话 */
function resolveFastPlanPersistTarget(entryId?: string): string | null {
  if (entryId) {
    const owner = fastPlanEntryOwners.get(entryId);
    if (owner) return owner;
  }
  return chat.currentConversationId;
}
/** 规划执行是否已到终态（失败/取消/全部节点结束），用于纠偏 node_failed 后仍 busy=true 的竞态 */
function isFastPlanTerminalState(payload: Record<string, any> | null | undefined): boolean {
  const state = payload?.state;
  if (!state || typeof state !== 'object') return false;
  if (payload?.treeCompleted || state.treeCompleted) return true;
  const status = String(state.executionStatus || '').toLowerCase();
  if (status === 'failed' || status === 'cancelled' || status === 'completed') return true;
  if (state.planningError) return true;
  if (state.isPlanning || state.isPreviewing || state.isPaused) return false;
  const nodes = Object.values(state.executionState || {}) as Array<{ status?: string }>;
  if (!nodes.length) return false;
  const hasRunning = nodes.some((n) => String(n?.status || '').toLowerCase() === 'running');
  const hasFailed = nodes.some((n) => String(n?.status || '').toLowerCase() === 'failed');
  // 有失败且无 running：视为终态（其余 pending 视为已中止；后端有时晚发或不发 tree_completed）
  return hasFailed && !hasRunning;
}

function resolveFastPlanActivityPhase(state: any): ExecutionPhase {
  const connectionStatus = String(state?.connectionStatus || '').toLowerCase();
  if (connectionStatus === 'reconnecting' || connectionStatus === 'connecting') return 'reconnecting';
  if (state?.humanRequest || state?.humanRequestQueue?.length || state?.humanInterventionRequired) return 'awaiting_human';
  if (state?.isPreviewing) return 'awaiting_plan_confirmation';
  if (state?.isPaused && state?.executionMode === 'step') return 'awaiting_step_confirmation';
  if (state?.isPaused) return 'paused';
  if (state?.plannedTree && !state?.treeCompleted) return 'executing';
  if (state?.isPlanning) return 'planning';
  const status = String(state?.executionStatus || '').toLowerCase();
  if (status === 'completed') return 'completed';
  if (status === 'failed') return 'failed';
  if (status === 'cancelled') return 'cancelled';
  return 'idle';
}

function updateFastPlanExecutionActivity(entryId: string, state?: any, forceBusy?: boolean) {
  const conversationId = resolveFastPlanBusyTarget(entryId) || chat.currentConversationId;
  if (!conversationId) return;
  if (fastPlanSessionManager.hasSubmission(conversationId, entryId)) return;
  const entry = fastPlanEntries.value.find(item => item.id === entryId);
  const data = entry?.data as FastPlanTimelineEntryData | undefined;
  if (data?.sessionId) {
    fastPlanSessionManager.bindSession(data.sessionId, {
      conversationId,
      tabPath: myPath,
      entryId,
      executionMode: (state?.executionMode || data.executionMode || fastPlanExecutionMode.value) as FastPlanExecutionMode,
      modelId: String((data.model as any)?.id || modelId.value || '') || null,
    });
    // A generic busy=true must not erase the manager's phase/connection. Keep
    // explicit renderer state/terminal handling for legacy compatibility.
    if (!state && fastPlanSessionManager.get(data.sessionId)) return;
  }
  const phase = state ? resolveFastPlanActivityPhase(state) : (forceBusy ? 'executing' : 'idle');
  const protectedState = forceBusy ?? !['idle', 'completed', 'failed', 'cancelled'].includes(phase);
  if (!protectedState) {
    executionActivity.remove(conversationId, 'fastplan');
    return;
  }
  executionActivity.update(conversationId, myPath, 'fastplan', {
    phase,
    protected: true,
    sessionId: data?.sessionId,
    entryId,
    executionMode: (state?.executionMode || data?.executionMode || fastPlanExecutionMode.value) as FastPlanExecutionMode,
    connectionStatus: state?.connectionStatus,
  });
}

/** busy=true 记下；busy=false 忽略卸载/历史回显误报；终态条目拒绝被再次点亮 */
function applyFastPlanBusyChange(entryId: string, busy: boolean) {
  const target = resolveFastPlanBusyTarget(entryId);
  if (!target) return;
  rememberFastPlanEntryOwner(entryId, target);
  const entry = fastPlanEntries.value.find((item) => item.id === entryId);
  if (busy) {
    // node_failed → error 已清 busy 后，紧跟的 state-change 仍可能 busy=true（plannedTree && !treeCompleted）
    if (entry && (entry.status === 'failed' || entry.status === 'complete')) return;
    setFastPlanBusyFor(target, true);
    return;
  }
  if (conversationRestoring.value) return;
  // 切走后条目已清空：卸载事件带来的 busy=false 不能清掉原会话 busy
  if (!entry) return;
  // 仍在执行的条目：历史回显/重挂载也可能发 busy=false，保持停止按钮
  if (entry.status === 'active') return;
  setFastPlanBusyFor(target, false);
}
type FastPlanExecutionMode = 'auto' | 'manual' | 'step';
const fastPlanExecutionMode = tabState.state.fastPlanExecutionMode;
let applyingRemoteExecutionMode = false;
const conversationRestoring = ref(false);
const loadingFastPlanEntries = ref(new Set<string>());
const historyContentLoading = computed(() => {
  // 会话恢复期间保留已有内容，不再用整个内容区遮罩。
  // FastPlan 条目有自己的 loading 状态；ChatConversation 的 loading context
  // 会抑制嵌套遮罩，这里不要再叠加顶层白屏。
  return false;
});

function deferCompletedFastPlanHistory(entries: ChatEmbeddedTimelineEntry[]) {
  return entries.map((entry, index) => ({
    ...entry,
    data: {
      ...(entry.data as FastPlanTimelineEntryData),
      // Keep every running entry and the newest completed entry warm. Older
      // completed plans remain as summaries until the user expands one.
      deferLoad: index !== entries.length - 1 && entry.status !== 'active',
    },
  }));
}
const taskPlanSwitching = ref(false);
const fastPlanWelcomeConnectionStatus = computed(() => fastPlanServiceStatus.presentationStatus);
const fastPlanWelcomeViewActive = ref(false);
const fastPlanWelcomeProbeReady = ref(false);
let fastPlanWelcomeProbeFrame: number | null = null;
let fastPlanWelcomeProbeTimer: ReturnType<typeof setTimeout> | null = null;
let fastPlanWelcomeProbeGeneration = 0;
let fastPlanWriterConversationId: string | null = null;
const fastPlanPluginOptions = reactive<FastPlanTimelinePluginOptions>({
  access: fastClawFastPlanAccess,
  async beforeSend(selection, prompt) {
      const selectedId = selection && typeof selection === 'object' ? (selection as { id?: string }).id : undefined;
      if (viewDisposed) return false;
      const current = tabState.capture();
      const ready = !!selectedId && await ensureModelProviderReady(selectedId, prompt);
      return ready && current() && !viewDisposed;
  },
  executionMode: fastPlanExecutionMode.value,
  confirmationPolicy: 'strict',
  retainConnectionOnSessionSwitch: false,
  composerMode: 'host',
  composerTarget: `#${fastPlanComposerTargetId}`,
  composerEntryId: retainedComposerEntry(tabState),
  density: 'auto',
  compactBreakpoint: 700,
  sessionManager: fastPlanSessionManager,
  theme: workspaceThemeMode.value,
  themeTokens: FASTCLAW_CHAT_THEME_TOKENS,
});
const fastPlanPlugin = createFastPlanTimelinePlugin(fastPlanPluginOptions);
watch(activeFastPlanEntryId, () => {
  fastPlanPluginOptions.composerEntryId = retainedComposerEntry(tabState);
}, { flush: 'sync' });
watch(
  () => `${chat.currentConversationId || ''}|${fastPlanEntries.value.map((entry) => {
    const data = entry.data as FastPlanTimelineEntryData;
    return `${entry.id}:${data.sessionId || ''}:${data.connectionMode || ''}:${data.executionMode || ''}:${entry.status || ''}`;
  }).join('|')}`,
  () => {
    const conversationId = chat.currentConversationId;
    if (!conversationId) return;
    for (const entry of fastPlanEntries.value) {
      const data = entry.data as FastPlanTimelineEntryData;
      if (!data.sessionId || (data.connectionMode !== 'live' && entry.status !== 'active')) continue;
      rememberFastPlanEntryOwner(entry.id, conversationId);
      fastPlanSessionManager.bindSession(data.sessionId, {
        conversationId,
        tabPath: myPath,
        entryId: entry.id,
        executionMode: (data.executionMode || fastPlanExecutionMode.value) as FastPlanExecutionMode,
        modelId: String((data.model as any)?.id || modelId.value || '') || null,
      });
    }
  },
  { immediate: true },
);
watch(fastPlanExecutionMode, (mode, prev) => {
  fastPlanPluginOptions.executionMode = mode;
  const entryId = activeFastPlanEntryId.value;
  if (entryId) {
    fastPlanEntries.value = fastPlanEntries.value.map((entry) => entry.id === entryId ? {
      ...entry,
      data: { ...(entry.data as FastPlanTimelineEntryData), executionMode: mode },
    } : entry);
  }
  if (prev === undefined || conversationRestoring.value || applyingRemoteExecutionMode) return;
  void persistConversationExecutionMode(mode);
}, { immediate: true });
watch(workspaceThemeMode, (theme) => {
  fastPlanPluginOptions.theme = theme;
}, { immediate: true });
watch(() => appConfigStore.serviceUrl('fastplan'), (serviceUrl) => {
  const config = resolveFastPlanServiceConfig(serviceUrl || SERVICE_URL_DEFAULTS.fastplan);
  Object.assign(fastPlanPluginOptions, config);
}, { immediate: true });
function cancelFastPlanWelcomeProbe() {
  fastPlanWelcomeProbeGeneration += 1;
  if (fastPlanWelcomeProbeFrame !== null) cancelAnimationFrame(fastPlanWelcomeProbeFrame);
  if (fastPlanWelcomeProbeTimer !== null) clearTimeout(fastPlanWelcomeProbeTimer);
  fastPlanWelcomeProbeFrame = null;
  fastPlanWelcomeProbeTimer = null;
  fastPlanWelcomeProbeReady.value = false;
}

/** 欢迎页连接仅用于状态展示；让首帧先完成，实际规划执行链保持不变。 */
function scheduleFastPlanWelcomeProbe() {
  cancelFastPlanWelcomeProbe();
  const generation = fastPlanWelcomeProbeGeneration;
  fastPlanWelcomeProbeFrame = requestAnimationFrame(() => {
    fastPlanWelcomeProbeFrame = null;
    fastPlanWelcomeProbeTimer = setTimeout(() => {
      fastPlanWelcomeProbeTimer = null;
      if (fastPlanWelcomeViewActive.value && generation === fastPlanWelcomeProbeGeneration) {
        fastPlanWelcomeProbeReady.value = true;
      }
    }, 0);
  });
}

interface BeforeTabCloseDetail {
  path: string;
  cancelExecution: boolean;
  waitUntil: (task: Promise<unknown>) => void;
}

async function prepareFastPlanForTabClose(cancelExecution = false) {
  const conversationId = chat.currentConversationId;
  if (cancelExecution && conversationId) {
    if (chat.isConversationStreaming(conversationId)) {
      await chat.stopStream(conversationId);
    }
    if (executionActivity.activityByPath(myPath)?.kind === 'fastplan' && activeFastPlanEntryId.value) {
      await fastPlanSessionManager.stopConversation(conversationId, activeFastPlanEntryId.value)
        .catch(error => console.warn('[FastPlan] stop while closing failed', error));
    }
    executionActivity.removeByPath(myPath);
    planActivity.remove(myPath);
    appStore.setTabUnfinished(myPath, 'fastplan', false);
    appStore.setTabUnfinished(myPath, 'chat', false);
  }
  // The target is owned by ChatView, so detach Teleport content before the
  // route and its target element are removed by KeepAlive.
  fastPlanPluginOptions.composerTarget = null;
  fastPlanPluginOptions.composerEntryId = null;
  await nextTick();
}

function onBeforeTabClose(event: Event) {
  const detail = (event as CustomEvent<BeforeTabCloseDetail>).detail;
  if (!detail || detail.path !== myPath) return;
  detail.waitUntil(prepareFastPlanForTabClose(detail.cancelExecution));
}

window.addEventListener('fastclaw:before-tab-close', onBeforeTabClose);
watch(
  [
    useChatPlanPackages,
    () => dialogueCharacter.value.taskPlan,
    () => fastPlanEntries.value.length,
    () => appConfigStore.serviceUrl('fastplan'),
    fastPlanWelcomeViewActive,
    fastPlanWelcomeProbeReady,
  ],
  ([packagesEnabled, taskPlanEnabled, entryCount, _serviceUrl, viewActive, probeReady]) => {
    if (!viewActive || !probeReady || !packagesEnabled || !taskPlanEnabled || Number(entryCount) > 0) {
      return;
    }
    // 服务状态由所有 Chat Tab 共享。空白欢迎页不再额外创建 `/ws`，
    // 真正发起任务时仍由对应 session 建立自己的 WebSocket。
    void fastPlanServiceStatus.ensureReady();
  },
  { immediate: true },
);
watch(
  [fastPlanBusy, () => chat.currentConversationId, activeFastPlanEntryId],
  ([busy, conversationId, entryId]) => {
    planActivity.update(myPath, {
      busy: Boolean(busy),
      conversationId: conversationId || undefined,
      entryId: entryId || undefined,
    });
    appStore.setTabUnfinished(myPath, 'fastplan', Boolean(busy));
    if (conversationId && entryId && busy) updateFastPlanExecutionActivity(entryId, undefined, true);
  },
  { immediate: true },
);
onUnmounted(() => {
  window.removeEventListener('fastclaw:before-tab-close', onBeforeTabClose);
  if (!fastPlanBusy.value) planActivity.remove(myPath);
  // Writer lease belongs to tabState; KeepAlive eviction must not release it.
});
const chatWorkspacePlugins = computed(() => ({
  timelineRenderers: {
    'fastclaw-context-divider': FastClawContextDivider,
    ...fastPlanPlugin.timelineRenderers,
  },
  partRenderers: {
    'fastclaw-web-search': FastClawCustomPart,
    'fastclaw-skill-used': FastClawCustomPart,
    'fastclaw-plan': FastClawCustomPart,
    'fastclaw-echarts': FastClawCustomPart,
    'fastclaw-vendor-config': VendorConfigRenderer,
  },
}));
function workspaceModelName(id: string | null): string | undefined {
  if (!id) return undefined;
  for (const group of modelTree.value) {
    const model = group.children.find((item) => item.id === id);
    if (model) return model.name;
  }
  return undefined;
}
function workspaceModelIcon(id?: string): string {
  if (!id) return selectedModelIcon.value;
  for (const group of modelTree.value) {
    if (group.children.some((item) => item.id === id)) return getProviderIcon(group.providerCode, group.sourceCategory);
  }
  return selectedModelIcon.value;
}
const { providerCompletionTimeline, chatPlanTimeline } = useChatTimelinePresentation({
  messages: () => chat.messages,
  fastPlanEntries: () => fastPlanEntries.value,
  taskPlanEnabled: () => dialogueCharacter.value.taskPlan,
  providerPanel: () => vendorConfigPanel.value,
  modelName: workspaceModelName,
  contextDividerLabel: () => t('chat.contextClearedDivider'),
});
const workspaceLabels = computed(() => ({
  scrollToBottom: t('chat.scrollToBottom'),
  questionDirectory: t('chat.questionDirectory'),
  openQuestionDirectory: t('chat.openQuestionDirectory'),
  closeQuestionDirectory: t('chat.closeQuestionDirectory'),
  copy: t('chat.messageActions.copy'),
  copied: t('chat.messageActions.copied'),
  edit: t('chat.messageActions.edit'),
  resend: t('chat.messageActions.resend'),
  regenerate: t('chat.messageActions.regenerate'),
  quote: t('chat.messageActions.quote'),
  tts: t('chat.messageActions.tts'),
  translate: t('chat.messageActions.translate'),
  favorite: t('chat.messageActions.favorite'),
  'edit-as-doc': t('chat.messageActions.editAsDoc'),
  'switch-model': t('chat.messageActions.switchModel'),
  'trace-source': t('chat.messageActions.traceSource'),
  export: t('chat.messageActions.export'),
  feedback: t('chat.messageActions.feedback'),
  like: t('chat.messageActions.like'),
  dislike: t('chat.messageActions.dislike'),
  branch: t('chat.messageActions.branch'),
  delete: t('chat.messageActions.delete'),
  confirm: t('chat.messageActions.confirm'),
  cancel: t('chat.messageActions.cancel'),
  more: t('chat.messageActions.more'),
  attachmentReference: t('chat.messageActions.quote'),
}));

let chatContentPaintGeneration = 0;
let chatPerfActivationReady = false;
let postActivationRefreshPending = false;
let cancelPostActivationIdle: (() => void) | undefined;
let cancelContentPaintTick: (() => void) | undefined;
let cancelContentPaintFrame: (() => void) | undefined;
let postPaintUiWorkPending = false;
let restoreLegacyScrollOnNextPaint = false;

function cancelPostActivationRefresh() {
  cancelPostActivationIdle?.();
  cancelPostActivationIdle = undefined;
}

function schedulePostActivationRefresh() {
  if (!postActivationRefreshPending || !viewEffects.isActive()) return;
  postActivationRefreshPending = false;
  cancelPostActivationRefresh();
  cancelPostActivationIdle = viewEffects.idle(() => {
    cancelPostActivationIdle = undefined;
    if (isChatModelCatalogStale()) void loadModels();
  });
}

function cancelChatContentPaintReport() {
  chatContentPaintGeneration += 1;
  cancelContentPaintTick?.();
  cancelContentPaintFrame?.();
}

function restoreLegacyScrollBeforePaint() {
  const wrap = getScrollWrap();
  if (restoreLegacyScrollOnNextPaint && wrap) {
    markWorkspaceProgrammatic();
    restoreReadingPosition(wrap, savedScrollTop.value, tabState.state.readingAnchor.value);
    workspaceProgrammaticScroll = false;
  }
  restoreLegacyScrollOnNextPaint = false;
}

/** 首屏已经可见后再恢复非关键 UI 行为，避免 observer/layout/reconnect 争抢切换帧。 */
function runPostPaintUiWork() {
  if (!postPaintUiWorkPending || !activeUiListenersAttached || route.path !== myPath) return;
  postPaintUiWorkPending = false;
  workspacePostPaintReady = true;

  attachNavResizeObserver();
  attachWorkspaceScrollListener();
  bindMediumZoomWheelZoom();
  scheduleFastPlanWelcomeProbe();

  if (shouldAutoScroll.value && (inConversation.value || fastPlanBusy.value)) {
    engageFollowStream();
  }
  const activeExecution = executionActivity.activityByPath(myPath);
  if (
    activeExecution?.kind === 'fastplan'
    && activeExecution.connectionStatus !== 'connected'
    && activeFastPlanEntryId.value
  ) {
    updateFastPlanCommand({ type: 'ensure-live' });
  }
}

/** Report after Vue has committed and Chromium has had a paint opportunity. */
function scheduleChatContentPaintReport() {
  if (!chatPerfActivationReady || route.path !== myPath || conversationRestoring.value) return;
  cancelChatContentPaintReport();
  const generation = chatContentPaintGeneration;
  cancelContentPaintTick = viewEffects.tick(() => {
    cancelContentPaintFrame = viewEffects.frame(() => {
      if (
        generation !== chatContentPaintGeneration
        || !chatPerfActivationReady
        || route.path !== myPath
        || conversationRestoring.value
      ) return;
      restoreLegacyScrollBeforePaint();
      cancelContentPaintFrame = viewEffects.frame(() => {
        if (
          generation !== chatContentPaintGeneration
          || !chatPerfActivationReady
          || route.path !== myPath
          || conversationRestoring.value
        ) return;
        const root = chatMainRef.value;
        const sessionStats = fastPlanSessionManager.stats();
        chatSwitchPerf.markFirstContent(myPath, {
          messageCount: chat.messages.length,
          planEntryCount: fastPlanEntries.value.length,
          fastPlanSessions: sessionStats.total,
          activeFastPlanSessions: sessionStats.active,
          // First-content time is captured before these diagnostic DOM queries,
          // so instrumentation cannot inflate the user-visible switch metric.
          collectDomResources: () => ({
             timelineDomNodes: root?.querySelectorAll('.jcr-timeline > *, .jcr-message').length ?? 0,
             markdownInstances: root?.querySelectorAll('.jcr-markdown').length ?? 0,
             richMarkdownInstances: root?.querySelectorAll('.md-editor-preview-wrapper').length ?? 0,
             deferredFastPlanMessages: root?.querySelectorAll('.fastplan-conversation__process-summary').length ?? 0,
           }),
         });
        runPostPaintUiWork();
        schedulePostActivationRefresh();
      });
    });
  });
}

watch(
  [conversationRestoring, () => chatPlanTimeline.value.length, () => route.path],
  scheduleChatContentPaintReport,
  { flush: 'post' },
);
const workspaceComposerActions = computed<ChatComposerAction[]>(() => {
  // 与旧输入区的 btn-left v-if/v-else-if 保持一致：技能模式只展示各自的标签和参数。
  return [
    { id: 'skill', label: t('chat.SelectSkillMCPTool'), position: 'start', active: slash.showMenu.value },
    { id: 'image', label: isVisionModel.value ? t('chat.imageRecognition') : t('chat.imageOcrHint'), position: 'start',
      kind: 'file', accept: '.jpg,.png,.jpeg,.webp,.bmp', multiple: true },
    { id: 'document', label: t('chat.documentOCR'), position: 'start',
      kind: 'file', accept: '.pdf,.doc,.docx,.txt,.md,.xls,.xlsx,.csv', multiple: true },
    { id: 'screenshot', label: t('chat.screenshot'), position: 'start' },
    { id: 'thinking', label: dialogueCharacter.value.think ? t('chat.deepThinkEnabled') : t('chat.deepThinkDisabled'),
      position: 'start', active: dialogueCharacter.value.think, disabled: !isDeepThinkModel.value, visible: !isAgentConfigLocked.value },
    { id: 'planning', label: dialogueCharacter.value.taskPlan ? t('chat.taskPlanEnabled') : t('chat.taskPlanDisabled'),
      position: 'start', active: dialogueCharacter.value.taskPlan, disabled: taskPlanSwitching.value, visible: !isAgentConfigLocked.value },
    { id: 'web-search', label: t('chat.webSearch'), position: 'start', visible: true },
    { id: 'mcp', label: t('chat.mcpTool'), position: 'start', visible: !isAgentConfigLocked.value },
    { id: 'clear-context', label: t('shortcuts.clearContext'), position: 'start', visible: chat.messages.length > 0 },
  ];
});
const workspaceComposerAttachments = computed<ChatComposerAttachment[]>(() => fileList.value.map((file) => ({
  id: file.id,
  kind: file.type === 'image' || file.type === 'screenshot' ? 'image' : 'file',
  name: file.name,
  previewUrl: file.preview_url,
  mimeType: fileRefs.get(file.id)?.type,
  size: file.size,
  status: file.status === 'uploaded' ? 'complete' : file.status === 'uploading' ? 'uploading' : file.status === 'error' ? 'error' : 'idle',
  error: file.error_message,
  metadata: { hostAttachmentId: file.id, hostType: file.type },
})));

function findHostMessage(id?: string): Message | undefined {
  return id ? chat.messages.find((message) => message.id === id) : undefined;
}
const composerContext = createComposerContextBindings(chatContextBinding, initialChatContext);
const composerSelectorProps = composerContext.selectors;
const composerHintProps = composerContext.hints;
const composerHintEvents = composerContext.hintEvents;
const composerPaletteEvents = computed(() => {
  const lease = chatContextBinding.captureUi();
  return {
    select: lease?.guard(slash.selectCommand),
    'update:searchQuery': lease?.guard((value: string) => { slash.searchQuery.value = value; }),
    'update:activeTab': lease?.guard((value: 'all' | 'skill' | 'mcp') => { slash.activeTab.value = value; }),
    'update:selectedIndex': lease?.guard((value: number) => { slash.selectedIndex.value = value; }),
    close: lease?.guard(slash.closePopup),
  };
});
const composerActionEvents = computed(() => {
  const lease = chatContextBinding.captureUi();
  const events = {
    'update:windowListVisible': lease?.guard((value: boolean) => { windowListVisible.value = value; }),
    toggleWindowList: lease?.guard(toggleWindowList),
    hideSelf: lease?.guard(screenshotHideSelf),
  };
  return (trigger: (action: ChatComposerAction, event: MouseEvent) => void) => ({
    ...events, trigger: lease?.guard((action: ChatComposerAction, event: MouseEvent) => {
      if (action.disabled) return;
      // Own the file-dialog lifetime; the public renderer's reused input has no source-context token.
      if (action.id === 'image') imageClick();
      else if (action.id === 'document') ocrClick();
      else trigger(action, event);
    }),
  });
});
const composerSelectorEvents = computed(() => {
  const lease = chatContextBinding.captureUi();
  return {
    ...composerContext.selectorUpdates.value,
    visibleChange: lease?.guard(onSelectVisibleChange),
    modelChange: lease?.guard(onChatModelChange),
    roleChange: lease?.guard(onRoleChange),
  };
});
const workspaceAbort = createWorkspaceAbortBinding(chatContextBinding, {
  cancelPending: owner => deferredChatSubmissionService.cancel(owner),
  stopPlan: (conversationId, entryId) => fastPlanSessionManager.stopConversation(conversationId, entryId),
  reportError: error => ElMessage.error(error instanceof Error ? error.message : String(error)),
});
const workspaceContextEvents = computed(() => {
  const lease = chatContextBinding.captureUi();
  const draft = lease?.model(lease.context.owner.state.messageStr);
  return {
    'update:modelValue': (value: string) => { if (draft) draft.value = value; },
    submit: lease?.guard(onWorkspaceSubmit),
    abort: workspaceAbort.value,
    'composer-files': lease?.guard(onWorkspaceFiles),
    'workspace-action': lease?.guard(onWorkspaceAction),
    'message-action': lease?.guard(onWorkspaceMessageAction),
  };
});
function rendererMessageText(message: RendererChatMessage): string {
  return message.parts.filter((part) => part.type === 'text').map((part) => part.text).join('\n\n');
}
async function onWorkspaceSubmit(event: ChatComposerSubmitEvent) {
  if (!event.text.trim() && !inConversation.value) return;
  messageStr.value = event.text;
  shouldAutoScroll.value = true;
  // 发送后立刻贴底跟随流式输出，避免视口停在问题开头
  if (useChatPlanPackages.value) engageFollowStream();
  await sendBtnFn();
}
function onWorkspaceFiles(event: ChatComposerFileSelectionEvent) {
  if (event.action === 'image') {
    for (const file of event.files) addImageAttachment(file);
    return;
  }
  if (event.action === 'document') {
    for (const file of event.files) void uploadSingleDoc(file);
  }
}
function onWorkspaceAction(event: ChatWorkspaceActionEvent) {
  if (event.scope === 'timeline' && event.action === 'custom') {
    void onFastPlanTimelineAction((event.payload || {}) as Record<string, any>);
    return;
  }
  if (event.scope !== 'composer') return;
  const action = event.actionId;
  if (action === 'skill') slash.showMenu.value ? slash.closePopup() : slash.openPopup();
  else if (action === 'screenshot') handleScreenshot();
  else if (action === 'thinking') thinkClick();
  else if (action === 'planning') void taskPlanClick();
  else if (action === 'clear-context') deleteAllText();
  else if (event.action === 'attachment-remove') {
    const id = (event.payload as any)?.attachment?.metadata?.hostAttachmentId;
    const index = fileList.value.findIndex((file) => file.id === id);
    if (index >= 0) removeFile(index);
  }
}
function selectedFastPlanModel() {
  for (const group of modelTree.value) {
    const selected = group.children.find((model) => model.id === modelId.value);
    if (!selected) continue;
    return {
      id: selected.id,
      model: selected.rawModelId || selected.modelId || selected.id,
    };
  }
  return modelId.value || undefined;
}
function updateFastPlanCommand(command: { type: 'stop' | 'pause' | 'resume' | 'confirm' | 'retry-node' | 'skip-node' | 'refresh' | 'ensure-live'; nodeId?: string }) {
  const entryId = activeFastPlanEntryId.value;
  if (!entryId) return;
  if (command.type === 'stop') {
    void fastPlanSessionManager.stopConversation(chat.currentConversationId || '', entryId)
      .catch(error => ElMessage.error(error instanceof Error ? error.message : String(error)));
    return;
  }
  // Submission owns connection/load while pending; activation must not replace its command.
  if (fastPlanSessionManager.hasSubmission(chat.currentConversationId || '', entryId)) return;
  const target = fastPlanSessionManager.controlTarget(chat.currentConversationId || '', entryId);
  if (target && command.type !== 'refresh') {
    void fastPlanSessionManager.executeControl(target, { ...command, type: command.type })
      .catch(error => ElMessage.error(error instanceof Error ? error.message : String(error)));
    return;
  }
  // Compatibility for non-managed history/legacy entries; sending migrates in R2-B.
  fastPlanEntries.value = fastPlanEntries.value.map((entry) => entry.id === entryId ? {
    ...entry,
    data: {
      ...(entry.data as FastPlanTimelineEntryData),
      command: { id: crypto.randomUUID(), ...command },
    },
  } : entry);
}
type FastPlanPendingAttachment = Record<string, unknown> & { file?: File };



async function enqueueFastPlanPrompt(entryId: string, prompt: string, attachments: FastPlanPendingAttachment[]) {
  const conversationId = chat.currentConversationId;
  if (!conversationId) throw new Error('请先准备规划会话');
  return planningSubmissionService.submit({
    owner: tabState, store: chat as unknown as PlanningConversationStore,
    tabPath: myPath, conversationId, entryId, prompt, attachments,
    model: selectedFastPlanModel(), executionMode: fastPlanExecutionMode.value,
    thinking: dialogueCharacter.value.think,
    apiBase: fastPlanPluginOptions.apiBase, wsBase: fastPlanPluginOptions.wsBase,
    uploadBase: fastPlanPluginOptions.uploadBase,
  });
}

function syncDesktopPlanEventToApp(event: Record<string, unknown>) {
  const sid = chat.currentConversationId;
  if (!sid || !event || typeof event !== 'object') return Promise.resolve();
  return fastPlanSessionManager.emitDesktopEvent(sid, event);
}

async function onFastPlanTimelineAction(payload: Record<string, any>) {
  const entryId = String(payload.entryId || '');
  if (!entryId) return;
  if (fastPlanSessionManager.hasSubmission(chat.currentConversationId || '', entryId)
    && ['busy-change', 'state-change', 'execution-completed', 'error'].includes(String(payload.type))) return;
  if (payload.type === 'loading-change') {
    if (!fastPlanEntries.value.some(entry => entry.id === entryId)) return;
    if (payload.loading) loadingFastPlanEntries.value.add(entryId);
    else loadingFastPlanEntries.value.delete(entryId);
    return;
  }
  if (payload.type === 'session-created') {
    const sessionId = String(payload.sessionId || payload.session?.id || payload.session?.session_id || '');
    if (sessionId) {
      // 条目已不在当前时间线（已切走）时不再落盘，避免污染新会话
      if (!fastPlanEntries.value.some((entry) => entry.id === entryId)) return;
      rememberFastPlanEntryOwner(entryId, chat.currentConversationId);
      fastPlanEntries.value = fastPlanEntries.value.map((entry) => entry.id === entryId ? {
        ...entry,
        data: { ...(entry.data as FastPlanTimelineEntryData), sessionId, command: undefined },
      } : entry);
      await onFastPlanSave(sessionId, String(payload.prompt || ''), resolveFastPlanPersistTarget(entryId));
      void syncDesktopPlanEventToApp({
        type: 'fastplan_session',
        session_id: sessionId,
        conversation_id: chat.currentConversationId,
      });
    }
  } else if (payload.type === 'busy-change') {
    applyFastPlanBusyChange(entryId, Boolean(payload.busy));
    if (payload.busy) updateFastPlanExecutionActivity(entryId, undefined, true);
  } else if (payload.type === 'state-change') {
    if (payload?.state?.connectionStatus === 'connected') fastPlanServiceStatus.markReady();
    const terminal = isFastPlanTerminalState(payload);
    const execStatus = String(payload?.state?.executionStatus || '').toLowerCase();
    const failedTerminal = terminal && (
      execStatus === 'failed'
      || execStatus === 'cancelled'
      || !!payload?.state?.planningError
      || Object.values(payload?.state?.executionState || {}).some(
        (n: any) => String(n?.status || '').toLowerCase() === 'failed',
      )
    );
    // 终态强制 busy=false，避免 node_failed 后仍按 plannedTree&&!treeCompleted 点亮停止按钮
    applyFastPlanBusyChange(entryId, terminal ? false : Boolean(payload.busy));
    updateFastPlanExecutionActivity(entryId, payload.state, terminal ? false : undefined);
    if (terminal) {
      if (!fastPlanEntries.value.some((entry) => entry.id === entryId)) {
        const target = resolveFastPlanBusyTarget(entryId);
        if (target) setFastPlanBusyFor(target, false);
        return;
      }
      fastPlanEntries.value = fastPlanEntries.value.map((entry) => entry.id === entryId
        ? { ...entry, status: failedTerminal ? 'failed' : 'complete' }
        : entry);
      const target = resolveFastPlanBusyTarget(entryId);
      if (target) setFastPlanBusyFor(target, false);
    }
  } else if (payload.type === 'execution-completed') {
    const target = resolveFastPlanBusyTarget(entryId);
    if (target) setFastPlanBusyFor(target, false);
    if (target) executionActivity.remove(target, 'fastplan');
    if (!fastPlanEntries.value.some((entry) => entry.id === entryId)) return;
    const failed = payload?.payload?.success === false
      || String(payload?.payload?.executionStatus || '').toLowerCase() === 'failed';
    fastPlanEntries.value = fastPlanEntries.value.map((entry) => entry.id === entryId
      ? { ...entry, status: failed ? 'failed' : 'complete' }
      : entry);
  } else if (payload.type === 'scroll-request') {
    // 用户上翻查看历史时不要被规划过程的贴底请求拽回去
    if (shouldAutoScroll.value) viewEffects.tick(() => pinWorkspaceToBottom());
  } else if (payload.type === 'error') {
    const target = resolveFastPlanBusyTarget(entryId);
    if (target) setFastPlanBusyFor(target, false);
    if (target) executionActivity.remove(target, 'fastplan');
    fastPlanEntries.value = fastPlanEntries.value.map((entry) => entry.id === entryId ? { ...entry, status: 'failed' } : entry);
    // node_failed 也会走这里；任务行已标红，避免再弹全局错误
    const message = String(payload.error?.message || '');
    const code = String(payload.error?.code || '');
    const isNodeFail = /node_failed|Node failed/i.test(message)
      || /node_failed/i.test(code)
      || (payload.error?.details && String((payload.error.details as any)?.type || '') === 'node_failed');
    if (message && !isNodeFail) ElMessage.error(message);
    void syncDesktopPlanEventToApp({
      type: 'error',
      content: message || '任务规划失败',
    });
    void syncDesktopPlanEventToApp({ type: 'complete', exit_code: -1 });
  } else if (payload.type === 'message-action') {
    const event = (payload.event || {}) as MessageActionEvent;
    if (event.action === 'resend') {
      const text = event.message
        ? rendererMessageText(event.message as any)
        : '';
      if (!text.trim()) return;
      if (fastPlanBusy.value) {
        ElMessage.warning(t('docGen.waitGenComplete'));
        return;
      }
      await resendUserQuestion(text);
      return;
    }
    if (event.action === 'regenerate') {
      if (fastPlanBusy.value) {
        ElMessage.warning(t('docGen.waitGenComplete'));
        return;
      }
      const entry = fastPlanEntries.value.find((item) => item.id === entryId);
      const prompt = String((entry?.data as FastPlanTimelineEntryData | undefined)?.prompt || '').trim();
      if (!prompt) {
        ElMessage.warning('无法重新生成：缺少原始规划问题');
        return;
      }
      if (!(await ensureModelProviderReady(modelId.value))) return;
      activeFastPlanEntryId.value = entryId;
      fastPlanPluginOptions.composerEntryId = entryId;
      await enqueueFastPlanPrompt(entryId, prompt, []);
      return;
    }
    if (event.action === 'like' || event.action === 'dislike') {
      const lease = chatContextBinding.captureUi();
      if (!lease?.isCurrent() || lease.context !== initialChatContext) return;
      const command = captureChatCommand(lease.context);
      const persistTarget = resolveFastPlanPersistTarget(entryId);
      const messageId = String(event.messageId || '');
      const feedback = Number((event.payload as { feedback?: number } | undefined)?.feedback);
      if (!messageId || !(feedback === 1 || feedback === -1 || feedback === 0)) return;
      // 若碰巧也在宿主 messages 中，同步写库
      const hostMessage = findHostMessage(messageId);
      if (hostMessage) {
        if (event.action === 'like') await handleLike(messageId, feedback);
        else await handleDislike(messageId, feedback);
      }
      // 规划条目内落盘，恢复会话后仍能显示赞/踩状态
      if (!command.isCurrent()) return;
      fastPlanEntries.value = fastPlanEntries.value.map((item) => {
        if (item.id !== entryId) return item;
        const data = { ...(item.data as FastPlanTimelineEntryData) } as FastPlanTimelineEntryData & {
          feedbackByMessageId?: Record<string, number>;
        };
        data.feedbackByMessageId = { ...(data.feedbackByMessageId || {}), [messageId]: feedback };
        return { ...item, data };
      });
      void persistFastPlanState(persistTarget);
    }
  }
}
async function onWorkspaceMessageAction(event: MessageActionEvent) {
  const message = findHostMessage(event.messageId);
  if (event.action === 'error-action') {
    return;
  }
  if (!message || event.action === 'copy') return;
  if (event.action === 'custom' && (event.payload as any)?.type === 'attachment-reference') {
    const rendererAttachment = (event.payload as any)?.attachment;
    const hostAttachment = rendererAttachment?.metadata?.hostAttachment as ChatAttachment | undefined
      || message.attachments?.find((attachment) => attachment.id === rendererAttachment?.id);
    if (hostAttachment?.type === 'document') handleDocQuote(hostAttachment);
  }
  else if (event.action === 'edit') handleEditMessage(message.id);
  else if (event.action === 'resend') await resendUserQuestion(message.content);
  else if (event.action === 'regenerate') await handleRegenerate(message.id);
  else if (event.action === 'delete') await handleDeleteMessage(message.id);
  else if (event.action === 'quote') handleQuote(message.content);
  else if (event.action === 'tts') handleTTS(message.id, message.content);
  else if (event.action === 'translate') await handleTranslate(message.id, event.menuItemId || 'zh');
  else if (event.action === 'favorite') await handleFavorite(message.id, message.is_favorite !== 1);
  else if (event.action === 'edit-as-doc') openEditor(message.content);
  else if (event.action === 'switch-model') onSingleMsgSwitchModel(message, event.messageId);
  else if (event.action === 'trace-source') handleTraceSource(event.menuItemId || 'result', message.id);
  else if (event.action === 'like') await handleLike(message.id, message.is_liked === 1 ? 0 : 1);
  else if (event.action === 'dislike') await handleDislike(message.id, message.is_liked === -1 ? 0 : -1);
  else if (event.action === 'feedback') {
    await qualityFeedbackActions.value?.request(message.id);
  }
  else if (event.action === 'branch') await handleBranch(message.id);
  else if (event.action === 'export') {
    await messageExportActions.value?.export(message.id, event.menuItemId || 'md');
  }
}

// 切换路由保留会话级流；关闭 Tab 时由 disposeChatStore 移交或停止。

// ─── 模型 / 知识库加载 ───
async function loadModels(force = false) {
  const isCurrentContext = tabState.capture();
  const selectedAtStart = modelId.value;
  try {
    const catalog = await loadChatModelCatalog(force);
    if (viewDisposed || tabState.disposed) return;
    // 供应商配置（base_url / api_key）
    try {
      const providers = catalog.providers;
      const map: Record<string, { id: string; baseUrl: string; hasApiKey: boolean }> = {};
      for (const p of providers) {
        const code = p.provider_code;
        if (!code) continue;
        map[code] = {
          id: p.id || "",
          baseUrl: p.base_url || "",
          hasApiKey: p.has_api_key,
        };
      }
      providerConfigMap.value = map;
    } catch (e) {
      console.warn("list_provider_summaries failed", e);
    }
    const tree: any[] = catalog.tree;
    modelTree.value = tree.map((p: any) => ({
      id: p.platformId,
      label:
        p.platformName ||
        p.platform_name ||
        p.name ||
        t("chat.defaultPlatform"),
      providerCode: p.providerCode || (p.platformId === "local" ? "LOCAL" : ""),
      sourceCategory: p.sourceCategory || "online",
      children: (p.modelList || p.models || p.children || []).map((m: any) => {
        const modelLabel = m.modelLabel || m.model_label || "";
        return {
          id: String(m.modelId || m.model_id || m.id),
          modelId: String(m.modelId || m.model_id || m.id),
          rawModelId: String(m.rawModelId || m.raw_model_id || ""),
          name: m.modelName || m.model_name || m.display_name || m.name || "",
          modelLabel,
          tags: toTagObjects(parseTags(modelLabel)),
          mxcs: m.mxcs ?? null,
        };
      }),
    }));

    // 独立模型数据模式：仅显示 sourceCategory === 'independent' 的分组
    if (appConfigStore.config.remote_servers?.independent_model_data) {
      modelTree.value = modelTree.value.filter(
        (g) => g.sourceCategory === 'independent'
      )
    }

    const pickFirstUsableModel = (): string => {
      for (const g of modelTree.value) {
        if (isGroupSkippedForAutoPick(g)) continue;
        if (g.children[0]) return g.children[0].id;
      }
      for (const g of modelTree.value) {
        if (isModelOptionDisabled(g)) continue;
        if (g.children[0]) return g.children[0].id;
      }
      return "";
    };

    const canUseModelId = (id: string): boolean => {
      for (const g of modelTree.value) {
        if (!g.children.some((m) => m.id === id)) continue;
        if (isModelOptionDisabled(g)) return false;
        return true;
      }
      return false;
    };

    if (!isCurrentContext() || modelId.value !== selectedAtStart) {
      void nextTick(() => { remoteModelSyncReady = true; });
      return;
    }
    if (!modelId.value) {
      const saved = localStorage.getItem("fastclaw-selected-model");
      if (saved && canUseModelId(saved)) modelId.value = saved;
      if (!modelId.value) {
        const defaultAssistant = settingsStore.settings["default_assistant_model"];
        if (defaultAssistant && canUseModelId(defaultAssistant)) {
          modelId.value = defaultAssistant;
        }
      }
      if (!modelId.value) {
        modelId.value = pickFirstUsableModel();
      }
    } else if (!canUseModelId(modelId.value)) {
      modelId.value = pickFirstUsableModel();
    }
    if (modelId.value) {
      lastUsableModelId.value = modelId.value;
    }
    // Let the initialization watcher flush before enabling remote side effects.
    void nextTick(() => { remoteModelSyncReady = true; });
  } catch (e) {
    console.warn("list_models_tree failed", e);
  }
}

watch(modelId, (v) => {
  if (v) localStorage.setItem("fastclaw-selected-model", v);
  if (applyingRemoteModel || !remoteModelSyncReady) return;
  const cid = chat.currentConversationId;
  if (cid) {
    const conv = chat.conversations.find((c) => c.id === cid);
    if (conv) conv.model_id = v || null;
  }
  void syncRemoteUiModel(v, cid);
});

watch(
  () => {
    const id = chat.currentConversationId;
    if (!id) return "";
    const conv = chat.conversations.find((c) => c.id === id);
    return conv?.model_id || "";
  },
  (mid) => {
    if (!mid || mid === modelId.value) return;
    const exists = modelTree.value.some((g) => g.children.some((m) => m.id === mid));
    if (!exists) return;
    applyingRemoteModel = true;
    modelId.value = mid;
    lastUsableModelId.value = mid;
    void nextTick(() => {
      applyingRemoteModel = false;
    });
  },
);

async function syncRemoteUiModel(id: string, conversationId?: string | null) {
  try {
    const invokeFn = await getInvoke();
    if (!invokeFn) return;
    await invokeFn("set_remote_ui_model", {
      modelId: id || null,
      conversationId: conversationId || null,
    });
  } catch {
    /* 远程未启用或旧版本二进制缺少该命令 */
  }
}

async function syncRemoteUiThinking(enabled: boolean) {
  try {
    const invokeFn = await getInvoke();
    if (!invokeFn) return;
    await invokeFn("set_remote_ui_thinking", { enabled });
  } catch {
    /* 远程未启用或旧版本二进制缺少该命令 */
  }
}

const selectModelRef = ref<InstanceType<typeof selectModel> | null>(null);

function advancedModelSearch() {
  selectModelRef.value?.open();
}

function onAdvancedModelSelect(id: string | number) {
  void (async () => {
    const mid = String(id);

    // 检查供应商配置完整性
    if (!(await ensureModelProviderReady(mid))) return;

    modelId.value = mid;
    lastUsableModelId.value = mid;
  })();
}

function resetChatDefaults() {
  selectedAgent.value = null;
  selectedAgentId.value = "";

  const saved = localStorage.getItem("fastclaw-selected-model");
  const savedExists = saved
    ? modelTree.value.some((g) => g.children.some((m) => m.id === saved))
    : false;
  if (saved && savedExists) {
    modelId.value = saved;
  } else {
    const defaultAssistant = settingsStore.settings["default_assistant_model"];
    const defaultExists = defaultAssistant
      ? modelTree.value.some((g) =>
        g.children.some((m) => m.id === defaultAssistant),
      )
      : false;
    if (defaultAssistant && defaultExists) {
      modelId.value = defaultAssistant;
    } else if (modelTree.value[0]?.children[0]) {
      modelId.value = modelTree.value[0].children[0].id;
    } else {
      modelId.value = "";
    }
  }

  // 非 Agent 对话恢复到普通聊天默认知识库状态：不预选任何知识库。
  knowledgeId.value = [];
}

// ─── 滚动 ───
/** 新版 workspace 实际滚动节点（.jcr-timeline），旧版为 el-scrollbar wrap */
function getWorkspaceScrollEl(): HTMLElement | null {
  const root = chatMainRef.value;
  if (!root) return null;
  return (
    root.querySelector<HTMLElement>('.jcr-timeline') ||
    root.querySelector<HTMLElement>('.fastclaw-chat-workspace .jcr-panel') ||
    null
  );
}

function getScrollWrap(): HTMLElement | null {
  if (useChatPlanPackages.value) return getWorkspaceScrollEl();
  return elScrollbarRef.value?.wrapRef ?? null;
}

const showScrollButton = ref(false);
let scrollPending = false;
let workspaceFollowRaf = 0;
let workspaceFollowDirty = false;
let workspaceScrollElBound: HTMLElement | null = null;
let workspaceProgrammaticScroll = false;
let workspaceProgrammaticUntil = 0;
let workspaceGrowObserver: ResizeObserver | null = null;
let workspaceTouchY = 0;
let workspacePostPaintReady = false;

function markWorkspaceProgrammatic(ms = 48) {
  workspaceProgrammaticScroll = true;
  workspaceProgrammaticUntil = performance.now() + ms;
}

function isWorkspaceProgrammatic() {
  return workspaceProgrammaticScroll || performance.now() < workspaceProgrammaticUntil;
}

function stopWorkspaceFollow() {
  if (workspaceFollowRaf) {
    cancelAnimationFrame(workspaceFollowRaf);
    workspaceFollowRaf = 0;
  }
  workspaceFollowDirty = false;
}

function pauseWorkspaceFollow() {
  shouldAutoScroll.value = false;
  stopWorkspaceFollow();
}

function workspaceDistanceFromBottom(wrap: HTMLElement) {
  return wrap.scrollHeight - wrap.scrollTop - wrap.clientHeight;
}

function resumeWorkspaceFollowIfNearBottom(wrap: HTMLElement) {
  const distance = workspaceDistanceFromBottom(wrap);
  showScrollButton.value = distance > 15;
  if (distance > 80) return;
  if (distance >= 48) return;
  const streaming = inConversation.value || fastPlanBusy.value;
  const wasAuto = shouldAutoScroll.value;
  shouldAutoScroll.value = true;
  if (!wasAuto && streaming) engageFollowStream();
}

/**
 * 通过 ChatWorkspace 的正式导航 API 选中最新问题；FastPlan 内部问题与
 * 普通会话问题使用同一套状态。贴底跟随时不要 scrollToEntry，否则会把视口拽回问题顶部。
 */
function syncLatestQuestionNavActive(options?: { scrollEntry?: boolean }) {
  if (!useChatPlanPackages.value) return;
  const api = chatWorkspaceRef.value as {
    activateLatestQuestion?: () => boolean;
    scrollToLatestQuestion?: (behavior?: ScrollBehavior) => unknown;
    scrollToEntry?: (entryId: string, behavior?: ScrollBehavior) => unknown;
  } | null;
  if (options?.scrollEntry === false) api?.activateLatestQuestion?.();
  else api?.scrollToLatestQuestion?.('auto');
}

const NESTED_PLAN_BOX_SEL = '.fp-task__details, .fp-planning-output, .fp-raw-logs, .jcr-plan-tasks__body';

function closestNestedPlanBox(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof Element)) return null;
  return target.closest(NESTED_PLAN_BOX_SEL);
}

function pinWorkspaceToBottom() {
  if (!useChatPlanPackages.value || !shouldAutoScroll.value) return;
  // KeepAlive 失活后不调度 RAF，也不读取脱离 document 的 DOM。
  if (!fastPlanWelcomeViewActive.value || route.path !== myPath) return;
  workspaceFollowDirty = true;
  if (workspaceFollowRaf) return;
  workspaceFollowRaf = requestAnimationFrame(() => {
    workspaceFollowRaf = 0;
    if (!workspaceFollowDirty || !shouldAutoScroll.value) return;
    workspaceFollowDirty = false;
    if (!fastPlanWelcomeViewActive.value || route.path !== myPath) return;
    const timeline = getWorkspaceScrollEl();
    if (!timeline?.isConnected) return;
    markWorkspaceProgrammatic();
    try {
      const api = chatWorkspaceRef.value as { scrollToBottom?: (behavior?: ScrollBehavior) => unknown } | null;
      if (api?.scrollToBottom) api.scrollToBottom('auto');
      else timeline.scrollTop = timeline.scrollHeight;
    } catch {
      timeline.scrollTop = timeline.scrollHeight;
    }
    workspaceProgrammaticScroll = false;
  });
}

/** 流式更新只标记一次贴底；新 token/ResizeObserver 会触发下一帧。 */
function engageFollowStream() {
  if (!useChatPlanPackages.value) return;
  shouldAutoScroll.value = true;
  pinWorkspaceToBottom();
}

function scrollToBottom(smooth = false) {
  viewEffects.tick(() => {
    viewEffects.frame(() => {
      if (useChatPlanPackages.value) {
        if (!shouldAutoScroll.value) return;
        engageFollowStream();
        return;
      }
      const wrap = getScrollWrap();
      if (!wrap) return;
      if (smooth) {
        wrap.scrollTo({ top: wrap.scrollHeight, behavior: "smooth" });
      } else {
        elScrollbarRef.value?.setScrollTop(wrap.scrollHeight);
      }
    });
  });
}

function scrollToBottomThrottled() {
  if (!viewEffects.isActive() || scrollPending) return;
  scrollPending = true;
  if (useChatPlanPackages.value) {
    if (shouldAutoScroll.value) pinWorkspaceToBottom();
  } else {
    scrollToBottom();
  }
  viewEffects.frame(() => {
    scrollPending = false;
  });
}

function scrollToBottomAndEngage() {
  shouldAutoScroll.value = true;
  if (useChatPlanPackages.value) engageFollowStream();
  else scrollToBottom(true);
}

let loadingMore = false;

function onWorkspaceTimelineScroll(event: Event) {
  // 用户滚动时关闭锚点下拉，避免固定定位与按钮错位；程序贴底滚动不关
  if (switchModelAnchorActive.value && !isWorkspaceProgrammatic()) {
    const elSelect = switchModelAnchorSelectorRef.value?.modelSelector as
      | { blur?: () => void }
      | null
      | undefined;
    elSelect?.blur?.();
    closeSwitchModelAnchor();
  }
  const wrap = event.currentTarget as HTMLElement | null;
  if (!wrap) return;
  const distance = workspaceDistanceFromBottom(wrap);
  showScrollButton.value = distance > 15;
  if (isWorkspaceProgrammatic()) return;
  if (distance > 80) {
    pauseWorkspaceFollow();
    return;
  }
  resumeWorkspaceFollowIfNearBottom(wrap);
}

function onWorkspaceUserScrollIntent(event: Event) {
  // 红框任务详情自己滚动，不要当成外层对话滚轮，也不要贴底打断一点点滚动
  if (closestNestedPlanBox(event.target)) return;
  if (event.type === 'wheel' && (event as WheelEvent).deltaY < 0) {
    pauseWorkspaceFollow();
    return;
  }
  if (event.type === 'touchstart') {
    workspaceTouchY = (event as TouchEvent).touches[0]?.clientY ?? 0;
    return;
  }
  if (event.type === 'touchmove') {
    const y = (event as TouchEvent).touches[0]?.clientY ?? workspaceTouchY;
    if (y > workspaceTouchY + 4) pauseWorkspaceFollow();
    workspaceTouchY = y;
  }
  const wrap = getWorkspaceScrollEl();
  if (!wrap) return;
  viewEffects.frame(() => {
    if (!wrap.isConnected || wrap !== getWorkspaceScrollEl() || isWorkspaceProgrammatic()) return;
    const distance = workspaceDistanceFromBottom(wrap);
    if (distance > 80) pauseWorkspaceFollow();
    else resumeWorkspaceFollowIfNearBottom(wrap);
  });
}

function attachWorkspaceGrowObserver() {
  if (!workspacePostPaintReady || !fastPlanWelcomeViewActive.value || route.path !== myPath) return;
  const wrap = getWorkspaceScrollEl();
  if (!wrap || typeof ResizeObserver === 'undefined') return;
  if (!workspaceGrowObserver) {
    workspaceGrowObserver = new ResizeObserver(viewEffects.guard(() => {
      if (shouldAutoScroll.value) pinWorkspaceToBottom();
    }));
  }
  workspaceGrowObserver.disconnect();
  for (const child of wrap.children) {
    if (child instanceof Element) workspaceGrowObserver.observe(child);
  }
  wrap.querySelectorAll('.fp-task__details, .fp-planning-output, .fp-raw-logs, .jcr-plan-tasks__body').forEach((el) => {
    workspaceGrowObserver!.observe(el);
  });
}

function detachWorkspaceGrowObserver() {
  workspaceGrowObserver?.disconnect();
  workspaceGrowObserver = null;
}

function attachWorkspaceScrollListener() {
  if (!viewEffects.isActive() || !workspacePostPaintReady || !useChatPlanPackages.value) return;
  const wrap = getWorkspaceScrollEl();
  if (wrap && wrap !== workspaceScrollElBound) {
    detachWorkspaceScrollListener();
    workspaceScrollElBound = wrap;
    wrap.addEventListener('scroll', onWorkspaceTimelineScroll, { passive: true });
    wrap.addEventListener('wheel', onWorkspaceUserScrollIntent, { passive: true, capture: true });
    wrap.addEventListener('touchstart', onWorkspaceUserScrollIntent, { passive: true, capture: true });
    wrap.addEventListener('touchmove', onWorkspaceUserScrollIntent, { passive: true, capture: true });
  }
  attachWorkspaceGrowObserver();
}

function detachWorkspaceScrollListener() {
  if (!workspaceScrollElBound) return;
  workspaceScrollElBound.removeEventListener('scroll', onWorkspaceTimelineScroll);
  workspaceScrollElBound.removeEventListener('wheel', onWorkspaceUserScrollIntent, true);
  workspaceScrollElBound.removeEventListener('touchstart', onWorkspaceUserScrollIntent, true);
  workspaceScrollElBound.removeEventListener('touchmove', onWorkspaceUserScrollIntent, true);
  workspaceScrollElBound = null;
}

watch(useChatPlanPackages, (enabled) => {
  if (enabled && workspacePostPaintReady && fastPlanWelcomeViewActive.value) {
    viewEffects.tick(() => attachWorkspaceScrollListener());
  }
  else {
    detachWorkspaceScrollListener();
    detachWorkspaceGrowObserver();
    stopWorkspaceFollow();
    if (workspacePostPaintReady) viewEffects.tick(() => attachNavResizeObserver());
  }
}, { immediate: true });

watch(() => chat.messages.length, () => {
  if (useChatPlanPackages.value && workspacePostPaintReady && fastPlanWelcomeViewActive.value) {
    viewEffects.tick(() => attachWorkspaceScrollListener());
  } else if (workspacePostPaintReady) {
    viewEffects.tick(() => attachNavResizeObserver());
  }
});

function onScroll({ scrollTop }: { scrollTop: number; scrollLeft: number }) {
  const wrap = getScrollWrap();
  if (!wrap) return;
  const distance = wrap.scrollHeight - scrollTop - wrap.clientHeight;
  showScrollButton.value = distance > 15;
  shouldAutoScroll.value = distance < 12;
  // 滚动到顶：加载更早历史（游标分页，方案 3）
  if (scrollTop <= 5 && chat.hasMore && !loadingMore && chat.currentConversationId) {
    const cid = chat.currentConversationId;
    const prevScrollHeight = wrap.scrollHeight;
    loadingMore = true;
    const restoreViewport = viewEffects.guard(() => viewEffects.tick(() => {
      const w = getScrollWrap();
      if ((chat as unknown as DeferredChatStore).currentConversationId === cid && w === wrap && w.isConnected) {
        w.scrollTop = w.scrollHeight - prevScrollHeight;
      }
    }));
    chat.loadMoreMessages(cid).finally(() => {
      loadingMore = false;
      // 数据加载不随 UI 停止；仅旧视图的高度补偿失效。
      restoreViewport();
    });
  }
}
function scrollToQuestion(index: number) {
  if (!viewEffects.isActive() || !scrollContainerRef.value) return;
  const gi = questionGroupIndices.value[index];
  if (gi === undefined) return;
  const el = scrollContainerRef.value.querySelector(
    `[data-group-index="${gi}"]`,
  ) as HTMLElement | undefined;
  if (el) {
    el.scrollIntoView({ behavior: "smooth", block: "start" });
    highlightedIndex.value = index;
    viewEffects.delay(() => {
      highlightedIndex.value = -1;
    }, 2500);
  }
}

function isQuestionHighlighted(gIdx: number): boolean {
  if (highlightedIndex.value < 0) return false;
  return questionGroupIndices.value[highlightedIndex.value] === gIdx;
}

watch(
  () => chat.messages.length,
  (len, prev) => {
    if (len <= (prev ?? 0)) return;
    if (useChatPlanPackages.value) {
      if (shouldAutoScroll.value) viewEffects.tick(() => engageFollowStream());
    } else {
      shouldAutoScroll.value = true;
      scrollToBottom(true);
    }
  },
);
watch(
  () => {
    const msg = chat.messages[chat.messages.length - 1];
    return msg ? `${msg.content}\0${msg.think_content ?? ''}\0${msg.status}\0${msg.stream_phase ?? ''}` : '';
  },
  () => {
    if (!shouldAutoScroll.value) return;
    if (useChatPlanPackages.value) pinWorkspaceToBottom();
    else scrollToBottomThrottled();
  },
);
watch(inConversation, (busy, wasBusy) => {
  const conversationId = chat.currentConversationId;
  if (conversationId) {
    if (busy) {
      executionActivity.update(conversationId, myPath, 'chat', {
        phase: 'executing',
        protected: true,
      });
      appStore.setTabUnfinished(myPath, 'chat', true);
    } else {
      executionActivity.remove(conversationId, 'chat');
      appStore.setTabUnfinished(myPath, 'chat', false);
    }
  }
  if (!useChatPlanPackages.value) return;
  if (busy && !wasBusy && shouldAutoScroll.value) viewEffects.tick(() => engageFollowStream());
});
watch(fastPlanBusy, (busy, wasBusy) => {
  if (!useChatPlanPackages.value) return;
  if (busy && !wasBusy && shouldAutoScroll.value) viewEffects.tick(() => engageFollowStream());
});
watch(
  () => {
    const last = fastPlanEntries.value.at(-1);
    return last ? `${fastPlanEntries.value.length}:${last.id}:${last.status}` : '0';
  },
  () => {
    if (!useChatPlanPackages.value || !shouldAutoScroll.value) return;
    if (fastPlanBusy.value) pinWorkspaceToBottom();
  },
);

// ─── 发送 / 停止 ───
// autoSendAfterStop: 在流式回答进行中触发的操作，是否在停止后继续发送输入框内容
//   - 点击按钮 → false（仅停止）
//   - 按 Enter → true（停止 + 发送新内容）
async function sendBtnFn(autoSendAfterStop = false) {
  if (messageDeletionService.isPending(answerChat.currentConversationId)) return;
  const lease = chatContextBinding.captureUi();
  if (!lease?.isCurrent() || lease.context !== initialChatContext) return;
  if (pendingAutoSend.value) { deferredChatSubmissionService.cancel(tabState); return; }
  if (sendBtnLock.value) return;
  const draftCurrent = tabState.capture();
  const sourceConversationId = chat.currentConversationId;
  const sourceModelId = modelId.value;
  const sourceTaskPlan = dialogueCharacter.value.taskPlan;
  const sourceQuote = quotedText.value;
  let preparation: SendPreparation | undefined;
  const isCurrentRequest = () => lease.isCurrent() && draftCurrent() && !viewDisposed
    && (!preparation || preparation.isCurrent())
    && chat.currentConversationId === sourceConversationId && modelId.value === sourceModelId
    && dialogueCharacter.value.taskPlan === sourceTaskPlan;
  sendBtnLock.value = true;
  try {
    if (!editorVisible.value && !sourceTaskPlan) {
      preparation = captureSendPreparation(lease, () => [
        remoteKbIds.value, localKbPaths.value,
        selectedAgent.value && [selectedAgent.value.id, selectedAgent.value.agent_name, selectedAgent.value.agent_icon],
        assistantStore.activeRole?.systemPrompt, ws.enabled, ws.provider,
        mcpChat.enabled, mcpChat.cfg.serverIds, isVisionModel.value, selectedModelGroup.value?.providerCode,
      ]);
    }
    // ── 文档编辑模式：发送修改指令而非普通对话 ──
    if (editorVisible.value) {
      if (inConversation.value) {
        await chat.stopStream(chat.currentConversationId || undefined);
        return;
      }
      preparation = captureSendPreparation(lease, () => [
        editorContent.value, activeVersionIdx.value, docVersions.value[activeVersionIdx.value],
        getProviderInfoForModel(sourceModelId)?.providerCode,
      ]);
      if (!isCurrentRequest()) return;
      preparation.commit();
      await sendDocEditMessage();
      return;
    }
    if (dialogueCharacter.value.taskPlan) {
      if (!useChatPlanPackages.value) {
        ElMessage.warning('请先启用新版规划界面');
        return;
      }
      const raw = messageStr.value.trim();
      if (!raw) return;
      const content = composeWithQuote(raw);
      const inputAttachments = fileList.value.slice();
      const inputQuote = quotedText.value;
      const attachments: FastPlanPendingAttachment[] = inputAttachments.map((attachment) => ({
        id: attachment.id,
        name: attachment.name,
        size: attachment.size,
        mimeType: fileRefs.get(attachment.id)?.type,
        file: fileRefs.get(attachment.id),
        metadata: { hostType: attachment.type },
      }));
      try {
        if (!isCurrentRequest()) return;
        if (!(await ensureModelProviderReady(modelId.value, content))) {
          ElMessage.warning('模型暂不可用，请完成配置后重试');
          return;
        }
        if (!isCurrentRequest()) return;
        await planningPreparationService.submit({
          owner: tabState, store: chat as unknown as PlanningPreparationStore,
          tabPath: myPath, conversationId: sourceConversationId,
          prompt: content, raw, quote: inputQuote, draftAttachments: inputAttachments, attachments,
          model: selectedFastPlanModel(), executionMode: fastPlanExecutionMode.value,
          thinking: dialogueCharacter.value.think,
          apiBase: fastPlanPluginOptions.apiBase, wsBase: fastPlanPluginOptions.wsBase,
          uploadBase: fastPlanPluginOptions.uploadBase,
        });
      } catch (error) {
        const message = error instanceof Error && error.message
          ? error.message
          : (typeof error === 'string' && error.trim()
            ? error
            : '创建规划会话失败，请稍后重试');
        // Retained entry/draft errors belong to the service, never to a disposed view.
        if (!viewDisposed && !tabState.disposed) ElMessage.error(message);
      }
      return;
    }
    if (inConversation.value) {
      await preparation!.wait(chat.stopStream(sourceConversationId || undefined));
      if (!isCurrentRequest() || inConversation.value) return;
      // 点击按钮仅停止，按 Enter 则停止后继续发送输入框内容
      if (!autoSendAfterStop) return;
    }
    const raw = messageStr.value.trim();
    if (!raw) return;
    if (!modelId.value) {
      ElMessage.warning(t("chat.pleaseSelectModel"));
      return;
    }
    if (!isCurrentRequest()) return;
    const content = composeWithQuote(raw);
    if (!(await preparation!.wait(ensureModelProviderReady(sourceModelId, content, preparation)))) return;
    if (!isCurrentRequest()) return;
    const attachments = fileList.value.slice();
    let waitForUploads = false;

    // 检查上传中/失败的文件
    const errorAttachments = fileList.value.filter((f) => f.status === "error");
    const uploadingAttachments = fileList.value.filter(
      (f) => f.status === "uploading",
    );
    if (errorAttachments.length > 0) {
      const errNames = errorAttachments.map((f) => f.name).join("、");
      ElMessage.warning(`文件「${errNames}」上传失败，将作为普通问答进行对话`);
      preparation!.mutate(() => { for (const file of [...fileList.value]) {
        if (file.status === 'error' || file.status === 'uploading') tabState.removeAttachment(file.id);
      } });
    } else if (uploadingAttachments.length > 0) {
      // 有文件正在上传中：二次确认是否等待完成后发送
      const uploadingNames = uploadingAttachments.map((f) => f.name).join("、");
      const uploadedCount = attachments.filter(
        (a) => a.status === "uploaded" && a.type === "document",
      ).length;
      const tip = uploadedCount > 0
        ? `文件「${uploadingNames}」正在上传中（已有 ${uploadedCount} 个文件上传完成）。是否等待上传完成后再发送？`
        : `文件「${uploadingNames}」正在上传中，是否等待上传完成后再发送？`;
      const confirmationDraft = messageStr.value;
      const confirmationQuote = quotedText.value;
      const confirmationFiles = JSON.stringify(fileList.value.map(file => file.id));
      const isCurrentConfirmation = () => isCurrentRequest()
        && messageStr.value === confirmationDraft && quotedText.value === confirmationQuote
        && JSON.stringify(fileList.value.map(file => file.id)) === confirmationFiles;
      try {
        await preparation!.wait(ElMessageBox.confirm(tip, "文件上传中", {
          confirmButtonText: "等待上传完成后发送",
          cancelButtonText: "作为普通问答发送",
          type: "warning",
          distinguishCancelAndClose: true,
        }));
        if (!isCurrentConfirmation()) return;
        waitForUploads = true;
      } catch (action) {
        if (!isCurrentConfirmation()) return;
        if (action !== 'cancel') {
          // Provider completion may already have cleared the original draft for its timeline.
          return; // Closing the dialog is not consent to send without attachments.
        }
        // 选择普通问答：移除上传中的文件，仅保留已上传的
        preparation!.mutate(() => { for (const file of [...fileList.value]) {
          if (file.status === 'uploading') tabState.removeAttachment(file.id);
        } });
      }
    }

    if (!isCurrentRequest()) return;
    // Provider completion can consume the draft for its own timeline; restore it
    // before transferring ownership so preparation failure/cancellation remains recoverable.
    preparation!.mutate(() => { messageStr.value = raw; quotedText.value = sourceQuote; });
    preparation!.commit();
    void deferredChatSubmissionService.submit({
      owner: tabState, store: chat as unknown as DeferredChatStore, tabPath: myPath,
      conversationId: sourceConversationId, raw, quote: sourceQuote, content,
      modelId: sourceModelId, vision: isVisionModel.value, waitForUploads,
      knowledgeBaseIds: remoteKbIds.value, localKbPaths: localKbPaths.value,
      systemPrompt: selectedAgent.value ? undefined : assistantStore.activeRole?.systemPrompt,
      assistantId: selectedAgent.value?.id,
      assistantDisplay: selectedAgent.value ? {
        agent_name: selectedAgent.value.agent_name,
        agent_icon: selectedAgent.value.agent_icon || agentDefaultIcon,
      } : undefined,
      webSearch: ws.enabled, searchProvider: ws.provider, thinking: dialogueCharacter.value.think,
      mcpTools: mcpChat.enabled, mcpServerIds: mcpChat.cfg.serverIds,
      skillName: selectedSkill.value?.name, providerCode: selectedModelGroup.value?.providerCode,
    });
  } catch (error) {
    if (isCurrentRequest()) ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    preparation?.dispose();
    sendBtnLock.value = false;
  }
}

let lastEnterTime = 0;
function keydownEnterFn(e: KeyboardEvent) {
  if (e.shiftKey) return;
  // 防抖：500ms 内不重复触发（IME 输入法可能导致 Enter 事件触发两次）
  const now = Date.now();
  if (now - lastEnterTime < 500) return;
  lastEnterTime = now;
  e.preventDefault();
  sendBtnFn(true);
}

// 重置当前实例的本地输入/编辑器草稿态（不触碰会话数据）
function resetLocalNewChatState() {
  conversationLoadSequence++;
  tabState.resetConversationUi();
  tabState.hydrationRequired = false;
  dropSessionRevision.value++;
  messageStr.value = "";
  clearQuote();
  tabState.clearAttachments();
  fileKbIds.value = [];
  conversationFileIds.value = [];
  resetChatDefaults();
  rightPanel.value = "none";
  currentWebSearchMessageId.value = null;
  fastPlanStore.clearCurrentSession();
  fastPlanDataObj.value = null;
  fastPlanEntries.value = [];
  activeFastPlanEntryId.value = null;
  fastPlanPluginOptions.composerEntryId = null;
  fastPlanBusyByConv.value = new Map();
  fastPlanEntryOwners.clear();
  fastPlanExecutionMode.value = 'auto';
  if (fastPlanWriterConversationId) {
    releaseFastPlanWriter(fastPlanWriterConversationId, planWriterId);
    fastPlanWriterConversationId = null;
  }
  dialogueCharacter.value.taskPlan = DEFAULT_TASK_PLAN_ENABLED;
  // 隐藏并重置文档编辑器
  editorVisible.value = false;
  editorContent.value = "";
  editorTitle.value = "";
  docVersions.value = [];
  activeVersionIdx.value = 0;
  Object.keys(docEditCardMap).forEach((k) => delete docEditCardMap[k]);
  Object.keys(docEditMessageIds).forEach((k) => delete (docEditMessageIds as Record<string, true>)[k]);
  isHistoryCollapsed.value = false;
}

// ─── 新建对话 ───
function newCreateChat() {
  // 多实例模式：若 tab 栏已存在「空白新建对话」tab，直接跳转复用，不再重复创建空 tab。
  const blankPath = appStore.findBlankChatTab()
  if (blankPath && blankPath !== myPath) {
    router.push({ path: blankPath, query: { title: 'sidebar.smartChat' } })
    return
  }
  if (blankPath === myPath) {
    // 当前即空白 tab：仅清理本地草稿态，不另开 tab。
    chat.resetToHome();
    resetLocalNewChatState();
    appStore.setTabTitle(myPath, 'sidebar.smartChat');
    return
  }
  // 当前 tab 已非空白且无其它可复用空白 tab：原地重置为空白首页（保留在现 tab）。
  if (!chat.currentConversationId && chat.messages.length === 0) {
    chat.resetToHome();
    resetLocalNewChatState();
    appStore.setTabTitle(myPath, 'sidebar.smartChat');
    return
  }
  // 否则新开一个独立对话 tab（独立 chat store 实例），当前对话原样保留在旧 tab。
  // 路由切到新 iid -> keep-alive 以 route.path 为 key 创建全新 ChatView 实例
  // -> TabBar 的 watch 检测到新 path 自动 push 一个新（可关闭、标题「智能对话」）tab。
  const iid = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
  router.push({ path: `/chat/${iid}`, query: { title: 'sidebar.smartChat' } });
}

// ─── 切换会话 ───
async function selectConversation(id: string, options: { hydrate?: boolean; messageId?: string } = {}) {
  // 收藏跳转：先关掉贴底跟随，避免 load 后的消息 watch 把视口拽到底部
  if (options.messageId) shouldAutoScroll.value = false;
  // 执行中的会话固定占用当前 Tab。历史会话优先跳转到已有 Tab，
  // 未打开时新建 Chat Tab，禁止卸载当前执行链。
  if (id !== chat.currentConversationId && executionActivity.isTabProtected(myPath)) {
    await navigateToConversation(router, id, { messageId: options.messageId });
    return false;
  }
  const openedHere = await openConversationInTab(
    router,
    myPath,
    id,
    () => loadConversationDetails(id),
    {
      ...options,
      // registry 与当前 store 不同步时强制 load，否则会跳过加载，表现为历史点击无反应
      hydrate: options.hydrate || chat.currentConversationId !== id,
    },
  );
  if (!openedHere || chat.currentConversationId !== id) {
    return false;
  }
  syncForegroundConversation();
  if (options.messageId) {
    shouldAutoScroll.value = false;
    await scrollToFavMessage(options.messageId);
  }
  return true;
}

let conversationLoadSequence = 0;
async function loadConversationDetails(id: string) {
  tabState.hydrationRequired = true;
  const sequence = ++conversationLoadSequence;
  const trace = (stage: string, details: Record<string, unknown> = {}) => console.info(`[FastPlanHistory] ${stage}`, JSON.stringify({ path: myPath, conversationId: id, sequence, ...details }));
  const isCurrentSelection = () => {
    // 准备阶段刻意不修改 currentConversationId；sequence 是唯一提交守卫。
    const current = !viewDisposed && !tabState.disposed && sequence === conversationLoadSequence;
    if (!current) trace('view.load.skipped.stale', { latestSequence: conversationLoadSequence, currentId: chat.currentConversationId });
    return current;
  };
  trace('view.load.begin', { packagesEnabled: useChatPlanPackages.value });
  chatSwitchPerf.markHydrateStarted(myPath);
  conversationRestoring.value = true;
  loadingFastPlanEntries.value.clear();
  const previousConversationId = chat.currentConversationId;
  try {
    // 切走前：钉住仍在执行的会话 busy，并落盘（须在 loadMessages 改 id 之前）
    if (previousConversationId && previousConversationId !== id) {
      const leavingBusy = fastPlanBusy.value
        || fastPlanEntries.value.some((entry) => entry.status === 'active');
      for (const entry of fastPlanEntries.value) {
        rememberFastPlanEntryOwner(entry.id, previousConversationId);
      }
      if (leavingBusy) setFastPlanBusyFor(previousConversationId, true);
      if (fastPlanEntries.value.length) {
        // 切会话不要 await 落盘：IPC 卡住时历史点击会像没反应
        const snapshot = fastPlanEntries.value.slice();
        const activeId = activeFastPlanEntryId.value;
        void persistFastPlanState(previousConversationId, {
          entries: snapshot,
          activeEntryId: activeId,
          allowNonCurrent: true,
        });
      }
    }
    const preparedMessages = await chat.prepareMessages(id);
    if (!isCurrentSelection() || !preparedMessages) return;

    // 先持久化当前对话的文件缓存和 doc-edit 状态
    persistFileCache();
    saveDocEditState();
    tabState.resetConversationUi();
    if (fastPlanWriterConversationId) {
      releaseFastPlanWriter(fastPlanWriterConversationId, planWriterId);
      fastPlanWriterConversationId = null;
    }
    selectedAgent.value = null;
    selectedAgentId.value = "";
    selectedSkill.value = null;
    fastPlanStore.clearCurrentSession();
    fastPlanDataObj.value = null;
    fastPlanEntries.value = [];
    activeFastPlanEntryId.value = null;
    fastPlanPluginOptions.composerEntryId = null;
    // 不清除其他会话的 busy 记录；按钮状态由当前会话 id 从 fastPlanBusyByConv 推导
    dialogueCharacter.value.taskPlan = false;
    rightPanel.value = "none";
    currentWebSearchMessageId.value = null;
    // 唯一提交点：数据已准备好后再一次性替换会话和消息，
    // 避免先清空、等 IPC、再回填造成的白屏。
    if (!isCurrentSelection()) return;
    chat.commitPreparedMessages(preparedMessages);
    trace('view.messages.loaded', { messageCount: chat.messages.length });
    if (!isCurrentSelection()) return;
    loadDocEditState(id);
    restoreFilesFromMessages();
    // 检查该会话是否关联 FastPlan session，若是则自动加载
    const conv = chat.conversations.find((c) => c.id === id);
    trace('view.conversation.found', { found: !!conv, mode: conv?.mode, modelId: conv?.model_id, hasExtraConfig: !!conv?.extra_config });
    if (conv?.model_id) {
      const exists = modelTree.value.some((g) => g.children.some((m) => m.id === conv.model_id));
      if ((exists || modelTree.value.length === 0) && conv.model_id !== modelId.value) {
        applyingRemoteModel = true;
        modelId.value = conv.model_id;
        lastUsableModelId.value = conv.model_id;
        void nextTick(() => {
          applyingRemoteModel = false;
        });
      }
    }
    await restoreAgentConversation(conv, isCurrentSelection);
    trace('view.agent.restored');
    if (!isCurrentSelection()) return;
    if (conv?.extra_config && useChatPlanPackages.value) {
      const persisted = readFastPlanPersistence(conv.extra_config);
      const rememberedBusy = !!fastPlanBusyByConv.value.get(id);
      // 宿主已有普通对话时，优先展示普通历史，不要先挂规划条目/loading（否则会一直转圈，随后落到规划欢迎页）
      const hasOrdinaryMessages = chat.messages.some((m) => m.role === 'user' || m.role === 'assistant');
      trace('view.persistence.parsed', { version: persisted.version, revision: persisted.revision, activeEntryId: persisted.activeEntryId, rememberedBusy, hasOrdinaryMessages, entries: persisted.entries.map(entry => ({ id: entry.id, sessionId: entry.sessionId, status: entry.status })) });
      if (hasOrdinaryMessages) {
        let remoteOnly = false;
        try {
          const cfg = JSON.parse(conv.extra_config || '{}');
          remoteOnly = cfg.fastplan_remote === true;
        } catch {
          remoteOnly = false;
        }
        if (remoteOnly) {
          // App 同步过来的规划：普通消息时间线 + planRuntime，不挂本地 FastPlan 条目
          dialogueCharacter.value.taskPlan = conversationTaskPlanEnabled(conv, 0);
          applyConversationExecutionMode(conv);
          fastPlanEntries.value = [];
          activeFastPlanEntryId.value = null;
          loadingFastPlanEntries.value.clear();
          fastPlanPluginOptions.composerEntryId = null;
          if (persisted.entries.length) void clearFastPlanPersistence(id);
          trace('view.timeline.skipped.ordinary', {
            messageCount: chat.messages.length,
            clearedPlanEntries: persisted.entries.length,
            remoteOnly: true,
          });
        } else {
          // 本机 FastPlan：即使已镜像消息到 App，仍恢复本地时间线条目
          fastPlanEntries.value = deferCompletedFastPlanHistory(persistedEntriesToTimeline(persisted).map((entry) => {
            rememberFastPlanEntryOwner(entry.id, id);
            const shouldKeepLive = rememberedBusy || entry.status === 'active';
            if (!shouldKeepLive) return entry;
            return {
              ...entry,
              status: 'active',
              data: {
                ...(entry.data as FastPlanTimelineEntryData),
                connectionMode: 'live',
              },
            };
          }));
          loadingFastPlanEntries.value = new Set(fastPlanEntries.value
            .filter(entry => !(entry.data as FastPlanTimelineEntryData).deferLoad)
            .map(entry => entry.id));
          activeFastPlanEntryId.value = fastPlanEntries.value.at(-1)?.id || null;
          dialogueCharacter.value.taskPlan = conversationTaskPlanEnabled(conv, fastPlanEntries.value.length);
          fastPlanPluginOptions.composerEntryId = dialogueCharacter.value.taskPlan ? activeFastPlanEntryId.value : null;
          const restoredExecutionMode = (fastPlanEntries.value.at(-1)?.data as FastPlanTimelineEntryData | undefined)?.executionMode;
          applyConversationExecutionMode(conv, restoredExecutionMode);
          if (rememberedBusy || fastPlanEntries.value.some((entry) => entry.status === 'active')) {
            setFastPlanBusyFor(id, true);
          }
          trace('view.timeline.restored.withMessages', {
            entryCount: fastPlanEntries.value.length,
            taskPlan: dialogueCharacter.value.taskPlan,
            messageCount: chat.messages.length,
          });
        }
      } else {
        fastPlanEntries.value = deferCompletedFastPlanHistory(persistedEntriesToTimeline(persisted).map((entry) => {
          rememberFastPlanEntryOwner(entry.id, id);
          const shouldKeepLive = rememberedBusy || entry.status === 'active';
          if (!shouldKeepLive) return entry;
          return {
            ...entry,
            status: 'active',
            data: {
              ...(entry.data as FastPlanTimelineEntryData),
              // 仍在执行时回切用 live，避免历史回显把停止按钮冲掉
              connectionMode: 'live',
            },
          };
        }));
        loadingFastPlanEntries.value = new Set(fastPlanEntries.value
          .filter(entry => !(entry.data as FastPlanTimelineEntryData).deferLoad)
          .map(entry => entry.id));
        // 回显始终以时间线最后一条规划会话作为输入框上方的任务列表来源。
        activeFastPlanEntryId.value = fastPlanEntries.value.at(-1)?.id || null;
        dialogueCharacter.value.taskPlan = conversationTaskPlanEnabled(conv, fastPlanEntries.value.length);
        fastPlanPluginOptions.composerEntryId = dialogueCharacter.value.taskPlan ? activeFastPlanEntryId.value : null;
        const restoredExecutionMode = (fastPlanEntries.value.at(-1)?.data as FastPlanTimelineEntryData | undefined)?.executionMode;
        applyConversationExecutionMode(conv, restoredExecutionMode);
        if (rememberedBusy || fastPlanEntries.value.some((entry) => entry.status === 'active')) {
          setFastPlanBusyFor(id, true);
        }
        trace('view.timeline.restored', { entryCount: fastPlanEntries.value.length, taskPlan: dialogueCharacter.value.taskPlan, activeEntryId: activeFastPlanEntryId.value, busy: fastPlanBusy.value });
      }
    } else if (conv?.extra_config) {
      try {
        const cfg = JSON.parse(conv.extra_config);
        trace('view.legacy.parsed', { sessionId: cfg.fastplan_session_id });
        const hasOrdinaryMessages = chat.messages.some((m) => m.role === 'user' || m.role === 'assistant');
        if (cfg.fastplan_session_id && hasOrdinaryMessages) {
          dialogueCharacter.value.taskPlan = false;
          void clearFastPlanPersistence(id);
        } else if (cfg.fastplan_session_id) {
          dialogueCharacter.value.taskPlan = true;
          fastPlanDataObj.value = { sessionId: cfg.fastplan_session_id };
          await nextTick();
          if (!isCurrentSelection()) return;
          trace('view.legacy.process', { sessionId: cfg.fastplan_session_id, rendererMounted: !!fastPlanViewRef.value });
          fastPlanViewRef.value?.processData(cfg.fastplan_session_id);
        }
      } catch {
        trace('view.legacy.failed');
        // extra_config 格式不正确，忽略
      }
    }
    if (fastPlanEntries.value.length > 0) chatSwitchPerf.markFastPlanSnapshot(myPath);
    trace('view.load.end', { entryCount: fastPlanEntries.value.length, taskPlan: dialogueCharacter.value.taskPlan });
    if (isCurrentSelection()) tabState.hydrationRequired = false;
  } finally {
    if (sequence === conversationLoadSequence) {
      conversationRestoring.value = false;
      chatSwitchPerf.markHydrateFinished(myPath);
      scheduleChatContentPaintReport();
    }
  }
}

// 切换会话时清空输入区文档标签，文档卡片仅显示在对应消息气泡上
function restoreFilesFromMessages() {
  tabState.clearAttachments();
  fileKbIds.value = [];
  conversationFileIds.value = [];
}

// 将文档附件持久化到对话缓存（可选传入 overrideDocs 以在 fileList 已清空时仍能保存）
function persistFileCache(overrideDocs?: ChatAttachment[]) {
  const convId = chat.currentConversationId;
  if (!convId) return;
  const docs =
    overrideDocs ??
    fileList.value.filter(
      (f) => f.type === "document" && f.status === "uploaded",
    );
  if (docs.length > 0) {
    chat.setConvFileCache(convId, docs);
  }
}

// ─── 删除对话 ───
const historyConversations = computed<Conversation[]>(() => chat.conversations);
const historyActivities = useHistoryActivitySummary(() => historyConversations.value, id => ({
  streaming: chat.isConversationStreaming(id),
  phase: executionActivity.activityByConversation(id)?.phase,
  unread: chat.hasUnreadCompletion(id),
}));
async function handleEditClick(id: string) {
  const conv = historyConversations.value.find(item => item.id === id);
  if (!conv) return;
  const renameOwnerClass = `fastclaw-chat-rename-${crypto.randomUUID()}`;
  let renameInputBound = false;
  try {
    const promptPromise = ElMessageBox.prompt(
      t("chat.editConvTitle"),
      t("chat.editConversation"),
      {
        confirmButtonText: t("common.confirm"),
        cancelButtonText: t("common.cancel"),
        inputValue: conv.title || "",
        customClass: `chat-conv-message-box ${renameOwnerClass}`,
        cancelButtonClass: 'chat-conv-cancel-btn',
      },
    );
    // 打开后取消默认全选：光标放到末尾（覆盖 EP 异步 focus 全选）
    const placeCaretAtEnd = () => {
      const input = document.querySelector(
        `.${renameOwnerClass} input`,
      ) as HTMLInputElement | null;
      if (!input) return false;
      const len = input.value.length;
      try {
        input.setSelectionRange(len, len);
      } catch {
        /* ignore */
      }
      return true;
    };
    const bindOnce = () => {
      const input = document.querySelector(
        `.${renameOwnerClass} input`,
      ) as HTMLInputElement | null;
      if (!input) return false;
      if (renameInputBound) return true;
      renameInputBound = true;
      const onFocus = () => { viewEffects.frame(placeCaretAtEnd); };
      input.addEventListener('focus', onFocus);
      const unbind = () => input.removeEventListener('focus', onFocus);
      viewEffects.own(unbind);
      viewEffects.delay(unbind, 3000);
      placeCaretAtEnd();
      return true;
    };
    viewEffects.frame(() => {
      if (!bindOnce()) {
        ;[30, 80, 160].forEach((ms) => viewEffects.delay(bindOnce, ms));
      } else {
        ;[30, 80, 160].forEach((ms) => viewEffects.delay(placeCaretAtEnd, ms));
      }
    });
    const { value } = await promptPromise;
    if (value && value.trim()) {
      await chat.updateConversation(conv.id, { title: value.trim() });
    }
  } catch {
    /* cancelled */
  }
}
async function handleDeleteClick(id: string) {
  const conv = historyConversations.value.find(item => item.id === id);
  if (!conv) return;
  try {
    await ElMessageBox.confirm(
      t("chat.confirmDeleteConv"),
      t("chat.deleteConversation"),
      {
        confirmButtonText: t("common.delete"),
        cancelButtonText: t("common.cancel"),
        type: "warning",
        customClass: 'chat-conv-message-box',
        cancelButtonClass: 'chat-conv-cancel-btn',
      },
    );
    // 删除前记下占用该会话的 tab，删库后再关，避免残留打开状态
    const openTabPath = findConversationTab(conv.id);
    await chat.deleteConversation(conv.id);
    if (openTabPath) {
      await closeTab(openTabPath);
    }
  } catch {
    /* cancelled */
  }
}

// ─── 附件 / 截图（PR3 完整实现） ───
function imageClick() {
  // 所有模型都允许上传图片：多模态模型直接传图，非多模态模型走 OCR 提取文字
  contextFilePicker.open({ accept: '.jpg,.png,.jpeg,.webp,.bmp', multiple: true }, files => {
    for (const file of files) addImageAttachment(file);
  });
}
function ocrClick() {
  contextFilePicker.open({ accept: '.pdf,.doc,.docx,.txt,.md,.xls,.xlsx,.csv', multiple: true }, files => {
    for (const file of files) void uploadSingleDoc(file);
  });
}
// 当前选中模型对象（从 modelTree 中查找）
const currentModel = computed(() => {
  if (!modelId.value) return null;
  for (const g of modelTree.value) {
    const m = g.children.find((c) => c.id === modelId.value);
    if (m) return m;
  }
  return null;
});

const VISION_MODEL_KEYWORDS = [
  "vl", "vision", "gpt-4o", "gpt-4-turbo", "claude-3", "claude-3.5",
  "gemini-2", "gemini-1.5", "gemini-pro-vision", "qwen-vl", "qwen2-vl",
  "glm-4v", "cogvlm", "cogview", "internvl", "yi-vision", "llava",
  "minicpm-v", "deepseek-vl", "pixtral", "multimodal",
];
const isVisionModel = computed(() => {
  // #24 修复: 优先读 DB 能力字段，fallback 关键词匹配
  const m = currentModel.value as any
  if (m?.supportsVision === true) return true
  if (m?.supportsVision === false) return false
  const name = (m?.name || m?.rawModelId || "").toLowerCase();
  return VISION_MODEL_KEYWORDS.some(kw => name.includes(kw)) || currentModel.value?.modelLabel.includes("图片解析") || currentModel.value?.modelLabel.includes("视觉");
});

// 已知不支持深度思考的模型关键词（轻量/嵌入/重排序/图像模型）
const NON_THINKING_KEYWORDS = [
  "embedding", "bge-", "text-embedding", "rerank", "bce-",
  "dall-e", "cogview", "flux", "sd-", "stable-diffusion",
  "tts", "whisper", "baai", "gte-", "jina-",
];
const isDeepThinkModel = computed(() => {
  const name = (currentModel.value?.name || currentModel.value?.rawModelId || "").toLowerCase();
  if (!name) return false;
  // 明确不支持思考的模型
  if (NON_THINKING_KEYWORDS.some(kw => name.includes(kw))) return false;
  // 默认支持（主流 LLM 均支持 thinking 参数）
  return true;
});
function addImageAttachment(file: File) {
  const url = URL.createObjectURL(file);
  const id = crypto.randomUUID();
  fileList.value.push({
    id,
    name: file.name || `image-${Date.now()}.png`,
    type: "image",
    size: file.size,
    preview_url: url,
    status: "pending",
  });
  // 保留原始 File 引用，供 OCR 直接读取（避免 blob URL fetch 的 CSP 问题）
  tabState.retainFile(id, file, url);
  // 非多模态模型：上传后立即开始 OCR 提取文字
  if (!isVisionModel.value) {
    startOcrForImage(id);
  }
}

/** 对已添加到 fileList 的图片立即执行 OCR 文字提取 */
async function startOcrForImage(attachmentId: string) {
  await recognizeTabImage(tabState, attachmentId, recognizeImageText);
}

async function getQuickUploadUrl(): Promise<string> {
  // Tauri 环境统一走 axum 代理，与知识库 API 一致，避免跨域和端口漂移问题
  if (isTauriEnv()) {
    const port = await getHttpPort();
    return `http://127.0.0.1:${port}/api/v1/documents/quick-upload`;
  }
  return "/fastkg/api/v1/documents/quick-upload";
}


/** 上传单个文档文件，供手动选择和粘贴共用 */
async function uploadSingleDoc(file: File) {
  const conversationId = chat.currentConversationId;
  await uploadTabDocument(tabState, file, {
    async upload(source, signal) {
      // 规划模式：跳过 FastKG 上传。
      //
      // 规划链的附件是传到 FastPlan 的 /api/upload、由 FastPlan 自己读盘，从不读 FastKG ——
      // 见下面的规划分支（sendBtnFn 里 taskPlan 那段）：它只从 fileRefs 取 File，
      // 完全不碰 fileKbIds / conversationFileIds。所以这次上传纯属浪费。
      //
      // 更要紧的是它**有害**：quick-upload 是同步跑完整条解析流水线才返回的
      // （FastKG quick_upload 逐阶段执行），前端却只给了 30s 超时，大文件或需要 OCR 的
      // PDF 几乎必然触发 → 弹一个"上传失败"的红字，而规划那边其实完全不受影响。
      // 用户看到失败提示会以为传不了，实际上点了发送就能分析。
      //
      // 条件带上 useChatPlanPackages：规划界面没启用时发送会直接中止（见 sendBtnFn 开头
      // 的 warning），那种情况下文件仍应正常进知识库，不能跟着一起跳过。
      //
      // 返回「成功但无 kb/file id」而不是提前 return 整个函数：附件推进列表与
      // ★ File 引用的保留（规划模式靠它，否则 FastPlanTimelineRenderer 里
      // `if (!(file instanceof File)) continue` 会静默丢弃文档）现在都由
      // uploadTabDocument 的 owner.retainFile(id, file) 负责；这里只需让它把
      // 附件标成 uploaded 并跳过 kb/file id 登记，语义与原先直接 return 一致。
      if (dialogueCharacter.value.taskPlan && useChatPlanPackages.value) {
        return { success: true, data: { knowledge_base_id: "", file_id: "" } };
      }
      const controller = new AbortController();
      const abort = () => controller.abort();
      signal.addEventListener('abort', abort, { once: true });
      const timeout = setTimeout(abort, 30_000);
      try {
        if (signal.aborted) abort();
        const uploadUrl = await getQuickUploadUrl();
        controller.signal.throwIfAborted();
        const formData = new FormData();
        formData.append('file', source);
        const response = await fetch(uploadUrl, {
          method: 'POST', headers: FASTKG_AUTH_HEADERS, body: formData, signal: controller.signal,
        });
        if (!response.ok) throw new Error(`上传失败（HTTP ${response.status}）`);
        return await response.json();
      } finally {
        clearTimeout(timeout);
        signal.removeEventListener('abort', abort);
      }
    },
    persist(docs) {
      if (conversationId) chat.setConvFileCache(conversationId, docs);
    },
  });
}
function removeFile(idx: number) {
  const file = fileList.value[idx];
  if (!file) return;
  tabState.removeAttachment(file.id);
  persistFileCache();
}

function getFileIcon(f: ChatAttachment): string {
  const ext = f.name.split(".").pop()?.toLowerCase() || "";
  if (ext === "pdf") return pdfIcon;
  if (["doc", "docx"].includes(ext)) return wordIcon;
  if (["xls", "xlsx"].includes(ext)) return xlsxIcon;
  if (["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg"].includes(ext))
    return pngIcon;
  return txtIcon;
}

function formatFileSize(bytes?: number): string {
  if (!bytes) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
}
// Accepted upload-wait commands are owned by deferredChatSubmissionService, including off-screen Tabs.
// 待补全请求只属于当前页面上下文；切换后不得继续发送到另一个模型或会话。
watch([modelId, () => chat.currentConversationId, () => dialogueCharacter.value.taskPlan],
  () => providerCompletion.cancel(), { flush: 'sync' });
watch([messageStr, quotedText, () => fileList.value.map(file => file.id).join(',')], () => {
  if (vendorConfigPanel.value && !providerDraftTransition) providerCompletion.cancel();
}, { flush: 'sync' });
watch(vendorConfigPanel, panel => { if (panel) scrollToBottom(true); });
onDeactivated(providerCompletion.cancel);
onUnmounted(providerCompletion.cancel);

const chatInputPlaceholder = computed(() => {
  const mode = settingsStore.settings.send_shortcut || 'enter'
  const hint = mode === 'alt_s'
    ? t('settings.sendByAltS') + '，Enter 换行'
    : t('settings.sendByEnter') + '，Shift+Enter 换行'
  return t('chat.inputPlaceholder') + '（' + hint + '）'
});

const sendBtnTitle = computed(() => {
  if (pendingAutoSend.value) return '取消发送准备';
  const mode = settingsStore.settings.send_shortcut || 'enter'
  return mode === 'alt_s'
    ? t('settings.sendByAltS') + '，Enter 换行'
    : t('settings.sendByEnter') + '，Shift+Enter 换行'
});

async function resolveAttachmentDataUrl(previewUrl: string): Promise<string | null> {
  if (!previewUrl) return null;
  if (previewUrl.startsWith("data:")) return previewUrl;
  if (previewUrl.startsWith("blob:")) {
    try {
      const blob = await fetch(previewUrl).then((r) => r.blob());
      return await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onloadend = () => resolve(reader.result as string);
        reader.onerror = reject;
        reader.readAsDataURL(blob);
      });
    } catch (e) {
      console.warn("[resolveAttachmentDataUrl] failed:", e);
      return null;
    }
  }
  return previewUrl;
}


const selectedAgentId = tabState.state.selectedAgentId;
const selectedAgent = tabState.state.selectedAgent;
const isAgentConfigLocked = computed(() => !!selectedAgent.value);
const knowledgeSelectorRef = ref<KnowledgeSelectorHandle | null>(null);
const workspaceSelectorsRef = ref<KnowledgeSelectorHandle | null>(null);
const activeKnowledgeSelector = computed(() => useChatPlanPackages.value ? workspaceSelectorsRef.value : knowledgeSelectorRef.value);
useKnowledgeFileDrop({
  selectedIds: knowledgeId,
  selector: activeKnowledgeSelector,
  session: () => `${chat.currentConversationId || ''}:${dropSessionRevision.value}`,
  enabled: () => !isAgentConfigLocked.value,
});

function closeComposerTransientUi() {
  contextFilePicker.cancel();
  void inlineScreenshotEvents.value?.cancel();
  windowListVisible.value = false;
  slash.closePopup();
}
// Presentation-only cleanup also covers conversation resets and same-path owner replacement.
watch(chatContextBinding.active, closeComposerTransientUi, { flush: 'sync' });

function parseAgentOpeningText(text: string) {
  const lines = (text || "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  const greetingLines: string[] = [];
  const questions: string[] = [];

  for (const line of lines) {
    if (line.startsWith("-")) {
      const question = line.slice(1).trim();
      if (question) questions.push(question);
      continue;
    }
    greetingLines.push(line);
  }

  return {
    greeting: greetingLines.join("\n"),
    questions,
  };
}

function getAgentOpeningText(agent: Agent): string {
  if (!agent.json_data) return "";
  try {
    const config = JSON.parse(agent.json_data);
    return typeof config.opening_text === "string" ? config.opening_text.trim() : "";
  } catch {
    return "";
  }
}

function parseAgentKnowledgeIds(agent: Agent): string[] {
  if (!agent.agent_knowledgebase) return [];
  try {
    const ids = JSON.parse(agent.agent_knowledgebase);
    return Array.isArray(ids) ? ids.map((id) => String(id)).filter(Boolean) : [];
  } catch {
    return [];
  }
}

function applyAgentChatConfig(agent: Agent) {
  if (agent.agent_model) {
    modelId.value = agent.agent_model;
  }
  knowledgeId.value = parseAgentKnowledgeIds(agent);
}

function applyAgentDisplayToLoadedMessages(agent: Agent) {
  const messages = chat.messages.map((message) => {
    if (message.role !== "assistant") return message;
    return {
      ...message,
      agent_name: agent.agent_name,
      agent_icon: agent.agent_icon || agentDefaultIcon,
    };
  });
  chat.messages.splice(0, chat.messages.length, ...messages);
}

async function restoreAgentConversation(conv: Conversation | undefined, isCurrent: () => boolean = () => !viewDisposed && !tabState.disposed) {
  if (!conv?.assistant_id) return;

  const agent = await agentStore.getAgent(conv.assistant_id);
  if (!agent || !isCurrent()) return;

  selectedAgent.value = agent;
  selectedAgentId.value = agent.id;
  applyAgentChatConfig(agent);
  prependAgentOpeningMessage(agent);
  applyAgentDisplayToLoadedMessages(agent);
}

function escapeAgentOpeningHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function buildAgentOpeningMessage(agent: Agent): string {
  const opening = parseAgentOpeningText(getAgentOpeningText(agent));
  const parts: string[] = [];

  if (opening.greeting) {
    parts.push(opening.greeting);
  }

  if (opening.questions.length > 0) {
    const suggestions = opening.questions
      .map((q) => `<div class="agent-opening-suggestion">
  <span class="agent-opening-suggestion-icon"></span>
  <span class="agent-opening-suggestion-text">${escapeAgentOpeningHtml(q)}</span>
</div>`)
      .join("");
    parts.push(`<div class="agent-opening-suggestions">
  <div class="agent-opening-suggestions-list">${suggestions}</div>
</div>`);
  }

  return parts.join("\n\n").trim();
}

function createAgentOpeningMessage(agent: Agent): Message | null {
  const content = buildAgentOpeningMessage(agent);
  if (!content || !chat.currentConversationId) return null;

  return {
    id: crypto.randomUUID(),
    conversation_id: chat.currentConversationId,
    role: "assistant",
    content,
    model_id: agent.agent_model || modelId.value || null,
    token_input: null,
    token_output: null,
    status: "completed",
    created_at: new Date().toISOString(),
    group_id: crypto.randomUUID(),
    agent_name: agent.agent_name,
    agent_icon: agent.agent_icon || agentDefaultIcon,
  };
}

function prependAgentOpeningMessage(agent: Agent) {
  const openingMessage = createAgentOpeningMessage(agent);
  if (!openingMessage) return;
  chat.messages.unshift(openingMessage);
}

function toggleWindowList(anchor?: HTMLElement | MouseEvent) {
  windowListVisible.value = !windowListVisible.value;
  const group = anchor instanceof HTMLElement ? anchor : screenshotGroupRef.value;
  if (windowListVisible.value && group) {
    const rect = group.getBoundingClientRect();
    popoverStyle.value = {
      bottom: (window.innerHeight - rect.top + 4) + 'px',
      left: rect.left + 'px',
    };
  }
}

/**
 * 启动独立遮罩窗口截图（主窗口全程不动，消除窗口抖动/闪烁）。
 * 固定请求身份；页面失效不阻止主窗口恢复，遮罩不可用时使用主窗口回退。
 */
async function handleNativeScreenshot(hideSelf: boolean) {
  const lease = chatContextBinding.captureUi();
  if (!isTauriEnv() || !lease?.isCurrent()) return;
  let invoke: Awaited<ReturnType<typeof getInvoke>> = null;
  const result = await startNativeScreenshot({
    isCurrent: lease.isCurrent,
    async prepareWindow() {
      invoke = await getInvoke();
      if (!invoke) throw new Error('Screenshot IPC unavailable');
      const { ensureScreenshotWindow } = await import(/* @vite-ignore */ "@/utils/screenshotWindow");
      return (await ensureScreenshotWindow()).ok;
    },
    async capture(requestId, inline) {
      return invoke!<CaptureMetadata>('capture_start', {
        requestId, presentation: inline ? 'inline' : 'window',
        hideSelf, restoreSelf: inline,
      });
    },
    async runWindow(requestId) {
      const { emit } = await import('@tauri-apps/api/event');
      await emit('screenshot-run', requestId);
    },
    async showInline(metadata, confirm, cancel) {
      const monitorImages = await invoke!<string[]>('capture_get_all_monitor_pixels');
      if (!lease.isCurrent()) return false;
      inlineScreenshotEvents.value = { confirm, cancel };
      captureResult.value = { ...metadata, monitorImages };
      return true;
    },
    confirm: onScreenshotConfirm,
    async cleanup(requestId) {
      if (captureResult.value?.request_id === requestId) {
        captureResult.value = null;
        inlineScreenshotEvents.value = null;
      }
      try { await invoke?.('capture_discard_pending', { requestId }); }
      catch (error) { console.warn('Discard screenshot data failed', error); }
      finally { await restoreMainWindow(); }
    },
    onError(error) {
      console.warn('Native screenshot failed', error);
      ElMessage.error(t('chat.screenshotFailed'));
    },
  });
  if (result === 'busy' && lease.isCurrent()) ElMessage.info('请先完成或取消当前截图');
}

async function screenshotHideSelf() {
  windowListVisible.value = false;
  await handleNativeScreenshot(true);
}

function onDocumentClick() {
  windowListVisible.value = false;
}

async function handleScreenshot() {
  const lease = chatContextBinding.captureUi();
  if (!lease?.isCurrent()) return;
  if (isTauriEnv()) {
    await handleNativeScreenshot(false);
  } else {
    await captureBrowserScreenshot({
      isCurrent: lease.isCurrent,
      capture: () => navigator.mediaDevices.getDisplayMedia({ video: true }),
      grabFrame: track => new (window as any).ImageCapture(track).grabFrame(),
      encode: bitmap => {
        const canvas = document.createElement('canvas');
        canvas.width = bitmap.width;
        canvas.height = bitmap.height;
        const ctx = canvas.getContext('2d');
        if (!ctx) throw new Error('Screenshot canvas is unavailable');
        ctx.drawImage(bitmap, 0, 0);
        return canvas.toDataURL('image/png');
      },
      accept: dataUrl => lease.context.owner.state.fileList.value.push({
        id: crypto.randomUUID(), name: `screenshot-${Date.now()}.png`,
        type: 'screenshot', preview_url: dataUrl, status: 'pending',
      }),
      onError: error => {
        console.warn('Web screenshot failed', error);
        ElMessage.warning(t('chat.screenshotCancelled'));
      },
    });
  }
}
/**
 * 恢复主窗口显示。用于"隐藏窗口截图"独立遮罩路径：主窗口在截图时被隐藏且刻意未恢复，
 * 需在遮罩关闭（确认/取消）后由此恢复。主窗口本就可见时该调用为幂等无副作用。
 */
async function restoreMainWindow() {
  if (!isTauriEnv()) return;
  await getInvoke()
    .then(async (invoke) => {
      if (!invoke) return;
      try {
        // 优先通过后端命令恢复主窗口，避免前端 getCurrentWindow() 在隐藏后状态不一致
        await invoke('show_main_window');
      } catch {
        // 后端命令不可用时回退到前端 API
        try {
          const { getCurrentWindow } = await import("@tauri-apps/api/window");
          const w = getCurrentWindow();
          await w.show();
          await w.setFocus();
        } catch { /* ignore */ }
      }
    })
    .catch(() => { /* ignore */ });
}

function onScreenshotConfirm(dataUrl: string) {
  // 所有模型都允许截图：多模态模型直接传图，非多模态模型走 OCR 提取文字
  const id = crypto.randomUUID();
  fileList.value.push({
    id,
    name: `screenshot-${Date.now()}.png`,
    type: "screenshot",
    preview_url: dataUrl,
    status: "pending",
  });
  // 非多模态模型：截图后立即开始 OCR
  if (!isVisionModel.value) {
    startOcrForImage(id);
  }
}

// ─── FastPlan 子系统 ───
const fastPlanViewRef = ref<InstanceType<typeof FastPlanView> | null>(null);
const fastPlanDataObj = tabState.state.fastPlanDataObj;


// 合并写入当前会话的 extra_config（保留已有字段，如 fastplan_session_id / mcp）
async function persistConvExtraConfig(patch: Record<string, unknown>) {
  const convId = chat.currentConversationId;
  if (!convId) return;
  const conv = chat.conversations.find((c) => c.id === convId);
  let cfg: Record<string, unknown> = {};
  if (conv?.extra_config) {
    try {
      cfg = JSON.parse(conv.extra_config) || {};
    } catch {
      cfg = {};
    }
  }
  const extraConfig = JSON.stringify({ ...cfg, ...patch });
  if (conv) conv.extra_config = extraConfig;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("update_conversation_extra_config", {
      id: convId,
      extraConfig,
    });
  } catch (e) {
    console.warn("update_conversation_extra_config failed", e);
  }
}

async function onFastPlanSave(sessionId: string, _prompt: string, targetConversationId?: string | null) {
  fastPlanDataObj.value = { sessionId };
  await persistFastPlanState(targetConversationId);
}

/** 普通会话被误写入规划数据时，从 extra_config 剥离 fastplan 字段 */
async function clearFastPlanPersistence(conversationId: string) {
  const conv = chat.conversations.find((item) => item.id === conversationId);
  if (!conv?.extra_config) return;
  let cfg: Record<string, unknown>;
  try {
    cfg = JSON.parse(conv.extra_config);
  } catch {
    return;
  }
  if (!cfg || typeof cfg !== 'object') return;
  if (!('fastplan' in cfg) && !('fastplan_session_id' in cfg)) return;
  const { fastplan: _fp, fastplan_session_id: _sid, ...rest } = cfg as Record<string, unknown>;
  const extraConfig = JSON.stringify(rest);
  conv.extra_config = extraConfig;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('update_conversation_extra_config', { id: conversationId, extraConfig });
  } catch (error) {
    console.warn('clear FastPlan persistence failed', error);
  }
}

async function persistFastPlanState(
  targetConversationId?: string | null,
  options?: {
    entries?: ChatEmbeddedTimelineEntry[];
    activeEntryId?: string | null;
    allowNonCurrent?: boolean;
    requireSuccess?: boolean;
  },
) {
  const convId = targetConversationId ?? chat.currentConversationId;
  if (!convId || (!options?.allowNonCurrent && convId !== chat.currentConversationId)) return;
  const entries = options?.entries ?? fastPlanEntries.value;
  const activeId = options && 'activeEntryId' in options ? options.activeEntryId ?? null : activeFastPlanEntryId.value;
  const conv = chat.conversations.find((item) => item.id === convId);
  if (!entries.some(entry => (entry.data as FastPlanTimelineEntryData)?.sessionId)) {
    try { if (conv?.extra_config && JSON.parse(conv.extra_config).fastplan_remote === true) return; } catch { /* legacy config */ }
  }
  try {
    await persistManagedFastPlanState(tabState, chat as unknown as PlanningConversationStore, convId, entries, activeId);
    fastPlanWriterConversationId = convId;
  } catch (error) {
    console.warn('persist FastPlan entries failed', error);
    if (options?.requireSuccess) throw error;
  }
}

function parseFastPlanExecutionMode(raw: unknown): FastPlanExecutionMode {
  return raw === 'manual' || raw === 'step' ? raw : 'auto';
}

function conversationExecutionMode(
  conv: { extra_config?: string | null } | null | undefined,
): FastPlanExecutionMode {
  try {
    const cfg = JSON.parse(conv?.extra_config || '{}') as Record<string, unknown>;
    return parseFastPlanExecutionMode(cfg.execution_mode ?? cfg.executionMode);
  } catch {
    return 'auto';
  }
}

function applyConversationExecutionMode(
  conv: { extra_config?: string | null } | null | undefined,
  fallback?: unknown,
) {
  const fromExtra = (() => {
    try {
      const cfg = JSON.parse(conv?.extra_config || '{}') as Record<string, unknown>;
      return cfg.execution_mode != null || cfg.executionMode != null;
    } catch {
      return false;
    }
  })();
  const mode = fromExtra ? conversationExecutionMode(conv) : parseFastPlanExecutionMode(fallback);
  if (fastPlanExecutionMode.value === mode) return;
  applyingRemoteExecutionMode = true;
  fastPlanExecutionMode.value = mode;
  applyingRemoteExecutionMode = false;
}

async function persistConversationExecutionMode(mode: FastPlanExecutionMode) {
  const convId = chat.currentConversationId;
  if (!convId || conversationRestoring.value) return;
  const conv = chat.conversations.find((item) => item.id === convId);
  if (!conv) return;
  if (conversationExecutionMode(conv) === mode) return;
  let root: Record<string, unknown> = {};
  try {
    const parsed = conv.extra_config ? JSON.parse(conv.extra_config) : {};
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) root = parsed;
  } catch {
    root = {};
  }
  conv.extra_config = JSON.stringify({ ...root, execution_mode: mode });
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('set_conversation_execution_mode', { sessionId: convId, mode });
  } catch (error) {
    console.warn('persist execution mode failed', error);
  }
}

function syncRemoteFastPlanFromConversation() {
  if (!useChatPlanPackages.value) return;
  if (tabState.state.planningPreparationId.value) return;
  const convId = chat.currentConversationId;
  if (!convId) return;
  const conv = chat.conversations.find((item) => item.id === convId);
  const enabled = conversationTaskPlanEnabled(conv, fastPlanEntries.value.length);
  applyConversationExecutionMode(conv, (fastPlanEntries.value.at(-1)?.data as FastPlanTimelineEntryData | undefined)?.executionMode);
  // App 开关只改 extra_config.task_plan；工具栏按钮跟这个标志走，不要等规划条目/空会话才点亮
  dialogueCharacter.value.taskPlan = enabled;

  let remote = false;
  try {
    remote = !!(conv?.extra_config && JSON.parse(conv.extra_config).fastplan_remote === true);
  } catch {
    remote = false;
  }
  if (!remote) {
    if (!enabled) fastPlanPluginOptions.composerEntryId = null;
    return;
  }
  const persisted = readFastPlanPersistence(conv?.extra_config);
  if (!persisted.entries.length) {
    if (!enabled) fastPlanPluginOptions.composerEntryId = null;
    return;
  }
  const timeline = deferCompletedFastPlanHistory(persistedEntriesToTimeline(persisted).map((entry) => {
    const data = (entry.data || {}) as FastPlanTimelineEntryData;
    return {
      ...entry,
      data: {
        ...data,
        confirmationPolicy: 'strict',
        executionMode: data.executionMode || fastPlanExecutionMode.value || 'auto',
        model: data.model || selectedFastPlanModel(),
      } satisfies FastPlanTimelineEntryData,
    };
  }));
  const nextKey = timeline
    .map((entry) => {
      const data = entry.data as FastPlanTimelineEntryData;
      return `${entry.id}:${data.sessionId}:${data.connectionMode}:${entry.status}`;
    })
    .join('|');
  const prevKey = fastPlanEntries.value
    .map((entry) => {
      const data = entry.data as FastPlanTimelineEntryData;
      return `${entry.id}:${data.sessionId}:${data.connectionMode}:${entry.status}`;
    })
    .join('|');
  const hasOrdinaryMessages = chat.messages.some((m) => m.role === 'user' || m.role === 'assistant');
  // 已有普通消息时只同步按钮，不把时间线切成本地 FastPlan 条目
  if (hasOrdinaryMessages) {
    fastPlanPluginOptions.composerEntryId = null;
    return;
  }
  if (nextKey === prevKey && enabled) return;
  fastPlanEntries.value = timeline;
  activeFastPlanEntryId.value = timeline.at(-1)?.id || persisted.activeEntryId || null;
  fastPlanPluginOptions.composerEntryId = enabled ? activeFastPlanEntryId.value : null;
}

watch(
  () => [
    chat.currentConversationId,
    chat.conversations.find((item) => item.id === chat.currentConversationId)?.extra_config || '',
  ],
  () => syncRemoteFastPlanFromConversation(),
);

const fastPlanStore = useFastPlanStore();
const fastPlanHasContent = computed(() =>
  dialogueCharacter.value.taskPlan && (
    fastPlanStore.messages.length > 0 ||
    Object.keys(fastPlanStore.executionState).length > 0 ||
    fastPlanStore.isPreviewing
  )
);

const remotePlanControls = createRemotePlanConfirmationBinding(chatContextBinding, context => {
  const sid = context.store.currentConversationId;
  if (!sid) return null;
  const live = remotePlanLiveBySession[sid];
  if (live) return live;
  const msgs = context.store.messages;
  for (let i = msgs.length - 1; i >= 0; i -= 1) {
    if (msgs[i]?.conversation_id === sid && msgs[i]?.planRuntime) return msgs[i].planRuntime as Record<string, any>;
  }
  return null;
}, (sessionId, action, expectedWaitId) => invoke('send_conversation_plan_control', { sessionId, action, expectedWaitId }),
error => ElMessage.error(error instanceof Error ? error.message : '规划确认失败'));
const remotePlanPanel = remotePlanControls.panel;

const remoteComposerPlan = computed(() => {
  const runtime = remotePlanPanel.value?.runtime;
  if (!runtime) return null;
  return adaptFastPlanTaskPlanExecution(runtime) || { status: 'planning' as const, phases: [] };
});

// ─── 思考 / 任务规划 / 联网开关 ───
function thinkClick() {
  if (!isDeepThinkModel.value) {
    ElMessage.warning(t("chat.deepThinkNotSupported"));
    return;
  }
  dialogueCharacter.value.think = !dialogueCharacter.value.think;
  void syncRemoteUiThinking(dialogueCharacter.value.think);
}
async function detachOrdinaryConversationForFastPlan(): Promise<boolean> {
  const previousConversationId = chat.currentConversationId;
  if (!previousConversationId) return true;

  // Persist conversation-scoped host state before changing the selected id.
  persistFileCache();
  saveDocEditState();
  if (inConversation.value) await chat.stopStream(previousConversationId);
  // A history click can win while stopStream is awaiting IPC. Never detach a
  // newer selection in that case.
  if (!chat.detachCurrentConversation(previousConversationId)) return false;

  if (fastPlanWriterConversationId) {
    releaseFastPlanWriter(fastPlanWriterConversationId, planWriterId);
    fastPlanWriterConversationId = null;
  }
  fastPlanStore.clearCurrentSession();
  fastPlanDataObj.value = null;
  fastPlanEntries.value = [];
  activeFastPlanEntryId.value = null;
  fastPlanPluginOptions.composerEntryId = null;
  setFastPlanBusyFor(previousConversationId, false);
  clearQuote();
  fileKbIds.value = [];
  conversationFileIds.value = [];
  rightPanel.value = 'none';
  currentWebSearchMessageId.value = null;
  loadDocEditState('');
  appStore.setTabTitle(myPath, 'sidebar.smartChat');
  return true;
}

function conversationTaskPlanEnabled(
  conv: { mode?: string; extra_config?: string | null } | null | undefined,
  entryCount = 0,
) {
  try {
    const cfg = JSON.parse(conv?.extra_config || '{}') as Record<string, unknown>;
    if (cfg.task_plan === false || cfg.taskPlan === false) return false;
    if (cfg.task_plan === true || cfg.taskPlan === true) return true;
  } catch {
    /* 无 extra_config 时走 mode / 条目兜底 */
  }
  if (String(conv?.mode || '').toLowerCase() === 'planning') return true;
  return entryCount > 0;
}

function patchConversationTaskPlanFlag(conversationId: string, enabled: boolean) {
  const conv = chat.conversations.find((item) => item.id === conversationId);
  if (!conv) return;
  try {
    const root = conv.extra_config ? JSON.parse(conv.extra_config) : {};
    conv.extra_config = JSON.stringify({ ...root, task_plan: enabled });
  } catch {
    conv.extra_config = JSON.stringify({ task_plan: enabled });
  }
}

async function taskPlanClick() {
  if (taskPlanSwitching.value) return;
  const enabling = !dialogueCharacter.value.taskPlan;
  taskPlanSwitching.value = true;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    if (enabling) {
      const result = await invoke<{
        session_id?: string
        sessionId?: string
        detachedFrom?: string | null
        title?: string
        taskPlan?: boolean
      }>('set_conversation_task_plan', {
        sessionId: chat.currentConversationId,
        enabled: true,
        modelId: modelId.value || null,
      });
      const newSid = String(result?.session_id || result?.sessionId || '').trim();
      if (!newSid) throw new Error('打开规划会话失败');
      dialogueCharacter.value.taskPlan = true;
      if (newSid !== chat.currentConversationId) {
        await chat.loadConversations(true);
        await selectConversation(newSid, { hydrate: true });
      } else {
        patchConversationTaskPlanFlag(newSid, true);
      }
      // 本地 FastPlan：保留/初始化时间线条目
      const activeEntryExists = fastPlanEntries.value.some((entry) => entry.id === activeFastPlanEntryId.value);
      if (!activeEntryExists) activeFastPlanEntryId.value = fastPlanEntries.value.at(-1)?.id || null;
      if (!fastPlanEntries.value.length) setFastPlanBusyFor(newSid, false);
      fastPlanPluginOptions.composerEntryId = activeFastPlanEntryId.value;
    } else {
      if (chat.currentConversationId) {
        await invoke('set_conversation_task_plan', {
          sessionId: chat.currentConversationId,
          enabled: false,
          modelId: modelId.value || null,
        });
      }
      dialogueCharacter.value.taskPlan = false;
      if (chat.currentConversationId) patchConversationTaskPlanFlag(chat.currentConversationId, false);
      if (useChatPlanPackages.value) {
        fastPlanPluginOptions.composerEntryId = null;
      }
    }
    console.log('[ChatView] taskPlan toggled:', dialogueCharacter.value.taskPlan);
  } catch (error) {
    const message = error instanceof Error && error.message
      ? error.message
      : String(error || '切换任务规划失败');
    ElMessage.error(message);
  } finally {
    taskPlanSwitching.value = false;
  }
}
function webSearchClick() {
  dialogueCharacter.value.webSearch = !dialogueCharacter.value.webSearch;
}

// ─── 清除上下文 / 清空对话 ───
async function deleteAllText() {
  await chat.clearContext();
  scrollToBottomAndEngage();
}
async function clearChat() {
  await chat.clearMessages();
}

// ─── 文档附件卡片辅助 ───
function getDocAttachments(msg: Message): ChatAttachment[] {
  if (!msg.attachments) return [];
  return msg.attachments.filter(
    (a) => a.type === "document" && a.status === "uploaded",
  );
}
function handleDocQuote(doc: ChatAttachment) {
  messagePresentationActions.value?.quoteDocument(doc);
}
function truncateFileName(name: string, maxLen: number): string {
  if (name.length <= maxLen) return name;
  // 保留扩展名关键部分：显示前 maxLen-1 字符 + …
  return name.substring(0, maxLen - 1) + "…";
}
// ─── 消息工具栏：复制/编辑/删除/翻译/重新生成/切换模型/分支/点赞/溯源 ───
async function handleRegenerate(id: string) {
  const source = answerSource(answerChat.messages, id);
  if (!source) return;
  const target = captureAnswerTarget(tabState, answerChat, source.groupId, source.content);
  const selectedModelId = modelId.value;
  const providerCode = getProviderInfoForModel(selectedModelId)?.providerCode;
  const valid = () => target.isCurrent() && !viewDisposed && modelId.value === selectedModelId && !answerChat.isLoading;
  if (!valid()) return;
  if (!(await ensureModelProviderReady(selectedModelId)) || !valid()) return;
  void answerChat.regenerateMessage(id, selectedModelId, providerCode);
}

const deletionActions = createMessageDeletionBinding(chatContextBinding, (kind, error) => {
  const detail = error instanceof Error ? error.message : String(error);
  if (kind === 'busy' || kind === 'unavailable') ElMessage.info(detail);
  else ElMessage.error(detail);
});
function handleDeleteMessage(id: string) { return deletionActions.value?.remove(id); }
function handleEditMessage(id: string) {
  const lease = chatContextBinding.captureUi();
  if (!lease?.isCurrent() || lease.context !== initialChatContext) return;
  if (inConversation.value) {
    ElMessage.warning(t('docGen.waitGenComplete'));
    return;
  }
  const msg = chat.messages.find((m) => m.id === id);
  if (!msg) return;
  if (msg.role === "user") {
    inlineQuestion.open(id);
    return;
  }
  messageEditor.open(id);
}
let resendSequence = 0;
async function resendUserQuestion(content: string, requestIsCurrent: () => boolean = () => true, onDispatch?: () => void) {
  const lease = chatContextBinding.captureUi();
  if (!lease?.isCurrent() || lease.context !== initialChatContext || !requestIsCurrent() || !content.trim()) return;
  const state = lease.context.owner.state;
  if (state.pendingAutoSend.value || state.sendBtnLock.value) return;
  if (inConversation.value || (dialogueCharacter.value.taskPlan && fastPlanBusy.value)) {
    ElMessage.warning(t('docGen.waitGenComplete'));
    return;
  }
  const sequence = ++resendSequence;
  state.messageStr.value = content;
  // 与输入框发送一致：即使当前停在历史问题上方，也强制滚到最新一轮并跟随流式输出
  scrollToBottomAndEngage();
  await submitDraftAfterRender(lease, () => sequence === resendSequence && requestIsCurrent(), onDispatch);
}

/** Rendering is a pre-submit UI wait, not permission to send a later draft/context. */
async function submitDraftAfterRender(
  lease = chatContextBinding.captureUi(), requestIsCurrent: () => boolean = () => true, onDispatch?: () => void,
) {
  if (!lease?.isCurrent() || lease.context !== initialChatContext) return;
  const command = captureChatCommand(lease.context);
  const state = lease.context.owner.state;
  const snapshot = () => JSON.stringify([
    state.messageStr.value, state.quotedText.value, state.fileList.value.map(file => file.id),
    state.modelId.value, state.dialogueCharacter.value, state.editorVisible.value,
    state.knowledgeId.value, state.selectedSkill.value?.name, state.selectedAgentId.value,
    state.fastPlanExecutionMode.value,
  ]);
  const intent = snapshot();
  await nextTick();
  if (!lease.isCurrent() || !command.isCurrent() || !requestIsCurrent() || viewDisposed
    || state.pendingAutoSend.value || state.sendBtnLock.value
    || snapshot() !== intent || inConversation.value
    || (dialogueCharacter.value.taskPlan && fastPlanBusy.value)) return;
  // Legacy dispatcher is still fixed to initialChatContext. Do not route another owner into it.
  onDispatch?.();
  await sendBtnFn();
}

const messageActions = createMessageActionBinding(chatContextBinding, {
  getInvoke,
  report: (kind, error) => {
    if (kind === 'favorite') ElMessage.error(t('chat.favoriteFailed'));
    else if (kind === 'unavailable') ElMessage.info('翻译功能需要 Tauri 后端支持');
    else if (kind === 'translate') {
      const detail = error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
      ElMessage.error(`翻译失败: ${detail}`);
    }
    // 赞/踩保持原来的静默失败策略。
  },
});
function handleLike(id: string, next: number) { return messageActions.value?.like(id, next); }
function handleDislike(id: string, next: number) { return messageActions.value?.dislike(id, next); }

// 多模型切换
/** 最近一次指针落点，用于把下拉锚到真实点击的 jcr-action（自定义图标无 data-action-icon） */
let lastPointerEl: Element | null = null;
function onGlobalPointerDownCapture(event: PointerEvent) {
  lastPointerEl = event.target instanceof Element ? event.target : null;
}
// 指针锚点监听由当前视图激活范围管理，隐藏实例不保留其他 Tab 的 DOM。

function findSwitchModelActionEl(messageId: string): HTMLElement | null {
  try {
    // 1) 优先用刚点击的按钮（最可靠）
    if (lastPointerEl) {
      const clickedBtn = lastPointerEl.closest(
        ".jcr-action, .jcr-more__item",
      ) as HTMLElement | null;
      const clickedMsg = lastPointerEl.closest(
        "[data-message-id]",
      ) as HTMLElement | null;
      if (
        clickedBtn &&
        clickedMsg?.getAttribute("data-message-id") === messageId
      ) {
        return clickedBtn;
      }
    }

    const msgEl = document.querySelector(
      `[data-message-id="${CSS.escape(messageId)}"]`,
    );
    if (!msgEl) return null;

    // 2) data-action-icon（包内 ChatActionIcon / 宿主 FastClawMessageActionIcon）
    const icon = msgEl.querySelector('[data-action-icon="switch-model"]');
    const byIcon = icon?.closest(".jcr-action, .jcr-more__item") as
      | HTMLElement
      | null;
    if (byIcon) return byIcon;

    // 3) title / aria-label 文案兜底
    const label = t("chat.messageActions.switchModel");
    const labeled = Array.from(
      msgEl.querySelectorAll<HTMLElement>(".jcr-action, .jcr-more__item"),
    ).find(
      (el) =>
        el.getAttribute("title") === label ||
        el.getAttribute("aria-label") === label,
    );
    return labeled || null;
  } catch {
    return null;
  }
}

function closeSwitchModelAnchor() {
  switchModelAnchorOpening = false;
  switchModelAnchorActive.value = false;
  switchModelAnchorMessageId.value = null;
  switchModelAnchorStyle.value = {};
  switchModelSelectedId.value = undefined;
}

function openSwitchModelDropdown(
  selectorRef: typeof switchModelAnchorSelectorRef | typeof switchModelSelectorRef,
) {
  switchModelAnchorOpening = true;
  // 双 nextTick：确保 v-if 挂载后再 toggle，避免首帧 ref 为空又回退弹窗
  viewEffects.tick(() => {
    viewEffects.tick(() => {
      const elSelect = selectorRef.value?.modelSelector as {
        toggleMenu?: () => void;
        focus?: () => void;
        $el?: HTMLElement;
      } | null | undefined;
      if (!elSelect) {
        switchModelAnchorOpening = false;
        return;
      }
      if (typeof elSelect.toggleMenu === "function") {
        elSelect.toggleMenu();
        viewEffects.tick(() => {
          switchModelAnchorOpening = false;
        });
        return;
      }
      elSelect.focus?.();
      viewEffects.tick(() => {
        const el = elSelect.$el ?? null;
        const wrapper = el?.querySelector(
          ".el-input__wrapper, .el-select__wrapper",
        ) as HTMLElement | null;
        (wrapper || el)?.click();
        switchModelAnchorOpening = false;
      });
    });
  });
}

function openSwitchModelNearAction(messageId: string, groupId: string, content: string) {
  let btn = findSwitchModelActionEl(messageId);
  let rect = btn?.getBoundingClientRect();
  // 「更多」菜单关闭后按钮可能已不可见，改锚到「更多」按钮或回退弹窗
  if (!btn || !rect || rect.width < 1 || rect.height < 1) {
    try {
      const msgEl = document.querySelector(
        `[data-message-id="${CSS.escape(messageId)}"]`,
      );
      const moreBtn = msgEl?.querySelector(
        ".jcr-more > summary.jcr-action",
      ) as HTMLElement | null;
      const moreRect = moreBtn?.getBoundingClientRect();
      if (moreBtn && moreRect && moreRect.width >= 1 && moreRect.height >= 1) {
        btn = moreBtn;
        rect = moreRect;
      }
    } catch {
      /* ignore */
    }
  }
  if (!btn || !rect || rect.width < 1 || rect.height < 1) {
    closeSwitchModelAnchor();
    switchModelTarget.value = captureAnswerTarget(tabState, answerChat, groupId, content);
    switchModelPickerVisible.value = true;
    return;
  }
  switchModelPickerVisible.value = false;
  switchModelTarget.value = captureAnswerTarget(tabState, answerChat, groupId, content);
  switchModelSelectedId.value = undefined;
  switchModelAnchorMessageId.value = messageId;

  // 按上下剩余空间决定展开方向；锚点放在按钮外侧，避免下拉遮挡 jcr-action
  const gap = 6;
  const panelGuess = 320;
  const spaceAbove = rect.top;
  const spaceBelow = window.innerHeight - rect.bottom;
  const preferBottom = spaceBelow >= panelGuess || spaceBelow >= spaceAbove;
  switchModelPlacement.value = preferBottom ? "bottom-start" : "top-start";
  switchModelFallbackPlacements.value = preferBottom
    ? ["bottom-start", "top-start", "bottom", "top"]
    : ["top-start", "bottom-start", "top", "bottom"];
  const anchorTop = preferBottom ? rect.bottom + gap : Math.max(0, rect.top - gap);
  switchModelAnchorStyle.value = {
    top: `${anchorTop}px`,
    left: `${rect.left}px`,
    width: `${Math.max(rect.width, 28)}px`,
    height: "1px",
  };

  const mountAndOpen = () => {
    switchModelAnchorActive.value = true;
    openSwitchModelDropdown(switchModelAnchorSelectorRef);
  };
  // 已有锚点时先卸载再挂载，确保下拉挂到新按钮位置
  if (switchModelAnchorActive.value) {
    switchModelAnchorActive.value = false;
    viewEffects.tick(mountAndOpen);
    return;
  }
  mountAndOpen();
}

async function handleSwitchModel(groupId: string, messageId?: string) {
  const userMsg = chat.messages.find(
    (m) => m.group_id === groupId && m.role === "user",
  );
  if (!userMsg) return;
  if (messageId) {
    openSwitchModelNearAction(messageId, groupId, userMsg.content);
    return;
  }
  switchModelTarget.value = captureAnswerTarget(tabState, answerChat, groupId, userMsg.content);
  switchModelPickerVisible.value = true;
}
function onSingleMsgSwitchModel(msg: Message, rendererMessageId?: string) {
  const msgs = chat.messages;
  const idx = msgs.indexOf(msg);
  const userMsg = msgs
    .slice(0, idx)
    .reverse()
    .find((m) => m.role === "user");
  if (!userMsg) return;
  let gid = msg.group_id || userMsg.group_id;
  if (!gid) {
    gid = crypto.randomUUID();
    userMsg.group_id = gid;
    msg.group_id = gid;
  }
  const anchorMessageId = rendererMessageId || msg.id;
  // 同一条消息再次点击：关闭锚点下拉（等价于开关）
  if (
    switchModelAnchorActive.value &&
    switchModelAnchorMessageId.value === anchorMessageId
  ) {
    const elSelect = switchModelAnchorSelectorRef.value?.modelSelector as
      | { blur?: () => void }
      | null
      | undefined;
    elSelect?.blur?.();
    closeSwitchModelAnchor();
    return;
  }
  handleSwitchModel(gid, anchorMessageId);
}
async function onSwitchModelChange(messageId: string, targetModelId: string) {
  const source = answerSource(answerChat.messages, messageId);
  if (!source) return;
  await submitModelAnswer(captureAnswerTarget(tabState, answerChat, source.groupId, source.content), targetModelId);
}
async function submitModelAnswer(target: ReturnType<typeof captureAnswerTarget>, targetModelId: string) {
  const valid = () => target.isCurrent() && !viewDisposed;
  const providerCode = getProviderInfoForModel(targetModelId)?.providerCode;
  if (!valid() || !targetModelId) return;
  if (!(await ensureModelProviderReady(targetModelId)) || !valid()) return;
  if (answerChat.isLoading) {
    try {
      await ElMessageBox.confirm(t('chat.modelSwitchBody'), '提示',
        { confirmButtonText: '切换并重新生成', cancelButtonText: '取消', type: 'warning' });
    } catch { return; }
    if (!valid()) return;
    await answerChat.stopStream(target.conversationId || undefined);
    if (!valid() || answerChat.isLoading) return;
  }
  if (!valid()) return;
  void answerChat.addModelAnswer(target.groupId, target.content, targetModelId, providerCode);
}
async function confirmSwitchModel(targetModelId: string) {
  const target = switchModelTarget.value;
  switchModelPickerVisible.value = false;
  closeSwitchModelAnchor();
  switchModelTarget.value = null;
  if (target) await submitModelAnswer(target, targetModelId);
}

function onSwitchModelSelected(id: string | undefined) {
  if (!id) return;
  confirmSwitchModel(id);
  switchModelSelectedId.value = undefined;
}

function onSwitchModelAnchorVisibleChange(visible: boolean) {
  if (visible) return;
  if (switchModelAnchorOpening) return;
  closeSwitchModelAnchor();
}

watch(switchModelPickerVisible, (visible) => {
  if (visible) {
    closeSwitchModelAnchor();
    switchModelSelectedId.value = undefined;
    viewEffects.tick(() => {
      switchModelSelectorRef.value?.focus();
    });
  }
});

// 分支
const branchActions = createConversationBranchBinding(chatContextBinding, {
  open: id => navigateToConversation(router, id),
  report: (kind, error) => {
    if (kind === 'created') ElMessage.success(t('chat.branchCreated'));
    else if (kind === 'unavailable') ElMessage.info('对话分支功能需要 Tauri 后端支持');
    else ElMessage.error(error instanceof Error ? error.message : String(error));
  },
});
function handleBranch(messageId: string) { return branchActions.value?.branch(messageId); }

// 消息导出快照与保存前的展示归属。
const messageExport = createMessageExportBinding(chatContextBinding, {
  run: async (content, format, control) => {
    if (format === 'md') return exportMarkdown(content, control);
    if (format === 'docx') return exportWord(content, renderMd, control);
    const result = await exportPdf(content, renderMd, undefined, control);
    if (!result.success && !result.canceled) throw new Error(result.error || 'Unknown error');
    return result.success;
  },
  loading: () => {
    const message = ElMessage.info({ message: t('chat.exportGenerating'), duration: 0 });
    return () => message.close();
  },
  report: (kind, format, error) => {
    if (kind === 'empty') ElMessage.warning(t('chat.emptyContentCannotExport'));
    else if (kind === 'streaming') ElMessage.warning(t('chat.streamingCannotExport'));
    else if (kind === 'success') ElMessage.success(t('chat.exportSuccess', { format: format === 'md' ? 'Markdown' : format === 'docx' ? 'Word' : 'PDF' }));
    else ElMessage.error(t('chat.exportFailed', { error: error instanceof Error ? error.message : String(error) }));
  },
});
provide(CHAT_MESSAGE_EXPORT, messageExport);
const messageExportActions = messageExport.actions;

// 溯源入口与联网来源的展示归属；不重新启用已注释的溯源侧栏。
const sourceInteraction = createSourceInteractionBinding(chatContextBinding, {
  traceActions: useTraceSourceActions,
  schedule: callback => viewEffects.tick(callback),
  scroll: () => getScrollWrap()?.scrollBy({ top: 100, behavior: 'smooth' }),
  getInvoke,
  fallback: url => { window.open(url, '_blank', 'noopener,noreferrer'); },
  linkFailed: () => ElMessage.error('打开来源链接失败，请重试'),
});
provide(CHAT_SOURCE_LINKS, sourceInteraction);
const sourceActions = sourceInteraction.actions;
const webSearchPanel = sourceInteraction.webPanel;
function handleTraceSource(type: string, messageId: string) { sourceActions.value?.trace(type, messageId); }
function handleWebSearch(messageId: string) { sourceActions.value?.webSearch(messageId); }
// 内部历史恢复/重置仍使用固定 Tab 的状态字段。
const currentWebSearchMessageId = tabState.state.currentWebSearchMessageId;

// 右侧面板是否应展示：必须有对应内容，避免空占位列导致主区不回填
const showRightAside = computed(() => {
  if (rightPanel.value === 'prompt') return true
  // if (rightPanel.value === 'trace') return !!currentTraceMsg.value // 右侧溯源面板暂时关闭
  if (rightPanel.value === 'websearch') return !!webSearchPanel.value
  if (rightPanel.value === 'preview') return previewStore.isOpen
  return false
});

const qualityFeedbackActions = createQualityFeedbackBinding(chatContextBinding, {
  getInvoke,
  prompt: async () => {
    const { value } = await ElMessageBox.prompt(t('chat.feedbackHint'), t('chat.feedbackPlaceholder'), {
      confirmButtonText: t('common.confirm'), cancelButtonText: t('common.cancel'),
      inputPlaceholder: t('chat.feedbackPlaceholder'),
      inputValidator: input => Boolean(input?.trim()) || t('chat.feedbackPlaceholder'),
    });
    return value;
  },
  report: kind => {
    if (kind === 'success') ElMessage.success(t('chat.feedbackSubmitted'));
    else if (kind === 'unavailable') ElMessage.info('质量反馈需要 Tauri 后端支持');
    else ElMessage.error(t('chat.submitFailed'));
  },
});

// 翻译
function handleTranslate(id: string, lang: string) { return messageActions.value?.translate(id, lang); }

// 编辑器分屏
const documentEditor = createDocumentEditorBinding(chatContextBinding, {
  title: (content, context) => context.store.messages.find(m => m.content === content)?.role === 'user'
    ? t('chat.myQuestion') : t('chat.aiAnswer'),
  panelWidth: getDefaultChatPanelPx,
});
const documentEditorPanel = documentEditor.panel;
const documentEditorActions = documentEditor.actions;
const documentVersionCards = documentEditor.cards;
function openEditor(content: string) {
  documentEditorActions.value?.open(content);
}

function sendDocEditMessage() {
  const lease = chatContextBinding.captureUi();
  if (!lease?.isCurrent() || lease.context !== initialChatContext) return;
  const { owner } = lease.context, state = owner.state;
  if (!state.editorVisible.value) return;
  const store = lease.context.store as unknown as DocumentSubmissionStore;
  void documentSubmissionService.submit({
    owner, store,
    conversationId: store.currentConversationId,
    raw: state.messageStr.value.trim(), quote: state.quotedText.value, document: state.editorContent.value,
    modelId: state.modelId.value, providerCode: getProviderInfoForModel(state.modelId.value)?.providerCode,
  });
}

function handleFavorite(id: string, next: boolean) { return messageActions.value?.favorite(id, next); }

const FAV_HIGHLIGHT_CLASS = 'fastclaw-fav-target'
let favHighlightTimer: (() => void) | null = null

function clearFavHighlight() {
  if (favHighlightTimer) {
    favHighlightTimer()
    favHighlightTimer = null
  }
  const root = chatMainRef.value
  root?.querySelectorAll(`.${FAV_HIGHLIGHT_CLASS}`).forEach((node) => {
    node.classList.remove(FAV_HIGHLIGHT_CLASS)
  })
}

function highlightFavTarget(el: HTMLElement) {
  if (!viewEffects.isActive() || !el.isConnected) return
  clearFavHighlight()
  el.classList.add(FAV_HIGHLIGHT_CLASS)
  favHighlightTimer = viewEffects.delay(() => {
    el.classList.remove(FAV_HIGHLIGHT_CLASS)
    favHighlightTimer = null
  }, 2500)
}

function findFavMessageEl(msgId: string): HTMLElement | null {
  const root = chatMainRef.value
  if (!root) return null
  // chat-renderer 使用 data-message-id；勿用全局 document，避免命中其它 keep-alive 页
  const escaped = typeof CSS !== 'undefined' && CSS.escape ? CSS.escape(msgId) : msgId.replace(/"/g, '\\"')
  return root.querySelector<HTMLElement>(`[data-message-id="${escaped}"]`)
}

function scrollLegacyToFavMessage(msgId: string): boolean {
  const groups = groupedMessages.value
  const gIdx = groups.findIndex((g) => g.messages.some((m) => m.id === msgId))
  if (gIdx < 0 || !scrollContainerRef.value) return false
  const el = scrollContainerRef.value.querySelector(`[data-group-index="${gIdx}"]`) as HTMLElement | null
  if (!el) return false
  el.scrollIntoView({ behavior: 'smooth', block: 'start' })
  const qIdx = questionGroupIndices.value.indexOf(gIdx)
  if (qIdx >= 0) {
    highlightedIndex.value = qIdx
    viewEffects.delay(() => { highlightedIndex.value = -1 }, 2500)
  } else {
    highlightFavTarget(el)
  }
  return true
}

/** 收藏消息：滚到精确气泡（含 turn 内嵌的 assistant），并短暂高亮 */
async function scrollToFavMessage(msgId: string) {
  if (!msgId || !viewEffects.isActive()) return
  const isCurrentView = viewEffects.guard(() => true)
  shouldAutoScroll.value = false
  const maxAttempts = 24
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    await nextTick()
    if (!isCurrentView()) return
    if (historyContentLoading.value || conversationRestoring.value) {
      if (!await viewEffects.wait(50)) return
      continue
    }

    if (useChatPlanPackages.value) {
      const api = chatWorkspaceRef.value as {
        scrollToEntry?: (entryId: string, behavior?: ScrollBehavior) => unknown
      } | null
      // 独立 message entry 可走正式 API；turn 内消息再靠 data-message-id
      try {
        await api?.scrollToEntry?.(`message:${msgId}`, 'smooth')
      } catch { /* fall through to DOM query */ }

      if (!isCurrentView()) return
      const el = findFavMessageEl(msgId)
      if (el) {
        markWorkspaceProgrammatic(400)
        el.scrollIntoView({ behavior: 'smooth', block: 'center' })
        highlightFavTarget(el)
        return
      }
    } else if (scrollLegacyToFavMessage(msgId)) {
      return
    }

    if (!await viewEffects.wait(50)) return
  }
}

const { render: renderMd } = useMarkdown()


const asideTab = tabState.state.asideTab;
const favItems = ref<FavItem[]>([])
const favLoading = ref(false)
const favDetailVisible = ref(false)
const favDetailItem = ref<FavItem | null>(null)

async function loadFavorites() {
  favLoading.value = true
  try {
    const invoke = await getInvoke()
    favItems.value = invoke ? await invoke<FavItem[]>('get_favorite_messages') : []
  } catch (e) {
    console.warn('load favorites failed', e)
  } finally {
    favLoading.value = false
  }
}

function switchAsideTab(tab: 'history' | 'favorites') {
  asideTab.value = tab
  if (tab === 'favorites') loadFavorites()
}

async function onFavClick(id: string) {
  const item = favItems.value.find(item => item.id === id)
  if (!item) return
  if (item.conversation_deleted || !item.conversation_id) {
    favDetailItem.value = item
    favDetailVisible.value = true
    return
  }
  asideTab.value = 'history'
  shouldAutoScroll.value = false
  await selectConversation(item.conversation_id, { messageId: item.message_id || undefined })
}

async function onFavRemove(id: string) {
  const item = favItems.value.find(item => item.id === id)
  if (!item) return
  if (!item.message_id) return
  try {
    const invoke = await getInvoke()
    if (!invoke) return
    await invoke('favorite_message', { messageId: item.message_id, isFavorite: false })
    favItems.value = favItems.value.filter((i) => i.id !== item.id)
    // 同步更新当前会话中对应消息的 is_favorite 标记（修复 #194 不可变模式）
    const msgIdx = chat.messages.findIndex((m) => m.id === item.message_id)
    if (msgIdx >= 0) {
      chat.messages[msgIdx] = { ...chat.messages[msgIdx], is_favorite: 0 }
    }
  } catch (e) {
    console.warn('remove favorite failed', e)
  }
}

function favRenderedContent(content: string): string {
  try { return renderMd(content) } catch { return content }
}
const favRoleLabel = (r: string) => (r === 'user' ? t('chat.myQuestion') : t('chat.aiAnswer'))

const messagePresentation = createMessagePresentationBinding(chatContextBinding, {
  scheduleFocus: callback => viewEffects.tick(callback),
  focus: () => textareaRef.value?.focus?.(),
  unsupported: () => ElMessage.warning(t('chat.ttsNotSupported')),
  failed: () => ElMessage.warning('朗读失败，请重试'),
});
const messagePresentationActions = messagePresentation.actions;
const readingMessageId = messagePresentation.readingMessageId;
function handleQuote(content: string) { messagePresentationActions.value?.quote(content); }
// Internal reset/detach cleanup still belongs to this fixed Tab, including while hidden.
// User-facing clear events use the captured presentation action instead.
function clearQuote() { quotedText.value = ''; }
function handleTTS(id: string, content: string) { messagePresentationActions.value?.tts(id, content); }
function stopReading() { messagePresentation.stop(); }
// 发送时将引用内容以 Markdown 引用块的形式拼接到正文前
// 第一行为给模型的追问说明（前端展示时会被解析隐藏），随后是引用块与正文
function composeWithQuote(raw: string): string {
  if (!quotedText.value) return raw;
  return (
    "[追问] 我针对下面引用的内容继续提问，请结合引用内容作答：\n\n" +
    "> " + quotedText.value + "\n\n" + raw
  );
}
// 提示词应用：把内容填入输入框
function onPromptApply(content: string) {
  // 清理首尾空白，避免模板变量前后出现多余换行
  messageStr.value = content.trim();
  rightPanel.value = "none";
  viewEffects.tick(() => textareaRef.value?.focus?.());
}

// 技能芯片
function handleSkillClick(text: string) {
  messageStr.value = text;
  viewEffects.tick(() => textareaRef.value?.focus?.());
}

async function handleAgentOpeningSuggestionClick(e: MouseEvent) {
  const target = e.target as HTMLElement | null;
  const suggestionEl = target?.closest(".agent-opening-suggestion") as HTMLElement | null;
  if (!suggestionEl || inConversation.value) return;

  const text =
    suggestionEl.querySelector(".agent-opening-suggestion-text")?.textContent?.trim() ||
    suggestionEl.textContent?.trim() ||
    "";
  if (!text) return;

  await resendUserQuestion(text);
}

// 粘贴：图片 / 文档
const SUPPORTED_DOC_EXTS = [
  "pdf",
  "doc",
  "docx",
  "txt",
  "md",
  "xls",
  "xlsx",
  "csv",
];
const SUPPORTED_DOC_MIMES = [
  "application/pdf",
  "application/msword",
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  "text/plain",
  "text/markdown",
  "text/x-markdown",
  "application/vnd.ms-excel",
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  "text/csv",
];

function isSupportedDocFile(file: File): boolean {
  if (SUPPORTED_DOC_MIMES.includes(file.type)) return true;
  const ext = file.name.split(".").pop()?.toLowerCase();
  return ext ? SUPPORTED_DOC_EXTS.includes(ext) : false;
}

function handlePaste(e: ClipboardEvent) {
  const items = e.clipboardData?.items;
  if (!items) return;
  for (const item of Array.from(items)) {
    if (item.kind === "file" && item.type.startsWith("image/")) {
      // 所有模型都允许粘贴图片：多模态直接传图，非多模态走 OCR 提取文字
      const file = item.getAsFile();
      if (!file) continue;
      addImageAttachment(file);
      e.preventDefault();
    }
    // 粘贴文档文件：与手动上传一致，阻止文本内容粘贴
    if (item.kind === "file") {
      const file = item.getAsFile();
      if (!file) continue;
      if (isSupportedDocFile(file)) {
        e.preventDefault();
        uploadSingleDoc(file);
      }
    }
  }
}

// ─── 生命周期 ───
const isFirstEnter = ref(true); // 每次组件被销毁重建，这个变量都会被重置为 true
const savedScrollTop = tabState.state.savedScrollTop;  // 保存滚动位置

function saveReadingPosition() {
  const wrap = getScrollWrap();
  if (!wrap?.isConnected || tabState.disposed) return;
  const position = captureReadingPosition(wrap);
  savedScrollTop.value = position.top;
  tabState.state.readingAnchor.value = position.anchor;
}

watch(() => route.path, (path, previous) => {
  if (previous === myPath && path !== myPath) saveReadingPosition();
}, { flush: 'sync' });

let activeUiListenersAttached = false;
let viewActivationGeneration = 0;

function syncForegroundConversation() {
  const id = chat.currentConversationId;
  const visible = activeUiListenersAttached && route.path === myPath && document.visibilityState !== 'hidden';
  // currentConversationId 先于异步消息加载更新；旧内容仍显示时不能把新会话当作已查看。
  const showingConversation = id && chat.messages.some(message => message.conversation_id === id);
  setForegroundConversation(myPath, visible && showingConversation ? id : null);
}

watch(() => [route.path, chat.currentConversationId, chat.messages, chat.messages.length], syncForegroundConversation, { flush: 'post' });

function onModelUpdated() {
  loadModels();
}

function attachActiveUiListeners() {
  if (!viewEffects.activate()) return;
  activeUiListenersAttached = true;
  viewEffects.listen(document, 'visibilitychange', syncForegroundConversation);
  viewEffects.listen(window, 'user-avatar-changed', onChatAvatarChanged);
  viewEffects.listen(document, 'click', onDocumentClick);
  viewEffects.listen(document, 'fastclaw:model-updated', onModelUpdated);
  viewEffects.listen(document, 'pointerdown', onGlobalPointerDownCapture, true);
  syncForegroundConversation();
  onChatAvatarChanged();
  viewEffects.own(() => {
    setForegroundConversation(myPath, null);
    lastPointerEl = null;
  });
}

function detachActiveUiListeners() {
  activeUiListenersAttached = false;
  viewEffects.deactivate();
}

let conversationRouteInFlight: string | null = null;
async function consumeConversationRoute() {
  if (route.path !== myPath || !activeUiListenersAttached) return;
  const id = typeof route.query.convId === 'string' ? route.query.convId : null;
  if (!id) return;
  const requestPath = route.fullPath;
  if (conversationRouteInFlight === requestPath) return;
  conversationRouteInFlight = requestPath;
  try {
    if (!chat.conversations.some(c => c.id === id)) {
      ElMessage.warning('原对话不存在或已删除');
      cancelPendingConversation(myPath, id);
    } else {
      const openedHere = await selectConversation(id, {
        hydrate: chat.currentConversationId === id && chat.messages.length === 0,
        messageId: typeof route.query.msgId === 'string' ? route.query.msgId : undefined,
      });
      if (!openedHere) return;
    }
    // 只消费本次导航的定位参数，不能清掉另一次点击或其他 Tab 的 query。
    if (route.path === myPath && route.fullPath === requestPath) {
      const { convId: _convId, msgId: _msgId, ...query } = route.query;
      await router.replace({ path: myPath, query });
    }
  } finally {
    if (conversationRouteInFlight === requestPath) conversationRouteInFlight = null;
  }
}

watch(() => [route.path, route.query.convId, route.query.msgId], () => {
  if (!isFirstEnter.value) void consumeConversationRoute();
}, { flush: 'post' });

async function activateChatView() {
  // ChatView 通常由 KeepAlive 激活；通过轻量路由外壳延迟挂载时则先收到 mounted。
  // 两条生命周期共用同一幂等入口，保证历史恢复和 FastPlan 探测不会漏掉或重复执行。
  if (activeUiListenersAttached || route.path !== myPath) return;
  chatContextBinding.activate(initialChatContext);
  chatPerfActivationReady = false;
  chatSwitchPerf.markViewActivated(myPath);
  const activationGeneration = ++viewActivationGeneration;
  const isCurrentActivation = () => (
    viewEffects.isActive() && activationGeneration === viewActivationGeneration
  );
  fastPlanWelcomeViewActive.value = true;
  workspacePostPaintReady = false;
  postPaintUiWorkPending = true;
  restoreLegacyScrollOnNextPaint = false;
  attachActiveUiListeners();
  await nextTick();
  if (!isCurrentActivation()) return;
  const firstEnter = isFirstEnter.value;
  if (firstEnter) {
    // 首次恢复会话期间用户可能已经发送消息。记录进入时的状态，避免下面的异步
    // loadConversations/loadSettings 完成后再用数据库旧消息覆盖刚插入的本地消息，
    // 否则欢迎页和会话页会反复切换，流式回答也可能短暂消失。
    const restoreConversationId = chat.currentConversationId;
    const shouldHydrateInitialMessages = !!restoreConversationId && (!restoringTabMemory || tabState.hydrationRequired);
    // 【场景一：首次进入】或【场景二：被max淘汰后重新进入】
    await chat.loadConversations();
    if (!isCurrentActivation()) return;
    await settingsStore.loadSettings();
    if (!isCurrentActivation()) return;
    loadModels();
    if (!route.query.convId &&
      shouldHydrateInitialMessages &&
      chat.currentConversationId === restoreConversationId
    ) {
      await selectConversation(restoreConversationId!, { hydrate: true });
      if (!isCurrentActivation()) return;
    }
    isFirstEnter.value = false;
  }
  await consumeConversationRoute();
  if (!isCurrentActivation()) return;
  chatPerfActivationReady = true;
  if (restoringTabMemory) restoreLegacyScrollOnNextPaint = true;
  if (asideTab.value === 'favorites') void loadFavorites();
  if (!firstEnter) {
    // 【场景三：keep-alive 切回】非关键目录刷新延后到内容首帧之后。
    // 当前实例已保留自己的 doc-edit 状态，无需重新读取并清空响应式对象。
    postActivationRefreshPending = true;
    restoreLegacyScrollOnNextPaint = true;
  }
  scheduleChatContentPaintReport();
}

onMounted(activateChatView);
onActivated(activateChatView);
/** 同一激活代次只释放展示资源；不能释放 Tab owner 或中止后台执行。 */
function stopChatViewPresentation() {
  chatContextBinding.deactivate();
  if (!activeUiListenersAttached) return;
  saveReadingPosition();
  viewActivationGeneration += 1;
  chatPerfActivationReady = false;
  postActivationRefreshPending = false;
  postPaintUiWorkPending = false;
  restoreLegacyScrollOnNextPaint = false;
  workspacePostPaintReady = false;
  cancelPostActivationRefresh();
  cancelChatContentPaintReport();
  fastPlanWelcomeViewActive.value = false;
  cancelFastPlanWelcomeProbe();
  stopWorkspaceFollow();
  scrollPending = false;
  detachWorkspaceGrowObserver();
  detachWorkspaceScrollListener();
  detachActiveUiListeners();
  onSelectVisibleChange(false);
  clearFavHighlight();
  highlightedIndex.value = -1;
  onInputAreaResizeEnd();
  onSplitDragEnd();
  unbindMediumZoomWheelZoom();
  navResizeObserver?.disconnect();
  navResizeObserver = null;
  closeSwitchModelAnchor();
  stopReading();
}

onDeactivated(stopChatViewPresentation);
onBeforeUnmount(() => {
  stopChatViewPresentation();
  messagePresentation.dispose();
  viewEffects.dispose();
  chatContextBinding.dispose();
  viewDisposed = true;
  conversationLoadSequence++;
});
onUnmounted(() => {
  // File/Blob/上传任务属于 Tab；缓存淘汰只卸载视图，确认关闭/刷新才释放。
});

provide('getProviderIcon', getProviderIcon);
provide('isVendorIncomplete', isVendorIncomplete);
</script>

<template>
  <div ref="chatviewShellRef" class="chatview-shell">
    <!-- 左：会话历史 -->
    <ChatHistorySidebar :conversations="historyConversations" :current-conversation-id="chat.currentConversationId"
      :collapsed="isHistoryCollapsed" :active="fastPlanWelcomeViewActive" :aside-tab="asideTab"
      :fav-items="favItems" :fav-loading="favLoading" :activities="historyActivities"
      @new-chat="newCreateChat" @select="selectConversation" @rename="handleEditClick" @delete="handleDeleteClick"
      @collapse="isHistoryCollapsed = true" @tab-change="switchAsideTab"
      @favorite-open="onFavClick" @favorite-remove="onFavRemove" />

    <!-- 中：聊天区 -->
    <main class="chat-main" ref="chatMainRef" :class="{ 'with-editor': editorVisible }" @click="handleAgentOpeningSuggestionClick"
      :style="editorVisible ? { flex: `0 0 ${chatPanelPx}px`, minWidth: `${CHAT_MIN_PX}px` } : {}">
      <!-- 折叠状态下的展开按钮 -->
      <el-tooltip v-if="isHistoryCollapsed" :content="t('chat.expandSidebarTitle')" placement="right" effect="dark"
        :show-after="300" :enterable="false" :hide-after="0" popper-class="chat-history-dark-tooltip">
        <button class="btn-expand-conv" @click="isHistoryCollapsed = false">
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 18 16">
            <path id="路径_5211" data-name="路径 5211"
              d="M103.007,185.666a.985.985,0,0,0,.968,1h15.5a1,1,0,0,0,0-2h-15.5a.985.985,0,0,0-.968,1Zm13.607-14.243v7.486a.5.5,0,0,0,.3.462.473.473,0,0,0,.528-.109l3.282-3.389a1.023,1.023,0,0,0,0-1.414l-3.283-3.39a.473.473,0,0,0-.527-.108.5.5,0,0,0-.3.462Zm-13.607,7.243a.985.985,0,0,0,.968,1h6.779a1,1,0,0,0,0-2h-6.779A.985.985,0,0,0,103.007,178.666Zm0-7a.985.985,0,0,0,.968,1h6.779a1,1,0,0,0,0-2h-6.779A.985.985,0,0,0,103.007,171.666Z"
              transform="translate(-103.007 -170.667)" fill="currentColor" />
          </svg>
        </button>
      </el-tooltip>

      <!-- 欢迎屏（无消息时） -->
      <div v-if="!vendorConfigPanel && !chat.messages.length && !fastPlanHasContent && !dialogueCharacter.taskPlan" class="welcome-screen"
        :class="{ 'welcome-screen--workspace': useChatPlanPackages }">
        <div style="display: flex; flex-direction: column; align-items: center; gap: 10px">
          <img v-if="appLogo" :src="appLogo" alt="logo" class="welcome-logo" />
          <span v-else v-html="logoSvg" class="welcome-logo welcome-logo-svg" />
          <div class="welcome-text">
            <!-- 标题 -->
            <div class="welcome-title">
              {{ $t("chat.welcomeTitle") }}
            </div>

            <!-- 副标题 -->
            <div class="welcome-sub">
              {{ $t("chat.welcomeSubtitle") }}
            </div>
          </div>
        </div>
        <div class="welcome-skills">
          <button class="skill-chip" @click="handleSkillClick(t('chat.skillPrompt1'))">
            <svg-icon iconName="周末计划" />
            {{ t("chat.welcomeSkill1") }}
          </button>
          <button class="skill-chip" @click="handleSkillClick(t('chat.skillPrompt2'))">
            <svg-icon iconName="通俗解释" />
            {{ t("chat.welcomeSkill2") }}
          </button>
          <button class="skill-chip" @click="handleSkillClick(t('chat.skillPrompt3'))">
            <svg-icon iconName="代码评审" />
            {{ t("chat.welcomeSkill3") }}
          </button>
          <button class="skill-chip" @click="handleSkillClick(t('chat.skillPrompt4'))">
            <svg-icon iconName="中译英" />
            {{ t("chat.welcomeSkill4") }}
          </button>
        </div>
      </div>
      <PlanningWelcome v-if="!vendorConfigPanel && useChatPlanPackages && dialogueCharacter.taskPlan && !fastPlanEntries.length && !chat.messages.length"
        :connection-status="fastPlanWelcomeConnectionStatus"
        class="fastplan-workspace-welcome" />
      <!-- 公共会话时间线 + 公共 Composer；普通聊天先切换，FastPlan 在 P3 合并为 embedded entry。 -->
      <el-alert v-if="chatSubmissionError" :title="chatSubmissionError" type="error" show-icon
        @close="chatSubmissionError = ''" />
      <ChatWorkspace
        v-if="useChatPlanPackages"
        ref="chatWorkspaceRef"
        class="fastclaw-chat-workspace"
        :class="{ 'is-input-resized': inputAreaHeightPx != null, 'is-input-resizing': isResizingInputArea }"
        :style="workspaceInputAreaStyle"
        :model-value="messageStr"
        v-on="workspaceContextEvents"
        :timeline="chatPlanTimeline"
        :content-loading="historyContentLoading"
        :conversation-id="chat.currentConversationId || undefined"
        :plugins="chatWorkspacePlugins"
        :composer-actions="workspaceComposerActions"
        :composer-attachments="workspaceComposerAttachments"
        :composer-loading="inConversation || fastPlanBusy || pendingAutoSend"
        :composer-disabled="!!vendorConfigPanel || (!modelId && !inConversation)"
        :composer-placeholder="chatInputPlaceholder"
        :composer-max-height="workspaceComposerMaxHeightPx"
        :composer-auto-resize="workspaceComposerAutoResize"
        :actions="FASTCLAW_MESSAGE_ACTIONS"
        :resolve-actions="resolveFastClawMessageActions"
        :theme-tokens="FASTCLAW_CHAT_THEME_TOKENS"
        :labels="workspaceLabels"
        :theme="workspaceThemeMode"
        appearance="seamless"
        :auto-scroll="false"
        :show-context="false"
        show-question-navigator
        show-scroll-to-bottom
      >
        <template #empty><span /></template>
        <template #message-avatar="{ message }">
          <div v-if="message.role === 'user'" class="fastclaw-user-avatar">
            <img :src="userAvatarSrc" alt="User" @error="userAvatarError = true" />
          </div>
          <img v-else-if="message.model?.avatar" :src="message.model.avatar" alt="AI" class="fastclaw-workspace-avatar" />
          <el-icon v-else :size="20" class="fastclaw-workspace-model-icon">
            <SvgIcon :icon-name="workspaceModelIcon(message.model?.id)" />
          </el-icon>
        </template>
        <template #message-action-icon="{ action, message, iconName, active, copied }">
          <FastClawMessageActionIcon :name="iconName"
            :label="iconName === 'loader' && action?.name === 'translate' ? t('chat.translating') : undefined"
            :active="action?.name === 'favorite' ? !!message?.metadata?.favorite
              : action?.name === 'translate' ? !!message?.translations?.length || !!message?.metadata?.translating
                : action?.name === 'tts' ? readingMessageId === message?.id
                  : active" :copied="copied" />
        </template>
        <!-- 问题气泡内联编辑：样式对齐任务规划（框内取消 | 输入 | 确认） -->
        <template #message-after-content="{ message }">
          <div
            v-if="message.role === 'user' && inlineQuestionPanel?.id === message.id"
            :key="inlineQuestionPanel.key"
            class="inline-user-edit workspace-inline-user-edit"
            @click.stop
          >
            <button class="inline-edit-cancel" type="button" :title="t('common.cancel')" v-on="{ click: inlineQuestionPanel.events.cancel }">
              <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
                <path d="M18 6L6 18M6 6l12 12" />
              </svg>
            </button>
            <textarea
              :value="inlineQuestionPanel.content" v-on="{ input: inlineQuestionPanel.events.input, keydown: inlineQuestionPanel.events.keydown }"
              class="inline-edit-textarea"
              rows="2"
              autofocus
            />
            <button
              class="inline-edit-submit"
              type="button"
              :title="t('common.confirm')"
              :disabled="!inlineQuestionPanel.content.trim() || inConversation || fastPlanBusy || sendBtnLock || pendingAutoSend"
              v-on="{ click: inlineQuestionPanel.events.submit }"
            >
              <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2.4" fill="none">
                <polyline points="20 6 9 17 4 12" />
              </svg>
            </button>
          </div>
        </template>
        <template #scroll-to-bottom-icon>
          <el-icon><Bottom /></el-icon>
        </template>
        <template #composer-before>
          <div :id="fastPlanComposerTargetId" class="fastplan-composer-contribution-target">
            <div
              v-if="remotePlanPanel && remoteComposerPlan"
              class="fastplan-conversation__host-composer-contribution"
              data-density="compact"
            >
              <FastPlanTaskList
                :key="remotePlanPanel.key"
                :plan="remoteComposerPlan"
                :confirmation="remotePlanPanel.confirmation"
                :aria-busy="remotePlanPanel.pending"
                density="compact"
                v-on="remotePlanPanel.events"
              />
            </div>
          </div>
          <ComposerContextHints v-bind="composerHintProps" v-on="composerHintEvents" />
          <CommandPalette :commands="slash.filtered.value" :selected-index="slash.selectedIndex.value"
            :visible="slash.showMenu.value" :search-query="slash.searchQuery.value" :active-tab="slash.activeTab.value"
            v-on="composerPaletteEvents" />
        </template>
        <template #composer-inside-top>
          <div class="fastclaw-workspace-resize-handle" role="separator" aria-orientation="horizontal"
            title="拖动调整输入区高度" @pointerdown="onInputAreaResizeStart" />
        </template>
        <!-- 输入区图片附件：el-image 点击放大（对齐旧版 attachments-bar）；删除按钮 stop，文档附件保持原卡片结构 -->
        <template #composer-attachment="{ attachment, remove, retry }">
          <ComposerAttachmentCard :attachment="attachment" :image-preview-list="imagePreviewList"
            :file-icon="composerAttachmentIcon(attachment)" @remove="remove($event)" @retry="retry($event)" />
        </template>
        <template #composer-selectors>
          <ComposerSelectors ref="workspaceSelectorsRef" :locked="isAgentConfigLocked" :model-popper-class="modelPopperOwnerClass"
            v-bind="composerSelectorProps" :model-groups="modelTree" :is-vendor-incomplete="isVendorIncomplete"
            :roles="assistantStore.roles" :role-id="assistantStore.activeRole?.id ?? ''" :busy="fastPlanBusy"
            v-on="composerSelectorEvents" />
        </template>
        <template #composer-action="{ action, trigger }">
          <ComposerAction :action="action" :active="fastPlanWelcomeViewActive"
            :window-list-visible="windowListVisible" :popover-style="popoverStyle"
            :conversation-id="chat.currentConversationId || undefined"
            v-on="composerActionEvents(trigger)" />
        </template>
        <template #composer-submit-icon><Promotion class="fastclaw-submit-icon" /></template>
        <template #composer-abort-icon><VideoPause class="fastclaw-abort-icon" /></template>
      </ChatWorkspace>
      <!-- 规划引擎面板 -->
      <div v-else-if="dialogueCharacter.taskPlan" class="fastplan-panel">
        <p>旧版规划入口已停用，请切换到新版规划界面。</p>
        <el-button @click="useChatPlanPackages = true">启用新版规划</el-button>
      </div>
      <!-- 消息滚动区 -->
      <div v-else ref="scrollContainerRef" class="msg-scroll">
        <el-scrollbar ref="elScrollbarRef" view-style="padding: 16px 45px 16px;" @scroll="onScroll">
          <!-- 消息列表 -->
          <template v-for="(group, gIdx) in groupedMessages" :key="group.groupId || `g-${gIdx}`">
            <div class="msg-group" :class="{ highlight: isQuestionHighlighted(gIdx) }" :data-group-index="gIdx">
              <!-- 用户消息 -->
              <template v-if="
                group.messages.length === 1 &&
                group.messages[0].role === 'user'
              ">
                <!-- 文档附件卡片（显示在用户提问上方） -->
                <div v-if="getDocAttachments(group.messages[0]).length" class="doc-attachments-strip">
                  <div v-for="doc in getDocAttachments(group.messages[0])" :key="doc.id" class="doc-card"
                    :title="doc.name">
                    <el-icon :size="16">
                      <DocumentIcon />
                    </el-icon>
                    <span class="doc-card-name">{{
                      truncateFileName(doc.name, 12)
                    }}</span>
                    <button class="doc-quote-btn" title="引用此文档" v-on="messagePresentationActions?.documentEvents(doc) || {}">
                      <el-icon :size="14">
                        <ChatLineRound />
                      </el-icon>
                    </button>
                  </div>
                </div>
                <div class="msg-row user">
                  <div v-if="inlineQuestionPanel?.id === group.messages[0].id" :key="inlineQuestionPanel.key" class="inline-user-edit" @click.stop>
                    <button class="inline-edit-cancel" title="取消" v-on="{ click: inlineQuestionPanel.events.cancel }">
                      <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2"
                        fill="none">
                        <path d="M18 6L6 18M6 6l12 12" />
                      </svg>
                    </button>
                    <textarea
                      :value="inlineQuestionPanel.content" v-on="{ input: inlineQuestionPanel.events.input, keydown: inlineQuestionPanel.events.keydown }"
                      class="inline-edit-textarea"
                      rows="2"
                      autofocus
                    />
                    <button class="inline-edit-submit" title="完成"
                      :disabled="!inlineQuestionPanel.content.trim() || inConversation || fastPlanBusy || sendBtnLock || pendingAutoSend" v-on="{ click: inlineQuestionPanel.events.submit }">
                      <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2.4"
                        fill="none">
                        <polyline points="20 6 9 17 4 12" />
                      </svg>
                    </button>
                  </div>
                  <MessageBubble v-else :message="group.messages[0]" :disable-edit="inConversation"
                    :user-avatar-src="userAvatarSrc" @edit="handleEditMessage" @delete="deletionActions?.remove"
                    @favorite="messageActions?.favorite" @quote="messagePresentationActions?.quote" @resend-edit="resendUserQuestion" />
                </div>
              </template>

              <!-- AI 单回答 -->
              <template v-else-if="
                group.messages.length === 1 &&
                group.messages[0].role === 'assistant'
              ">
                <div class="msg-row assistant">
                  <MessageBubble :message="group.messages[0]" :showSwitchModel="true"
                    :reading-id="readingMessageId"
                    :user-avatar-src="userAvatarSrc" :showRegenerate="group.messages[0].id === lastAiMsgId"
                    :showBranch="true"
                    :model-groups="modelTree"
                    :hide-doc-edit="!!docEditMessageIds[group.messages[0].id]" @regenerate="handleRegenerate"
                    @delete="deletionActions?.remove" @edit="handleEditMessage" @translate="messageActions?.translate"
                    @tts="messagePresentationActions?.tts" @favorite="messageActions?.favorite" @quote="messagePresentationActions?.quote" @edit-as-doc="documentEditorActions?.open"
                    @switch-model-change="onSwitchModelChange" @like="messageActions?.like" @dislike="messageActions?.dislike"
                    @trace-source="sourceActions?.trace" @web-search="sourceActions?.webSearch" @branch="branchActions?.branch">
                    <template #below-content>
                      <DocVersionCard v-if="docEditCardMap[group.messages[0].id]"
                        :title="editorTitle || t('chat.aiAnswer')"
                        :version-label="docEditCardMap[group.messages[0].id].label"
                        :timestamp="docEditCardMap[group.messages[0].id].timestamp"
                        @click="documentVersionCards[group.messages[0].id]" />
                    </template>
                  </MessageBubble>
                </div>
              </template>

              <!-- 多模型回答组（含普通 user+assistant 对） -->
              <template v-else>
                <ModelAnswerGroup v-if="group.messages.some((m) => m.role === 'assistant')" :messages="group.messages"
                  :reading-id="readingMessageId"
                  :allMessages="chat.messages" :messageIndex="group.startIndex" :lastAiMsgId="lastAiMsgId"
                  :user-avatar-src="userAvatarSrc" :showBranch="true"
                  :model-groups="modelTree" :doc-edit-cards="docEditCardMap" :in-conversation="inConversation"
                  :doc-edit-title="editorTitle || t('chat.aiAnswer')" @regenerate="handleRegenerate"
                  @delete="deletionActions?.remove" @edit="handleEditMessage" @resend-edit="resendUserQuestion"
                  @translate="messageActions?.translate" @tts="messagePresentationActions?.tts" @favorite="messageActions?.favorite" @quote="messagePresentationActions?.quote"
                  @edit-as-doc="documentEditorActions?.open" @switch-model-change="onSwitchModelChange" @like="messageActions?.like"
                  @dislike="messageActions?.dislike" @trace-source="sourceActions?.trace" @web-search="sourceActions?.webSearch"
                  @branch="branchActions?.branch" @version-change="documentEditorActions?.select"
                  @quality-submit="qualityFeedbackActions?.submit"
                  @quote-doc="messagePresentationActions?.quoteDocument" />
              </template>
            </div>
            <!-- 上下文已清除分隔线 -->
            <div v-if="showContextDivider(gIdx)" class="context-cleared-divider">
              <span>{{ t('chat.contextClearedDivider') }}</span>
            </div>
          </template>
          <ChatTimeline v-if="vendorConfigPanel" :entries="providerCompletionTimeline" :plugins="chatWorkspacePlugins"
            :show-actions="false" :theme="workspaceThemeMode" scroll-mode="host" appearance="seamless">
            <template #avatar="{ message }">
              <img v-if="message.role === 'user'" :src="userAvatarSrc" alt="User" class="fastclaw-workspace-avatar" />
              <el-icon v-else :size="20"><SvgIcon :icon-name="workspaceModelIcon(message.model?.id)" /></el-icon>
            </template>
          </ChatTimeline>
        </el-scrollbar>
      </div>

      <!-- 文件选择由宿主一次性选择器统一处理，新旧输入区不复用迟到 change 的 DOM。 -->

      <!-- 输入区 -->
      <div
        v-if="!useChatPlanPackages"
        class="input-area"
        ref="inputAreaRef"
        :class="{ 'is-resized': inputAreaHeightPx != null, 'is-resizing': isResizingInputArea }"
        :style="inputAreaStyle"
      >
        <!-- 回到底部 -->
        <button v-show="showScrollButton" class="scroll-bottom-btn" @click="scrollToBottomAndEngage">
          <el-icon>
            <Bottom />
          </el-icon>
        </button>
        <!-- 追问引用条（位于模型 / 知识库选择器上方） -->
        <div v-if="quotedText" class="quote-ref-bar">
          <span class="quote-ref-text" :title="quotedText">{{ quotedText }}</span>
          <button class="quote-ref-close" type="button" :title="t('common.cancel')" @click="messagePresentationActions?.clearQuote">
            <el-icon :size="14">
              <Close />
            </el-icon>
          </button>
        </div>
        <!-- 顶部：模型 / 知识库 / 提示词 -->
        <div class="input-selectors" v-if="!isAgentConfigLocked">
          <div class="selectors-left">
            <div class="model-select-wrapper">
              <div v-if="modelId" class="model-select-prefix-icon">
                <el-icon :size="16">
                  <svg-icon :iconName="selectedModelIcon" />
                </el-icon>
              </div>
              <el-select v-model="modelId" :placeholder="t('chat.selectModelPlaceholder')" placement="top"
                size="default" class="chat-model-select" :popper-class="`main-chat-select-popper ${modelPopperOwnerClass}`"
                :disabled="isAgentConfigLocked" @visible-change="onSelectVisibleChange" @change="onChatModelChange">
                <template #header>
                  <el-input v-model="searchModelStr" :placeholder="t('chat.searchModel')" clearable size="small">
                    <template #prepend>
                      <el-tooltip
                        :content="hideIncompleteModels ? t('chat.showAllModels') : t('chat.hideIncompleteModels')"
                        placement="top">
                        <el-button :type="hideIncompleteModels ? 'warning' : 'default'" size="small"
                          @click="hideIncompleteModels = !hideIncompleteModels">
                          <el-icon size="16">
                            <Hide v-if="hideIncompleteModels" />
                            <View v-else />
                          </el-icon>
                        </el-button>
                      </el-tooltip>
                    </template>
                    <template #append>
                      <el-button @click="advancedModelSearch">{{
                        t("chat.advanced")
                      }}</el-button>
                    </template>
                  </el-input>
                </template>
                <el-option-group v-for="g in showModelList" :key="g.label" :label="g.label">
                  <el-option v-for="m in g.children" :key="m.id" :label="m.name" :value="m.id">
                    <el-tooltip :disabled="!isVendorIncomplete(g.providerCode) || !isSelectVisible"
                      :content="t('chat.vendorIncompleteHint')" placement="top" :show-after="500"
                      effect="dark" :hide-after="0">
                      <div class="model-item" :class="{ 'vendor-incomplete': isVendorIncomplete(g.providerCode) }"
                        @mouseenter="onMxcsHover(m)"
                        @mouseleave="clearMxcsPanel">
                        <el-icon :size="18" style="margin-right: 4px">
                          <svg-icon :iconName="getProviderIcon(g.providerCode, g.sourceCategory)" />
                        </el-icon>
                        <div class="model-item-title" :title="m.name">
                          {{ m.name }}
                        </div>
                        <div class="model-tag-box" v-if="m.tags?.length > 0">
                          <div class="scroll-box" :title="m.modelLabel">
                            <template v-for="(tag, tagInd) in m.tags" :key="tagInd">
                              <el-tag type="primary" effect="light"
                                v-if="PRESET_CHIPS.includes(tag.label || tag.value)">
                                <el-icon :size="14" :title="tag.label || tag.value">
                                  <svg-icon :iconName="tag.label || tag.value" />
                                </el-icon>
                              </el-tag>
                              <el-tag type="primary" effect="light" v-else>
                                {{ tag.label || tag }}
                              </el-tag>
                            </template>
                          </div>
                        </div>
                      </div>
                    </el-tooltip>
                  </el-option>
                </el-option-group>
              </el-select>
              <!-- 模型详情固定面板：始终贴在下拉框右侧，高度与下拉框一致 -->
              <Teleport to="body">
                <div v-if="hoveredMxcsModel && mxcsPanelRect" class="mxcs-fixed-side-panel" :style="mxcsPanelStyle">
                  <div style="height:100%;overflow:hidden;" v-html="chat.formatMxcsContent(hoveredMxcsModel.mxcs)">
                  </div>
                </div>
              </Teleport>
            </div>
            <KnowledgeBaseSelect ref="knowledgeSelectorRef" v-model="knowledgeId" :disabled="isAgentConfigLocked" />
            <!-- 角色选择器 -->
            <el-select :model-value="assistantStore.activeRole?.id ?? ''" size="default"
              style="width: 160px;" clearable placement="top" :placeholder="t('chat.defaultRole')"
              @change="onRoleChange">
              <el-option value="" :label="t('chat.defaultRole')" />
              <el-option v-for="r in assistantStore.roles" :key="r.id" :value="r.id" :label="r.name" />
            </el-select>
          </div>
        </div>
        <!-- 附件预览 -->
        <div v-if="fileList.length" class="attachments-bar">
          <div v-for="(f, idx) in fileList" :key="f.id" class="att-item"
            :class="['att-type-' + f.type, 'att-status-' + (f.status || 'pending')]">
            <!-- 图片/截图：缩略图 + el-image 内置预览（支持缩放/旋转/多图翻页） -->
            <template v-if="f.type === 'image' || f.type === 'screenshot'">
              <el-image
                v-if="f.preview_url"
                class="att-img-el"
                :src="f.preview_url"
                fit="cover"
                :preview-src-list="imagePreviewList"
                :initial-index="imagePreviewIndex(f)"
                :preview-teleported="true"
                @click.stop
              >
                <template #error>
                  <div class="att-img-error">{{ $t('chat.uploadFailed') }}</div>
                </template>
              </el-image>
              <span v-if="f.status === 'uploading'" class="att-status-label uploading">
                <el-icon class="is-loading">
                  <Loading />
                </el-icon>
              </span>
              <span v-else-if="f.status === 'error'" class="att-status-label error">
                <el-tooltip :content="f.error_message || $t('chat.uploadFailed')" placement="top" :show-after="300">
                  <el-icon>
                    <WarningFilled />
                  </el-icon>
                </el-tooltip>
              </span>
            </template>
            <!-- 文档：图标卡片 -->
            <template v-else>
              <div class="att-doc-card">
                <div class="att-doc-icon">
                  <img :src="getFileIcon(f)" alt="" class="att-doc-icon-img" />
                  <div v-if="f.status === 'uploading'" class="att-doc-spinner">
                    <el-icon class="is-loading">
                      <Loading />
                    </el-icon>
                  </div>
                </div>
                <div class="att-doc-info">
                  <span class="att-doc-name" :title="f.name">{{ f.name }}</span>
                  <span v-if="f.size" class="att-doc-size">{{ formatFileSize(f.size) }}</span>
                  <span v-else-if="f.status === 'error'" class="att-doc-err">
                    <el-tooltip :content="f.error_message || $t('chat.uploadFailed')" placement="top" :show-after="300">
                      <el-icon>
                        <WarningFilled />
                      </el-icon>
                    </el-tooltip>
                  </span>
                </div>
              </div>
            </template>
            <button class="att-close" @click="removeFile(idx)">
              <el-icon>
                <Close />
              </el-icon>
            </button>
          </div>
        </div>
        <!-- 输入框外层（圆角包裹） -->
        <div class="input-box-wrapper" :class="{ 'no-top-border': isAgentConfigLocked }">
          <div
            class="input-area-resize-handle"
            role="separator"
            aria-orientation="horizontal"
            title="拖动调整输入区高度"
            @pointerdown="onInputAreaResizeStart"
          />
          <!-- 斜杠命令面板 -->
          <CommandPalette :commands="slash.filtered.value" :selected-index="slash.selectedIndex.value"
            :visible="slash.showMenu.value" :search-query="slash.searchQuery.value" :active-tab="slash.activeTab.value"
            @select="slash.selectCommand" @update:search-query="slash.searchQuery.value = $event"
            @update:active-tab="slash.activeTab.value = $event"
            @update:selected-index="slash.selectedIndex.value = $event" @close="slash.closePopup()" />
          <!-- 选中的 Skill / MCP chip -->
          <div v-if="selectedSkill" class="selected-skill-chip">
            <span class="chip-badge" :class="selectedSkill.type">{{ selectedSkill.type === 'mcp' ? 'MCP' : 'Skill'
            }}</span>
            <span class="chip-name">{{ selectedSkill.labelEn }}</span>
            <button class="chip-close" @click="selectedSkill = null" title="取消选择">×</button>
          </div>
          <el-input ref="textareaRef" v-model="messageStr" type="textarea" :autosize="inputTextareaAutosize"
            :placeholder="chatInputPlaceholder" @keydown="handleChatKeydown"
            @input="(val: string) => slash.handleInput(val, textareaRef?.textarea?.selectionEnd || 0)"
            @paste="handlePaste" />
          <!-- 底部按钮（chatGLM 风格） -->
          <div class="input-buttons">
            <div class="btn-left">
              <el-tooltip :content="isVisionModel
                ? t('chat.imageRecognition')
                : t('chat.imageOcrHint')
                " placement="top">
                <button class="btn-icon-only" @click="imageClick">
                  <el-icon :size="17">
                    <svg-icon iconName="image" />
                  </el-icon>
                </button>
              </el-tooltip>
              <el-tooltip :content="t('chat.documentOCR')" placement="top">
                <button class="btn-icon-only" @click="ocrClick">
                  <el-icon :size="18">
                    <FileUp />
                  </el-icon>
                </button>
              </el-tooltip>
              <div ref="screenshotGroupRef" class="screenshot-group">
                <el-tooltip :content="t('chat.screenshot')" placement="top">
                  <button class="btn-icon-only" @click="handleScreenshot">
                    <el-icon :size="17">
                      <svg-icon iconName="screenshot" />
                    </el-icon>
                  </button>
                </el-tooltip>
                <button class="btn-icon-only hide-win-btn" :title="t('chat.hideWindowScreenshot')" @click.stop="toggleWindowList">
                  <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none">
                    <polyline points="6 9 12 15 18 9" />
                  </svg>
                </button>
                <Teleport to="body">
                  <div v-if="windowListVisible" class="hide-window-popover" :style="popoverStyle" @click.stop>
                    <div class="hide-win-item" @click="screenshotHideSelf">
                      {{ $t('chat.hideWindowScreenshot') }}
                    </div>
                  </div>
                </Teleport>
              </div>
              <el-tooltip v-if="!isAgentConfigLocked" :content="!isDeepThinkModel
                ? t('chat.deepThinkNotSupported')
                : dialogueCharacter.think
                  ? t('chat.deepThinkEnabled')
                  : t('chat.deepThinkDisabled')
                " placement="top">
                <button class="btn-icon-only switch" @click="thinkClick" :style="{
                  color: dialogueCharacter.think && isDeepThinkModel
                    ? 'var(--text-secondary)'
                    : '',
                  cursor: isDeepThinkModel ? 'pointer' : 'not-allowed',
                  opacity: isDeepThinkModel ? 1 : 0.4,
                }">
                  <el-icon :size="17">
                    <svg-icon iconName="think" />
                  </el-icon>
                </button>
              </el-tooltip>
              <el-tooltip v-if="!isAgentConfigLocked" :content="dialogueCharacter.taskPlan
                ? t('chat.taskPlanEnabled')
                : t('chat.taskPlanDisabled')
                " placement="top">
                <button class="btn-icon-only switch" :disabled="taskPlanSwitching" @click="taskPlanClick" :style="{
                  color: dialogueCharacter.taskPlan
                    ? 'var(--text-secondary)'
                    : '',
                }">
                  <el-icon :size="18">
                    <svg-icon iconName="taskPlan" />
                  </el-icon>
                </button>
              </el-tooltip>
              <WebSearchPopover />
              <McpToolPopover v-if="!isAgentConfigLocked" />
              <el-tooltip
                :content="chat.activeContextCount >= 20 ? t('chat.contextTooLongHint', { n: chat.activeContextCount }) : t('shortcuts.clearContext')"
                placement="top">
                <span v-show="chat.messages.length" class="tip-wrap"
                  :style="chat.activeContextCount >= 20 ? { color: 'var(--color-warning, #fa8c16)' } : {}">
                  <el-popconfirm :title="t('chat.clearContextConfirm')" :width="240" placement="top"
                    @confirm="deleteAllText">
                    <template #reference>
                      <button class="btn-icon-only"
                        :style="chat.activeContextCount >= 20 ? { color: 'var(--color-warning, #fa8c16)' } : {}">
                        <el-icon :size="17">
                          <DocumentDelete />
                        </el-icon>
                      </button>
                    </template>
                  </el-popconfirm>
                </span>
              </el-tooltip>
            </div>
            <div class="btn-right">
              <el-button circle class="send-btn" type="primary" :icon="inConversation || pendingAutoSend ? VideoPause : Promotion"
                :title="sendBtnTitle" :disabled="!messageStr && !inConversation" @mousedown.prevent @click="sendBtnFn()" />
            </div>
          </div>
        </div>
      </div>

      <!-- 问题导航 -->
      <QuestionNav v-if="!useChatPlanPackages && chat.messages.length" :messages="chat.messages" :messageListRef="scrollContainerRef"
        :question-group-indices="questionGroupIndices"
        :max-height="navMaxHeight" :nav-top="navTop" @scroll-to="scrollToQuestion" />
    </main>

    <!-- 右：提示词 / 溯源 / 节点详情 -->
    <aside v-if="showRightAside" class="right-aside" :class="rightPanel">
      <div v-if="rightPanel === 'prompt'" class="panel-prompt">
        <div class="panel-head">
          <span>{{ t("chat.promptWord") }}</span>
          <el-icon class="panel-close" @click="rightPanel = 'none'">
            <Close />
          </el-icon>
        </div>
        <div class="panel-body">
          <PromptPanel @apply="onPromptApply" @close="rightPanel = 'none'" />
        </div>
      </div>
      <!-- 右侧溯源面板暂时注释掉，溯源内容由底部内联组件展示
      <div v-else-if="rightPanel === 'trace' && currentTraceMsg" class="panel-trace">
        <div class="panel-head">
          <span>{{ t("chat.traceTitle") }}</span>
          <el-icon class="panel-close" @click="rightPanel = 'none'">
            <Close />
          </el-icon>
        </div>
        <div class="panel-body">
          <TraceSource v-if="currentTraceMsg.trace_source" :message="currentTraceMsg"
            @quality-submit="qualityFeedbackActions?.submit" @close="rightPanel = 'none'" />
          <div v-else class="placeholder-tip">{{ t("chat.noTraceData") }}</div>
        </div>
      </div>
      -->
      <div v-else-if="webSearchPanel" class="panel-websearch" :key="webSearchPanel.messageId">
        <div class="panel-head">
          <span>{{ t("chat.webSearch") }} {{ webSearchPanel.hits.length }}</span>
          <el-icon class="panel-close" @click="webSearchPanel.close">
            <Close />
          </el-icon>
        </div>
        <div class="panel-body">
          <WebSearchSources :message-id="webSearchPanel.messageId" :hits="webSearchPanel.hits" :query="webSearchPanel.query" />
        </div>
      </div>
      <div v-else-if="rightPanel === 'preview'" class="panel-preview">
        <PreviewPanel />
      </div>
    </aside>

    <!-- 拖拽分隔条（文档编辑模式下） -->
    <div v-if="editorVisible" class="split-divider" :class="{ dragging: isDraggingSplit }"
      @mousedown="onSplitDragStart">
      <div class="split-divider__grip">
        <span class="grip-dot"></span>
        <span class="grip-dot"></span>
        <span class="grip-dot"></span>
      </div>
    </div>

    <!-- 分屏编辑器 -->
    <div v-if="editorVisible" class="editor-pane">
      <LazyChatPanel v-if="fastPlanWelcomeViewActive && documentEditorPanel" :key="documentEditorPanel.key"
        :loader="loadDocumentEditor" :label="t('chat.docEdit')"
        :content="documentEditorPanel.content" :title="documentEditorPanel.title" :versions="documentEditorPanel.versions"
        :active-version-idx="documentEditorPanel.activeVersionIdx" v-on="documentEditorPanel.events" />
    </div>

    <!-- 截图遮罩（Tauri 端） -->
    <ScreenshotOverlay v-if="captureResult" :key="captureResult.request_id ?? undefined" :imageWidth="captureResult.width" :imageHeight="captureResult.height"
      :imageOffsetX="captureResult.offset_x" :imageOffsetY="captureResult.offset_y" :monitors="captureResult.monitors"
      :monitorImages="captureResult.monitorImages || []" :windows="captureResult.windows" v-on="inlineScreenshotEvents || {}" />

    <!-- 隐藏/淘汰仅卸载展示，编辑草稿仍归 Tab；主动关闭才清理编辑目标。 -->
    <LazyChatPanel v-if="fastPlanWelcomeViewActive && messageEditorPanel" :key="messageEditorPanel.key"
      :loader="loadMessageEditor" :label="t('chat.editMessage')" presentation="dialog" :model-value="true"
      :content="messageEditorPanel.content" :draft-content="messageEditorPanel.draftContent" :loading="messageEditorPanel.loading"
      v-on="messageEditorPanel.events" />


    <!-- 切换模型：锚点下拉（贴 jcr-action 按钮，上下自适应） -->
    <div
      v-if="switchModelAnchorActive"
      class="switch-model-anchor"
      :style="switchModelAnchorStyle"
      aria-hidden="true"
    >
      <ModelSelector
        ref="switchModelAnchorSelectorRef"
        v-model="switchModelSelectedId"
        :model-groups="modelTree"
        hide-advanced
        icon-tags
        :is-vendor-incomplete="isVendorIncomplete"
        :placement="switchModelPlacement"
        :fallback-placements="switchModelFallbackPlacements"
        :popper-options="switchModelPopperOptions"
        :popper-class="'model-selector-popper switch-model-popper'"
        class="switch-model-anchor__selector"
        @change="onSwitchModelSelected"
        @visible-change="onSwitchModelAnchorVisibleChange"
      />
    </div>

    <!-- 切换模型选择器回退弹窗：找不到按钮锚点时使用，逻辑与原先一致 -->
    <el-dialog v-model="switchModelPickerVisible" :title="t('chat.switchModelTitle')" width="400px" align-center>
      <ModelSelector
        ref="switchModelSelectorRef"
        v-model="switchModelSelectedId"
        :model-groups="modelTree"
        hide-advanced
        icon-tags
        :is-vendor-incomplete="isVendorIncomplete"
        :popper-class="'model-selector-popper switch-model-popper'"
        @change="onSwitchModelSelected"
      />
    </el-dialog>

    <!-- 已删除收藏详情弹窗 -->
    <el-dialog v-model="favDetailVisible" :title="favDetailItem?.conversation_title || '收藏详情'" width="600px"
      align-center>
      <div v-if="favDetailItem" class="fav-detail">
        <div class="fav-detail-meta">
          <span class="fav-role-badge" :class="favDetailItem.role">{{ favRoleLabel(favDetailItem.role) }}</span>
          <span class="fav-deleted-tag">原对话已删除</span>
        </div>
        <div class="fav-detail-content" v-html="favRenderedContent(favDetailItem.content)" />
      </div>
      <template #footer>
        <el-button @click="favDetailVisible = false">关闭</el-button>
      </template>
    </el-dialog>

    <!-- 高级模型选择器 -->
    <selectModel ref="selectModelRef" @select-model-data="onAdvancedModelSelect" />

  </div>
</template>

<style lang="scss" scoped>
@use '../components/chat/workspace/composerContextHints';
@use '../components/chat/workspace/composerSelectors';
@use '../components/chat/workspace/composerActions';
.chatview-shell {
  display: flex;
  width: 100%;
  height: 100%;
  // background: var(--app-background); //智能对话子菜单区域背景
  color: var(--text-primary);
  position: relative;
  overflow: hidden;
  min-height: 0;
}

/* ─── 左：会话历史 ─── */
.btn-send
{
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 16px;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-md);
  font-weight: 500;
  transition: all 0.2s;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
  }
}

.btn-expand-conv {
  width: 33px;
  height: 33px;
  border: 1px solid var(--border-color);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  // background: var(--bg-secondary, rgba(0,0,0,0.03));
  border-radius: 4px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-primary);
  transition: all 0.2s;
  flex-shrink: 0;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);

  &:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  // border: none;
  // border-radius: 6px;
  // background: transparent;
  // cursor: pointer;
  // color: var(--text-secondary);
  // display: flex;
  // align-items: center;
  // justify-content: center;

  // &:hover {
  //   background: var(--bg-hover);
  // }
}

.btn-expand-conv {
  position: absolute;
  top: 12px;
  left: 8px;
  z-index: 10;
}


.fav-role-badge {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--bg-secondary);
  color: var(--text-secondary);
}

.fav-role-badge.user {
  color: var(--color-text-info);
}

.fav-deleted-tag {
  font-size: 10px;
  color: #b91c1c;
  background: #fef2f2;
  border-radius: 4px;
  padding: 0 5px;
}

.fav-detail-content {
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 50vh;
  overflow-y: auto;
  font-size: 14px;
  line-height: 1.7;
}

.fav-detail-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

/* ─── 中：聊天区 ─── */
.chat-main {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  position: relative;
  background: var(--bg-primary);


  // background: var(--main-chat-background);
  //background: transparent;
  &.with-editor {
    min-width: 280px; // 保证聊天区最小可用宽度
  }
}

.fastplan-panel {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.fastplan-area {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.msg-scroll {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  contain: layout style; // 隔离流式输出引起的布局重排

  :deep(.el-scrollbar) {
    height: 100%;
  }

  :deep(.el-scrollbar__view) {
    min-height: 100%;
    display: flex;
    flex-direction: column;
  }
}

.welcome-screen {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  padding-top: clamp(48px, 12vh, 120px);
  gap: 18px;
}

.welcome-logo {
  width: 65px;
  height: 65px;
  flex-shrink: 0;
}

.welcome-logo-svg {
  color: var(--color-primary);

  :deep(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }
}

.welcome-text {
  text-align: center;
}

.welcome-title {
  font-size: var(--font-size-3xl);
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 6px;
}

.welcome-sub {
  font-size: var(--font-size-base);
  color: var(--text-secondary);
}

.welcome-skills {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 12px;
  justify-content: center;

}

.skill-chip {
  /* 关键：行内弹性，图标文字同一行 */
  display: inline-flex;
  align-items: center;
  /* 图标和文字间距，替代svg上的margin-right，更稳定 */
  gap: 5px;
  /* 可选：去掉按钮默认内边距、边框按需调整 */
  border: none;
  cursor: pointer;
  border: 1px solid var(--border-color);
  //background: var(--bg-primary);
  color: var(--text-primary);
  //padding: 8px 14px;
  //border-radius: 20px;
  cursor: pointer;
  font-size: var(--font-size-md);

  height: 42px;
  padding: 0 20px;
  background: var(--chat-input-skill-chip);
  //border:1px solid #E5E7EB;
  border-radius: 999px;

  &:hover {
    // border-color: var(--color-primary);
    // color: var(--color-primary);
    box-shadow: 0 4px 12px rgba(0, 0, 0, .06);
  }
}

.context-cleared-divider {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 6px 0 18px;
  color: var(--text-secondary, #909399);
  font-size: 12px;
  user-select: none;

  &::before,
  &::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--border-color, #e4e7ed);
  }

  span {
    white-space: nowrap;
  }
}

.msg-group {
  margin-bottom: 14px;

  &:last-child {
    margin-bottom: 0px;
  }

  &.highlight {

    // 只高亮用户提问气泡，不影响 AI 回答区域
    :deep(.msg-row.user .bubble.user-bubble),
    :deep(.user-msg-wrapper .bubble.user-bubble) {
      border-radius: 5px !important;
      background: color-mix(in srgb,
          var(--color-primary) 18%,
          transparent) !important;
      border-radius: 18px 18px 4px 18px;
      transition: background-color 0.4s ease;
    }
  }
}

/* ─── 文档附件卡片（显示在用户提问上方） ─── */
.doc-attachments-strip {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 6px;
  justify-content: flex-end;
}

.doc-card {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 8px 4px 10px;
  border-radius: 8px;
  background: var(--bg-secondary, var(--bg-hover));
  border: 1px solid transparent;
  cursor: default;
  max-width: 200px;
  transition:
    border-color 0.2s,
    box-shadow 0.2s;

  &:hover {
    border-color: var(--color-primary, #1677ff);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  .el-icon {
    flex-shrink: 0;
    color: var(--text-secondary);
  }

  .doc-quote-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    margin-left: auto;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    border-radius: 4px;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.15s, background 0.15s;

    &:hover {
      background: var(--bg-hover);
      color: var(--color-primary, #1677ff);
    }
  }

  &:hover .doc-quote-btn {
    opacity: 1;
  }
}

.doc-card-name {
  font-size: 12px;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 120px;
}

.msg-row {
  display: flex;

  &.user {
    justify-content: flex-end;
  }

  &.assistant {
    justify-content: flex-start;
  }
}

.doc-edit-card-row {
  display: flex;
  justify-content: flex-start;
  padding: 4px 0 0;
}

.inline-user-edit {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  width: min(760px, 100%);
  max-width: 100%;
  box-sizing: border-box;
  margin-left: auto;
  padding: 8px 10px;
  border: 1px solid var(--border-color, var(--jcr-border, #e2e8f0));
  border-radius: 12px;
  background: var(--bg-primary, var(--jcr-bg, #fff));
}

.inline-edit-cancel,
.inline-edit-submit {
  width: 32px;
  height: 32px;
  margin-top: 2px;
  border: none;
  background: transparent;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  cursor: pointer;
  color: var(--text-secondary, var(--jcr-muted, #64748b));
  border-radius: 8px;
  transition: color 0.15s, background 0.15s;

  &:hover {
    background: color-mix(in srgb, var(--text-primary, #1f2937) 6%, transparent);
    color: var(--text-primary, var(--jcr-fg, #1f2937));
  }

  &:disabled {
    cursor: not-allowed;
    opacity: 0.4;
  }
}

.inline-edit-submit {
  color: var(--color-primary, var(--jcr-primary, #2563eb));

  &:hover:not(:disabled) {
    color: var(--color-primary, var(--jcr-primary, #2563eb));
    background: color-mix(in srgb, var(--color-primary, #2563eb) 8%, transparent);
    opacity: 1;
  }
}

.inline-edit-textarea {
  flex: 1;
  min-width: 0;
  min-height: 40px;
  max-height: 320px;
  padding: 8px 10px;
  border: 1px solid var(--border-color, var(--jcr-border, #e2e8f0));
  border-radius: 8px;
  color: var(--text-primary, var(--jcr-fg, #1f2937));
  background: transparent;
  font: inherit;
  font-size: var(--font-size-base, inherit);
  line-height: 1.5;
  resize: vertical;
  outline: none;

  &:focus {
    border-color: color-mix(in srgb, var(--color-primary, #2563eb) 55%, var(--border-color, #e2e8f0));
  }
}

.inline-edit-input {
  flex: 1;
  min-width: 0;

  :deep(.el-textarea__inner) {
    resize: none;
    line-height: 1.6;
    font-size: var(--font-size-base);
    color: var(--text-primary);
    border-radius: 8px;
    border: 1px solid var(--border-color);
    box-shadow: none !important;
    background: transparent;
  }
}

.scroll-bottom-btn {
  position: absolute;
  right: 24px;
  bottom: calc(100% + 8px);
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: 1px solid var(--border-color);
  background: var(--bg-primary);
  color: var(--text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);

  &:hover {
    color: var(--color-primary);
  }
}

/* ─── 输入区 ─── */
.input-area {
  position: relative;
  padding: 8px 16px 16px;
  background: var(--chat-input-skill-chip);
  border: 1px solid var(--border-color);
  box-shadow: 0 8px 24px rgba(15, 23, 42, .05);
  border-radius: 20px;
  margin: 10px;
  min-height: 100px;
  flex-shrink: 0;

  &.is-resizing {
    user-select: none;
  }

  /* 手动拉高：外框固定高度，输入框填满剩余空间 */
  &.is-resized {
    display: flex;
    flex-direction: column;
    overflow: hidden;

    > *:not(.input-box-wrapper):not(.input-area-resize-handle) {
      flex-shrink: 0;
    }

    .input-box-wrapper {
      flex: 1;
      min-height: 0;
      display: flex;
      flex-direction: column;

      .selected-skill-chip,
      .input-buttons {
        flex-shrink: 0;
      }

      :deep(.el-textarea) {
        flex: 1;
        min-height: 0;
        height: auto;
        display: flex;
        flex-direction: column;
      }

      :deep(.el-textarea__inner) {
        flex: 1;
        height: 100% !important;
        min-height: 0 !important;
        max-height: none !important;
        overflow-y: auto !important;
      }
    }
  }
}

.input-area-resize-handle {
  position: absolute;
  top: 0;
  left: 16px;
  right: 16px;
  height: 10px;
  cursor: ns-resize;
  z-index: 25;
  touch-action: none;

  &::after {
    content: "";
    position: absolute;
    left: 50%;
    top: 4px;
    transform: translateX(-50%);
    width: 40px;
    height: 3px;
    border-radius: 999px;
    background: var(--border-color);
    opacity: 0.65;
    transition: background 0.15s, opacity 0.15s;
  }

  &:hover::after,
  .input-area.is-resizing &::after {
    background: var(--color-primary);
    opacity: 1;
  }
}

// #26 修复: 模型选择器自适应宽度，右侧抽屉展开时不溢出
.chat-model-select {
  width: 200px;
  max-width: 50vw;
  min-width: 120px;
}

.attachments-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 8px;
}

.att-item {
  position: relative;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: var(--bg-secondary, var(--bg-hover));
  display: flex;
  align-items: center;

  &.att-type-image,
  &.att-type-screenshot {
    padding: 4px;
  }

  &.att-type-document,
  &.att-type-other {
    padding: 0;
    // overflow: hidden;
    width: 210px;
  }

  &.att-status-uploading {
    border-color: var(--color-warning);
    opacity: 0.85;
  }

  &.att-status-error {
    border-color: var(--color-danger);
    background: rgba(255, 77, 79, 0.05);
  }

  &:hover .att-close {
    opacity: 1;
  }
}

.att-img-el {
  width: 64px;
  height: 64px;
  border-radius: 4px;
  cursor: zoom-in;
  display: block;
  overflow: hidden;

  :deep(.el-image__inner) {
    object-fit: cover;
    border-radius: 4px;
  }
}

.att-img-error {
  width: 64px;
  height: 64px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-hover);
  color: var(--text-secondary);
  font-size: 11px;
  text-align: center;
  padding: 4px;
  box-sizing: border-box;
}

/* 文档附件卡片 */
.att-doc-card {
  display: flex;
  align-items: stretch;
  width: 100%;
  min-height: 60px;
}

.att-doc-icon {
  position: relative;
  flex-shrink: 0;
  width: 52px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-hover);
  border-right: 1px solid var(--border-color);
}

.att-doc-icon-img {
  width: 30px;
  height: 30px;
  object-fit: contain;
}

.att-doc-spinner {
  position: absolute;
  inset: 0;
  background: rgba(255, 255, 255, 0.75);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 18px;
  color: var(--color-primary);
}

.att-doc-info {
  flex: 1;
  min-width: 0;
  padding: 8px 10px 8px 8px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 4px;
}

.att-doc-name {
  font-size: 12px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-primary);
  line-height: 1.4;
}

.att-doc-size {
  font-size: 11px;
  color: var(--text-tertiary);
  line-height: 1.2;
}

.att-doc-err {
  font-size: 11px;
  color: var(--color-danger);
  display: flex;
  align-items: center;
}

/* 状态角标（仅图片类型使用） */
.att-status-label {
  position: absolute;
  top: -6px;
  left: -6px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  z-index: 2;

  &.uploading {
    background: var(--color-warning);
    color: #fff;
  }

  &.error {
    background: var(--color-danger);
    color: #fff;
  }
}

.att-close {
  position: absolute;
  top: -6px;
  right: -6px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: none;
  color: #fff;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  z-index: 3;
  background: var(--color-primary);
}

.input-box-wrapper {
  position: relative;
  //border: 1px solid var(--border-color);
  border-top: 1px solid var(--border-color);

  &.no-top-border {
    border-top: none;
  }

  border-radius: 0px;
  //background: var(--input-primary, var(--bg-primary));

  padding: 10px 0px 0px 0px;
  transition: border-color 0.2s;
  position: relative;
  z-index: 20; // 高于右侧 aside (#6 按钮穿透修复)

  // &:focus-within {
  //   border-color: var(--color-primary);
  // }

  :deep(.el-textarea__inner) {
    border: none;
    box-shadow: none;
    resize: none;
    padding: 0;
    background: transparent;
    color: var(--text-primary); // 使用主题文字颜色
  }

  /* placeholder 颜色由全局主题规则控制 */
}

/* ── 选中的 Skill / MCP chip ── */

/* ── Skill 触发按钮 (@) ── */
.skill-trigger-btn {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.skill-trigger-btn.active {
  //color: var(--color-primary, #6366f1);
}

.input-buttons {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: 6px;
}

.btn-left,
.btn-right {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 2px;
}

.btn-right {
  gap: 0px;
  flex-shrink: 0;
}

.btn-icon,
.tip-wrap {
  display: inline-flex;
  align-items: center;
}

.btn-icon-only {
  border: none;
  background: transparent;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--text-secondary);

  &.switch {
    color: color-mix(in srgb, var(--text-secondary) 55%, transparent);
  }

  &:hover {
    background: var(--dropdown-hover-color);
    // color: #fff;
  }
}

.btn-icon-only-active {
  border: none;
  background: var(--bg-hover);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--color-primary);

  &.switch {
    color: color-mix(in srgb, var(--text-secondary) 55%, transparent);
  }
}

.btn-icon {
  width: 60px;
  height: 32px;
}

.btn-icon-only {
  padding: 6px;
}

.send-btn {
  width: 36px;
  height: 36px;
}

/* ─── 右：提示词 / 溯源 ─── */
.right-aside {
  width: 290px;
  flex-shrink: 0;
  border-left: 1px solid var(--chat-divider-color) !important;
  background: var(--bg-primary);
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  position: relative;
  z-index: 10;
}

.panel-prompt,
.panel-trace,
.panel-websearch,
.panel-preview {
  height: 100%;
  display: flex;
  flex-direction: column;
}

/* 网页预览面板更宽，便于阅读正文 */
.right-aside.preview {
  width: 520px;
}

.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 0;
  border-bottom: 1px solid var(--border-color);
  font-weight: 600;
  font-size: 14px;
  color: var(--text-primary);
}

.panel-close {
  cursor: pointer;
  color: var(--text-secondary);

  &:hover {
    color: var(--color-primary);
  }
}

.panel-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 12px;
}

.placeholder-tip {
  padding: 32px 16px;
  text-align: center;
  color: var(--text-tertiary);
  font-size: 13px;
}

/* ─── 分屏编辑器 ─── */
.editor-pane {
  min-width: 500px;
  flex: 1;
  background: var(--bg-primary);
}

/* ─── 拖拽分隔条 ─── */
.split-divider {
  width: 3px;
  cursor: col-resize;
  background: var(--chat-divider-color);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: background 0.15s;
  user-select: none;
  position: relative;
  z-index: 10;

  &:hover,
  &.dragging {
    background: var(--theme-dailog-color);
  }

  &__grip {
    display: flex;
    flex-direction: column;
    gap: 3px;
    pointer-events: none;
  }
}

.grip-dot {
  width: 3px;
  height: 3px;
  border-radius: 50%;
  background: var(--text-secondary);
  transition: background 0.15s;

  .split-divider:hover &,
  .split-divider.dragging & {
    background: #fff;
  }
}

// 拖拽时禁止选择文本
:global(.fastclaw-chat-split-dragging) {
  user-select: none !important;
  cursor: col-resize !important;

  * {
    user-select: none !important;
  }
}

/* ─── 切换模型弹窗 ─── */
/* 切换模型锚点下拉：贴按钮外侧，不压住 jcr-action */
.switch-model-popper {
  margin: 0 !important;
}

/* 切换模型：隐藏的 ModelSelector 锚点，仅用于定位 teleported 下拉面板 */
.switch-model-anchor {
  position: fixed;
  z-index: 4000;
  overflow: hidden;
  opacity: 0;
  pointer-events: none;
}

.switch-model-anchor__selector {
  width: 100%;
  height: 100%;

  :deep(.el-select),
  :deep(.el-select__wrapper),
  :deep(.el-input__wrapper) {
    width: 100% !important;
    min-height: 100% !important;
    height: 100% !important;
    padding: 0 !important;
    background: transparent !important;
    border: none !important;
    box-shadow: none !important;
  }
}

.model-item {
  width: 100%;
  display: flex;
  align-items: center;
  max-width: 340px;
  overflow: hidden;
  position: relative;

  &.vendor-incomplete {
    overflow: visible;
  }

  .model-item-title {
    max-width: 200px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;

  }

  // 供应商配置不完整 → 模型名称置灰
  &.vendor-incomplete {
    .model-item-title {
      opacity: 0.45;
    }
  }

  .model-tag-box {
    // max-width: calc(100% - 200px);
    flex: 1;
    margin-left: 10px;
    overflow: hidden;
    white-space: nowrap;

    .scroll-box {
      height: 100%;
      display: flex;
      align-items: center;
      color: var(--color-primary) !important;
    }

    .el-tag {
      font-weight: normal;
    }

    .el-tag+.el-tag {
      margin-left: 4px;
    }
  }
}

.selectors-right {
  display: flex;
  align-items: center;
  align-self: flex-start;

  :deep(.el-button) {
    background: transparent;
    border: 1px solid var(--border-color);
    border-radius: 18px;
    height: 32px;
    color: var(--text-primary);
  }
}

.kb-select-wrapper,
.model-select-wrapper {
  position: relative;
  display: inline-flex;
  align-items: center;

  .model-select-prefix-icon {
    position: absolute;
    left: 14px;
    top: 50%;
    transform: translateY(-50%);
    z-index: 1;
    pointer-events: none;
    display: flex;
    align-items: center;
    color: var(--text-secondary);
  }

  :deep(.el-select__wrapper) {
    background: transparent !important; // 背景色
    border: 1px solid var(--border-color); // 边框
    border-radius: 18px !important; // 圆角
    padding: 0 14px;
    height: 32px !important;
  }
}

.model-select-wrapper :deep(.el-select .el-select__selected-item) {
  padding-left: 22px;
}

:deep(.el-select__wrapper) {
  min-height: 32px !important;
  padding: 4px 12px;
}



/* ─── 截图隐藏窗口下拉 ─── */

/* 1. 左侧蓝色标签（修正hover和颜色） */

/* 2. 竖线分割 */
.divider-line {
  width: 1px;
  height: 18px;
  background: var(--chat-divider-color);
  margin: 0 8px;
}

/* 图片导出浮动操作栏 */
.image-export-float-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  margin: 0 16px 8px;
  background: var(--bg-primary, #fff);
  border: 1px solid var(--color-primary, #409eff);
  border-radius: 8px;
  box-shadow: 0 -2px 8px rgba(0, 0, 0, 0.08);
}

.image-export-float-bar-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.image-export-float-bar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>

<style>
/* 历史会话侧栏提示：默认深底白字；情景主题（除雾灰调）改为与输入区按钮 tooltip 一致（白底黑字 + 向下三角） */
.chat-history-dark-tooltip.el-popper {
  pointer-events: none !important;
}
.chat-history-dark-tooltip.el-popper.is-dark,
.chat-history-dark-tooltip.el-popper {
  background: #303133 !important;
  color: #fff !important;
  border: none !important;
}
.chat-history-dark-tooltip.el-popper .el-popper__arrow::before {
  background: #303133 !important;
  border-color: #303133 !important;
}
/* 情景主题（除雾灰调）+ 黑暗主题：白底黑字，箭头样式对齐「截图」等默认 tooltip */
html.dark .chat-history-dark-tooltip.el-popper,
html.dark .chat-history-dark-tooltip.el-popper.is-dark,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .chat-history-dark-tooltip.el-popper,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .chat-history-dark-tooltip.el-popper.is-dark,
html.light[data-theme-id="custom-scene"] .chat-history-dark-tooltip.el-popper,
html.light[data-theme-id="custom-scene"] .chat-history-dark-tooltip.el-popper.is-dark {
  background: #fff !important;
  color: #000 !important;
  border: none !important;
  box-shadow: 0 2px 12px 0 rgba(0, 0, 0, 0.1) !important;
  --el-text-color-primary: #000 !important;
  --el-text-color-regular: #000 !important;
  --el-bg-color: #fff !important;
}
html.dark .chat-history-dark-tooltip.el-popper .el-popper__arrow::before,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .chat-history-dark-tooltip.el-popper .el-popper__arrow::before,
html.light[data-theme-id="custom-scene"] .chat-history-dark-tooltip.el-popper .el-popper__arrow::before {
  background: #fff !important;
  border-color: #fff !important;
}
.chat-history-dark-tooltip.el-popper[style*="translate3d(0px, 0px"],
.chat-history-dark-tooltip.el-popper[style*="translate(0px, 0px"],
.chat-history-dark-tooltip.el-popper[style*="left: 0px"][style*="top: 0px"]:not([style*="translate"]) {
  visibility: hidden !important;
  opacity: 0 !important;
}

/* 场景主题（除雾灰调外）→ 历史会话图标白色 */
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .conv-img-icons,
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .conv-mode-icon,
[data-theme-id="custom-scene"] .conv-img-icons,
[data-theme-id="custom-scene"] .conv-mode-icon { color: #fff !important; }

/* 情景主题（除雾灰调外）→ 历史会话 conversation-status 动画/未读点白色 */
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .conversation-status,
[data-theme-id="custom-scene"] .conversation-status {
  color: #fff !important;
}
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .conversation-status-dot,
[data-theme-id="custom-scene"] .conversation-status-dot {
  background: #fff !important;
}

.md-editor-preview .agent-opening-suggestions {
  margin-top: 10px;
}

.md-editor-preview .agent-opening-suggestions-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.md-editor-preview .agent-opening-suggestion {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  box-sizing: border-box;
  padding: 14px 18px;
  border-radius: 12px;
  background: #f5f6f8;
  color: var(--text-primary);
  font-size: 15px;
  line-height: 1.5;
  word-break: break-word;
  cursor: pointer;
  transition: background 0.15s ease, transform 0.15s ease;
}

.md-editor-preview .agent-opening-suggestion:hover {
  background: #eef1f5;
  transform: translateY(-1px);
}

.md-editor-preview .agent-opening-suggestion-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 20px;
  width: 20px;
  height: 20px;
}

.md-editor-preview .agent-opening-suggestion-icon::before {
  content: "";
  display: block;
  width: 18px;
  height: 18px;
  background: #5ca8df;
  mask: url("data:image/svg+xml,%3Csvg viewBox='0 0 24 24' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M12 20h9' stroke='black' stroke-width='2' fill='none' stroke-linecap='round' stroke-linejoin='round'/%3E%3Cpath d='M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z' stroke='black' stroke-width='2' fill='none' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E") center / contain no-repeat;
  -webkit-mask: url("data:image/svg+xml,%3Csvg viewBox='0 0 24 24' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M12 20h9' stroke='black' stroke-width='2' fill='none' stroke-linecap='round' stroke-linejoin='round'/%3E%3Cpath d='M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z' stroke='black' stroke-width='2' fill='none' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E") center / contain no-repeat;
}

.md-editor-preview .agent-opening-suggestion-text {
  font-weight: 600;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .md-editor-preview .agent-opening-suggestion,
html.light[data-theme-id="custom-scene"] .md-editor-preview .agent-opening-suggestion {
  background: rgba(255, 255, 255, 0.14) !important;
  color: #ffffff !important;
}

html.dark .md-editor-preview .agent-opening-suggestion {
  background: rgba(255, 255, 255, 0.08);
}

/* 情景主题（除雾灰调）下提示词面板搜索框样式 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-panel .el-input__inner {
  color: #fff !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-panel .el-input__icon {
  color: #fff !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-panel .el-input__wrapper {
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.3) inset !important;
}

/* 输入框提示文字（placeholder） */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-panel .el-input__inner::placeholder {
  color: rgba(255, 255, 255, 0.7) !important;
}

/* 情景主题（除雾灰调）下提示词面板 tabs 未选中标签 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-panel .el-tabs__item:not(.is-active) {
  color: #fff !important;
}

/* 提示词面板搜索框和新建按钮高度统一 */
.prompt-panel .el-input--small .el-input__wrapper,
.prompt-panel .el-button--small {
  height: 32px !important;
}

.prompt-panel .el-button--small {
  padding: 0 16px !important;
}

/* 提示词面板 tabs 选中下划线颜色随主题 */
.prompt-panel .el-tabs__active-bar {
  background-color: var(--color-primary) !important;
}

/* 提示词面板添加/使用按钮样式与新建对话按钮一样随主题改变 */
.prompt-panel .prompt-actions .el-button--text:not(.el-button--danger),
.prompt-panel .prompt-actions .el-button--small.is-text:not(.el-button--danger) {
  background: var(--btn-primary-bg, var(--color-primary)) !important;
  border-color: var(--btn-primary-border, var(--color-primary)) !important;
  color: var(--btn-primary-text, #fff) !important;
  padding: 4px 12px !important;
  border-radius: 4px !important;
}

.prompt-panel .prompt-actions .el-button--text:not(.el-button--danger):hover,
.prompt-panel .prompt-actions .el-button--small.is-text:not(.el-button--danger):hover {
  background: var(--btn-primary-hover-bg, var(--color-primary)) !important;
  border-color: var(--btn-primary-hover-border, var(--color-primary)) !important;
  color: var(--btn-primary-hover-text, #fff) !important;
}

/* 提示词面板删除按钮：红色背景，白色文字 */
.prompt-panel .prompt-actions .el-button--danger {
  background: var(--color-danger) !important;
  border-color: var(--color-danger) !important;
  color: #fff !important;
  padding: 4px 12px !important;
  border-radius: 4px !important;
}

.prompt-panel .prompt-actions .el-button--danger:hover {
  background: var(--color-danger-light, var(--color-danger)) !important;
  border-color: var(--color-danger-light, var(--color-danger)) !important;
  color: #fff !important;
}

/* 情景主题（除雾灰调）下提示词面板分类标签选中/悬浮样式 */
/* html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-panel .cat-tag.active {
  background: var(--color-primary) !important;
  color: #fff !important;
  border-color: var(--color-primary) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-panel .cat-tag:hover {
  background: var(--color-primary) !important;
  color: #fff !important;
  border-color: var(--color-primary) !important;
} */

/* 提示词面板新建按钮：与新建对话按钮样式统一，跟随主题 */
.prompt-panel .el-button--primary {
  background: var(--btn-primary-bg, var(--color-primary)) !important;
  border-color: var(--btn-primary-border, var(--color-primary)) !important;
  color: var(--btn-primary-text, #fff) !important;
}

.prompt-panel .el-button--primary:hover {
  background: var(--btn-primary-hover-bg, var(--color-primary)) !important;
  border-color: var(--btn-primary-hover-border, var(--color-primary)) !important;
  color: var(--btn-primary-hover-text, #fff) !important;
}

/* 
&.active {
    border-color: var(--color-primary);
    color: var(--color-primary);
    background: color-mix(in srgb, var(--color-primary) 8%, transparent);
  }

  &:hover {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }
   */
/* 情景主题（除雾灰调）下 panel-cats 选中按钮样式 */
html.dark[data-theme-id="simple-dark"] .prompt-panel .cat-tag.active,
html.light[data-theme-id="simple-light"] .prompt-panel .cat-tag.active {
  border-color: var(--color-primary);
  color: var(--color-primary);
  background: color-mix(in srgb, var(--color-primary) 8%, transparent);
}

/* 情景主题（除雾灰调）下 panel-cats 按钮悬浮样式 */
html.dark[data-theme-id="simple-dark"] .prompt-panel .cat-tag:hover html.light[data-theme-id="simple-light"] .prompt-panel .cat-tag:hover {
  border-color: var(--color-primary);
  color: var(--color-primary);
}

/* 雾灰调主题下新建按钮 */
html.light[data-theme-id="scene-graphite-cyan"] .prompt-panel .el-button--primary {
  background: var(--color-primary) !important;
  border-color: var(--color-primary) !important;
  color: #fff !important;
}

/* 情景主题（除雾灰调）下 panel-cats 选中按钮样式 */
/* html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-panel .cat-tag.active {
  background: var(--color-primary) !important;
  border-color: var(--color-primary) !important;
  color: #fff !important;
} */

/* 情景主题（除雾灰调）下 panel-cats 按钮悬浮样式 */
/* html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-panel .cat-tag:hover {
  background: var(--color-primary) !important;
  border-color: var(--color-primary) !important;
  color: #fff !important;
} */

/* 雾灰调主题下 panel-cats 选中按钮样式（保持默认） */
/* html.light[data-theme-id="scene-graphite-cyan"] .prompt-panel .cat-tag.active {
  background: color-mix(in srgb, var(--color-primary) 8%, transparent) !important;
  border-color: var(--color-primary) !important;
  color: var(--color-primary) !important;
} */

/* 新建提示词弹窗：与知识库创建弹窗样式统一，随主题变化 */
.prompt-create-dialog {
  border-radius: 18px !important;
  overflow: hidden !important;
}

.prompt-create-dialog .el-dialog__header {
  padding: 18px 24px !important;
  border-bottom: 1px solid var(--border-color) !important;
  margin-right: 0 !important;
}

.prompt-create-dialog .el-dialog__title {
  font-size: 17px !important;
  font-weight: 600 !important;
}

.prompt-create-dialog .el-dialog__body {
  padding: 20px 24px !important;
}

.prompt-create-dialog .el-dialog__footer {
  padding: 14px 24px !important;
  border-top: 1px solid var(--border-color) !important;
}

/* 情景/暗色主题（除雾灰调）下弹窗适配 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-create-dialog,
html.dark .prompt-create-dialog {
  background: var(--theme-dailog-color) !important;
  border: 1px solid rgba(255, 255, 255, 0.15) !important;
  color: #fff !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-create-dialog .el-dialog__title,
html.dark .prompt-create-dialog .el-dialog__title {
  color: #fff !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-create-dialog .el-form-item__label,
html.dark .prompt-create-dialog .el-form-item__label {
  color: rgba(255, 255, 255, 0.85) !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-create-dialog .el-input__wrapper,
html.dark .prompt-create-dialog .el-input__wrapper {
  background: rgba(255, 255, 255, 0.1) !important;
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.2) inset !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .prompt-create-dialog .el-input__inner,
html.dark .prompt-create-dialog .el-input__inner {
  color: #fff !important;
}

/* 情景主题（除雾灰调）下 mcp/ws 触发器按钮 hover 背景主题色、图标白色 */
/* html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-trigger:hover,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .ws-trigger:hover {
  background: var(--color-primary) !important;
  color: #fff !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-trigger:hover svg,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-trigger:hover svg *,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .ws-trigger:hover svg,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .ws-trigger:hover svg * {
  fill: currentColor !important;
  stroke: currentColor !important;
} */

/* 情景主题（除雾灰调）下 btn-left 等深色 tooltip 文字为黑色 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-popper.is-dark {
  color: #000 !important;
}


/* 欢迎区 skill-chip 前置图标（覆盖 path 上的 fill="#333"） */
.welcome-skills .skill-chip svg,
.welcome-skills .skill-chip svg path {
  color: var(--text-primary);
  fill: currentColor !important;
}

/* textarea placeholder 颜色比正常文字浅（用 text-primary 半透明混合背景） */
.el-textarea__inner::placeholder,
.el-textarea__inner::-webkit-input-placeholder {
  color: color-mix(in srgb, var(--text-primary) 38%, transparent) !important;
}

/* 暗色/情景主题下切换模型弹窗 的 item鼠标悬浮背景颜色 */
html.dark .el-collapse-item.is-active .el-collapse-item__wrap .el-collapse-item__content .model-box .model-item:hover,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-collapse-item.is-active .el-collapse-item__wrap .el-collapse-item__content .model-box .model-item:hover {
  background: rgba(255, 255, 255, 0.10) !important;
}

html.dark .quote-ref-bar,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .quote-ref-bar {
  background: rgba(255, 255, 255, 0.10) !important;
}

/* ai回答区域的下载按钮的弹窗文字颜色 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .dropdown-item {
  color: #fff;
}

/* 仅覆盖无类型默认 tag；success/warning 等语义标签保持 Element Plus 原色 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-tag:not(.el-tag--success):not(.el-tag--warning):not(.el-tag--danger):not(.el-tag--info):not(.el-tag--primary) {
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: #ffffff;
}

/* 主聊天模型下拉框内的能力标签：更淡的浅色底 + 柔和主题色图标 */
.main-chat-select-popper .el-tag {
  background: color-mix(in srgb, var(--color-primary) 5%, #ffffff) !important;
  border: 1px solid color-mix(in srgb, var(--color-primary) 8%, transparent) !important;
  color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
  border-radius: 6px;
}

.el-select-dropdown .scroll-box .el-tag {
  color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
}

.main-chat-select-popper .el-tag .el-icon,
.main-chat-select-popper .el-tag svg,
.main-chat-select-popper .el-tag svg * {
  color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
  fill: currentColor !important;
}

/* 暗色 / 情景主题（除雾灰调）：保持浅色胶囊感，略提高对比 */
html.dark .main-chat-select-popper .el-tag,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .main-chat-select-popper .el-tag {
  background: color-mix(in srgb, var(--color-primary) 12%, transparent) !important;
  border: 1px solid color-mix(in srgb, var(--color-primary) 16%, transparent) !important;
  color: color-mix(in srgb, var(--color-primary) 60%, #ffffff) !important;
}

html.dark .main-chat-select-popper .el-tag .el-icon,
html.dark .main-chat-select-popper .el-tag svg,
html.dark .main-chat-select-popper .el-tag svg *,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .main-chat-select-popper .el-tag .el-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .main-chat-select-popper .el-tag svg,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .main-chat-select-popper .el-tag svg * {
  color: color-mix(in srgb, var(--color-primary) 60%, #ffffff) !important;
  fill: currentColor !important;
}

.main-chat-select-popper .el-select-dropdown__wrap {
  max-height: calc(68vh - 53px);
}

/* 模型下拉需高于 mxcs 详情面板，避免汇智网等带 mxcs 的项无法点击 */
.main-chat-select-popper.el-popper {
  z-index: 2002 !important;
}

/* mxcs 模型详情面板：与下拉框样式一致；仅展示，不拦截下拉项点击 */
.mxcs-fixed-side-panel {
  background: var(--theme-dailog-color) !important;
  color: var(--el-text-color-regular, var(--text-primary));
  border-radius: 8px;
  box-shadow: var(--el-box-shadow-light, 0 6px 16px rgba(0, 0, 0, 0.18));
  padding: 12px;
  overflow-y: auto;
  font-size: var(--font-size-lg) !important;
  pointer-events: none;
}

.mxcs-fixed-side-panel * {
  font-size: var(--font-size-sm) !important;
}

/* 情景主题（除雾灰调）：与下拉框一致的半透明毛玻璃 + 白字。
   面板内容由 v-html 渲染并继承容器 color，故只需在容器上设置颜色与边框变量。 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mxcs-fixed-side-panel,
html.light[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .mxcs-fixed-side-panel {
  background: rgba(255, 255, 255, 0.10);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  color: #ffffff;
  --border-color: rgba(255, 255, 255, 0.18);
}

.md-editor-preview.github-theme {
  --md-theme-color: #000;
  color: var(--text-primary) !important;
}

/* 
html.light[data-theme-id="scene-graphite-cyan"] .md-editor-preview.github-theme {
  --md-theme-color: #000;
  color: #000 !important;
} */

html.light[data-theme-id="scene-graphite-cyan"] .md-editor-code-head,
html.light[data-theme-id="scene-graphite-cyan"] [class*="language-"] {
  background: var(--table-header-bg-color) !important;
}
html.light[data-theme-id="scene-graphite-cyan"] [class*="language-"] {
  background-color: #f3f3f4 !important;
}
.md-editor-code-head
{
  background: var(--table-header-bg-color) !important;
  border-bottom: 1px solid var(--border-color) !important;
}
.md-editor-collapse-tips,
.md-editor-copy-button,
.md-editor-code-lang
{
  color:var(--text-secondary);
}

/* 代码区域 */
/* .github-theme .md-editor-code {
    background: rgba(0,0,0,0.05) !important;

}  */

.md-editor-preview code {
  direction: ltr;
  color: var(--text-primary);
  background-color: var(--dropdown-hover-color);
  border-radius: var(--md-theme-code-inline-radius);
  padding: 2px 4px;
  line-height: 22px;
}

.md-editor-preview.github-theme h3,
.md-editor-preview.github-theme h1,
.md-editor-preview.github-theme h2,
.md-editor-preview.github-theme h4,
.md-editor-preview.github-theme h5,
.md-editor-preview.github-theme h6 {
  color: var(--text-primary) !important;
}

/* html.light[data-theme-id="scene-graphite-cyan"] .md-editor-preview.github-theme h3,
html.light[data-theme-id="scene-graphite-cyan"] .md-editor-preview.github-theme h1,
html.light[data-theme-id="scene-graphite-cyan"] .md-editor-preview.github-theme h2,
html.light[data-theme-id="scene-graphite-cyan"] .md-editor-preview.github-theme h4,
html.light[data-theme-id="scene-graphite-cyan"] .md-editor-preview.github-theme h5,
html.light[data-theme-id="scene-graphite-cyan"] .md-editor-preview.github-theme h6 {
  color: #000 !important;
}  */

.el-select-dropdown .scroll-box .el-tag,
.el-select-dropdown .scroll-box .el-tag.el-tag--primary,
.el-select-dropdown .scroll-box .el-tag .el-tag__content {
  color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
}

.el-select-dropdown__header .el-input__wrapper {
  width: 100% !important;
  background: transparent !important;
  box-shadow: none !important;
}

/* 仅改眼睛图标 /「高级」文字色，与下拉列表文字一致 */
.main-chat-select-popper .el-select-dropdown__header .el-button:not(.el-button--warning),
.main-chat-select-popper .el-select-dropdown__header .el-button:not(.el-button--warning) span,
.main-chat-select-popper .el-select-dropdown__header .el-button:not(.el-button--warning) .el-icon,
.main-chat-select-popper .el-select-dropdown__header .el-button:not(.el-button--warning) svg {
  color: var(--text-color-light-black, var(--text-primary)) !important;
  --el-button-text-color: var(--text-color-light-black, var(--text-primary)) !important;
  fill: currentColor !important;
}

/* 仅改搜索框占位符：比列表正常文字略浅 */
.main-chat-select-popper .el-select-dropdown__header .el-input__inner::placeholder,
.main-chat-select-popper .el-select-dropdown__header .el-input__inner::-webkit-input-placeholder {
  color: color-mix(in srgb, var(--text-color-light-black, var(--text-primary)) 55%, transparent) !important;
  -webkit-text-fill-color: color-mix(in srgb, var(--text-color-light-black, var(--text-primary)) 55%, transparent) !important;
  opacity: 1 !important;
}

/* 仅改分类标题（豆包 / 本电脑等）：与列表项文字同色 */
.main-chat-select-popper .el-select-group__title {
  color: var(--text-color-light-black, var(--text-primary)) !important;
}

/* 情景主题：模型名/左侧图标与供应商标题同为浅色（压过 useLog 历史黑字规则） */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .main-chat-select-popper .el-select-dropdown__item,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .main-chat-select-popper .el-select-dropdown__item .model-item,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .main-chat-select-popper .el-select-dropdown__item .model-item-title,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .main-chat-select-popper .el-select-dropdown__item .el-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .main-chat-select-popper .el-select-dropdown__item .el-icon svg,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .main-chat-select-popper .el-select-dropdown__item .el-icon svg * {
  color: #fff !important;
  fill: currentColor !important;
}

html.dark .main-chat-select-popper .el-select-dropdown__item,
html.dark .main-chat-select-popper .el-select-dropdown__item .model-item-title,
html.dark .main-chat-select-popper .el-select-dropdown__item .el-icon,
html.dark .main-chat-select-popper .el-select-dropdown__item .el-icon svg,
html.dark .main-chat-select-popper .el-select-dropdown__item .el-icon svg * {
  color: var(--text-primary, #fff) !important;
  fill: currentColor !important;
}

/* ModelSelector 下拉框内 tag：更淡的浅色底 + 柔和主题色 */
.model-selector-popper .el-tag,
.model-selector-popper .el-tag.el-tag--primary,
.model-selector-popper .el-tag .el-tag__content {
  background: color-mix(in srgb, var(--color-primary) 5%, #ffffff) !important;
  border: 1px solid color-mix(in srgb, var(--color-primary) 8%, transparent) !important;
  color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
}

.model-selector-popper .el-tag .el-icon,
.model-selector-popper .el-tag svg,
.model-selector-popper .el-tag svg * {
  color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
  fill: currentColor !important;
}

/* 编辑对话弹窗样式 */
.edit-conv-message-box .el-input__wrapper,
.chat-conv-message-box .el-input__wrapper {
  background: transparent !important;
  box-shadow: none !important;
  border: 1px solid var(--border-color) !important;
}

/* 编辑对话输入框：选中高亮用浅主题色，避免深蓝盖住文字 */
.chat-conv-message-box .el-message-box__input input::selection,
.chat-conv-message-box .el-input__inner::selection,
.chat-conv-message-box input::selection {
  background: color-mix(in srgb, var(--color-primary) 28%, transparent) !important;
  color: var(--text-primary) !important;
}
.chat-conv-message-box .el-message-box__input input::-moz-selection,
.chat-conv-message-box .el-input__inner::-moz-selection,
.chat-conv-message-box input::-moz-selection {
  background: color-mix(in srgb, var(--color-primary) 28%, transparent) !important;
  color: var(--text-primary) !important;
}

.theme-action {
  min-width: 76px;
  height: 36px;
  padding: 0 16px;
  border: 1px solid transparent;
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-base);
  font-weight: 600;
  transition: all 0.18s ease;
}

.theme-action--secondary {
  color: var(--btn-secondary-text) !important;
  background: var(--cancel-btn-color) !important;
  border-color: var(--btn-secondary-border) !important;
}

.theme-action--secondary:hover {
  background: var(--cancel-btn-hover-color) !important;
  color: var(--btn-secondary-hover-text) !important;
  border-color: var(--btn-secondary-hover-border) !important;
}
.btn-qx {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 16px;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: 1px solid var(--btn-primary-border, transparent);
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-md);
  font-weight: 500;
  transition: all 0.2s;


}

.btn-qx:hover {
  background: var(--btn-primary-hover-bg, var(--color-primary)) !important;
  color: var(--btn-primary-hover-text, #fff) !important;
  border-color: var(--btn-primary-hover-border, transparent) !important;
}

.theme-action--primary {
  background: var(--btn-primary-hover-bg, var(--color-primary)) !important;
  color: var(--btn-primary-hover-text, #fff) !important;
  border-color: var(--btn-primary-hover-border, transparent) !important;
}

.theme-action--primary:hover {
  background: var(--btn-primary-hover-bg, var(--color-primary)) !important;
  color: var(--btn-primary-hover-text, #fff) !important;
  border-color: var(--btn-primary-hover-border, transparent) !important;
}

/* 对话附件图片预览（el-image-viewer，teleport 到 body）：
   黑暗主题 + 情景主题（除雾灰调）下，关闭/翻页/缩放旋转按钮图标为黑色；
   不影响明亮主题与雾灰调 */

/* 预览图四周留白，避免贴满程序窗口；关闭钮/底栏位置不变，缩放旋转仍可用 */
.el-image-viewer__canvas {
  box-sizing: border-box;
  padding: 48px 56px 72px;
}

/* 问题气泡图片（md-editor medium-zoom）放大预览：窗口居中 + 四周留边；--fc-medium-zoom-scale 供滚轮缩放 */
body.medium-zoom--opened .medium-zoom-image--opened {
  position: fixed !important;
  inset: auto !important;
  left: 50% !important;
  top: 50% !important;
  width: auto !important;
  height: auto !important;
  max-width: calc(100vw - 96px) !important;
  max-height: calc(100vh - 96px) !important;
  object-fit: contain !important;
  transform: translate(-50%, -50%) scale(var(--fc-medium-zoom-scale, 1)) !important;
  transform-origin: center center !important;
}

html.dark .el-image-viewer__wrapper .el-image-viewer__btn,
html.dark .el-image-viewer__wrapper .el-image-viewer__btn .el-icon,
html.dark .el-image-viewer__wrapper .el-image-viewer__btn svg,
html.dark .el-image-viewer__wrapper .el-image-viewer__actions,
html.dark .el-image-viewer__wrapper .el-image-viewer__actions .el-icon,
html.dark .el-image-viewer__wrapper .el-image-viewer__actions svg,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-image-viewer__wrapper .el-image-viewer__btn,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-image-viewer__wrapper .el-image-viewer__btn .el-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-image-viewer__wrapper .el-image-viewer__btn svg,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-image-viewer__wrapper .el-image-viewer__actions,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-image-viewer__wrapper .el-image-viewer__actions .el-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-image-viewer__wrapper .el-image-viewer__actions svg {
  color: #000000 !important;
  fill: currentColor !important;
}

html.dark .el-image-viewer__wrapper .el-image-viewer__btn svg *,
html.dark .el-image-viewer__wrapper .el-image-viewer__actions svg *,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-image-viewer__wrapper .el-image-viewer__btn svg *,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-image-viewer__wrapper .el-image-viewer__actions svg * {
  stroke: currentColor !important;
  fill: currentColor !important;
}

.welcome-screen--workspace {
  position: absolute;
  inset: 0;
  z-index: 1;
  height: auto;
  min-height: 0;
  padding-bottom: 180px;
  overflow-y: auto;
  pointer-events: none;

  .skill-chip {
    pointer-events: auto;
  }
}

.fastplan-workspace-welcome {
  position: absolute;
  inset: 0 0 150px;
  z-index: 1;
  height: auto;
  min-height: 0;
  overflow: hidden;
  pointer-events: none;
}

/* chat-renderer 宿主桥：全部颜色继续读取 FastClaw 运行时主题变量。 */
.fastclaw-chat-workspace {
  min-width: 0;
  min-height: 0;
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: transparent !important;
  --jcr-bg: transparent !important;
  --jcr-surface: transparent !important;
  --jcr-subtle-bg: transparent !important;
}
.fastclaw-chat-workspace.jcr-workspace,
.fastclaw-chat-workspace .jcr-workspace,
.fastclaw-chat-workspace .jcr-conversation,
.fastclaw-chat-workspace .jcr-timeline,
.fastclaw-chat-workspace .jcr-panel {
  background: transparent !important;
  --jcr-bg: transparent !important;
  --jcr-surface: transparent !important;
  --jcr-subtle-bg: transparent !important;
}
.fastclaw-chat-workspace .jcr-workspace,
.fastclaw-chat-workspace .jcr-conversation {
  min-height: 0 !important;
  flex: 1 1 auto !important;
  height: 100% !important;
  max-height: 100%;
  overflow: hidden !important;
}
.fastclaw-chat-workspace .jcr-workspace__body,
.fastclaw-chat-workspace .jcr-conversation__content,
.fastclaw-chat-workspace .jcr-conversation__main {
  min-height: 0 !important;
  flex: 1 1 auto !important;
  overflow: hidden !important;
}
/* 强制 timeline 成为唯一纵向滚动容器，避免内容撑开后被外层裁切却滚不动 */
.fastclaw-chat-workspace .jcr-conversation__main > .jcr-timeline {
  flex: 1 1 auto !important;
  min-height: 0 !important;
  height: 100% !important;
  max-height: 100% !important;
  overflow-x: hidden !important;
  overflow-y: auto !important;
}
.fastclaw-chat-workspace .jcr-state.jcr-state--empty {
  color: var(--text-primary) !important;
}
.fastclaw-chat-workspace .jcr-conversation__loading,
.fastclaw-chat-workspace .jcr-state,
.fastclaw-chat-workspace .jcr-state--empty {
  background: transparent !important;
}
.fastclaw-chat-workspace .jcr-conversation__scroll {
  padding: 16px 45px;
}
.fastclaw-chat-workspace .jcr-conversation__footer {
  padding: 10px 20px 14px;
  background: transparent !important;
}
.fastclaw-chat-workspace.is-input-resized .jcr-conversation__footer {
  height: auto;
  min-height: var(--fastclaw-composer-height);
  display: flex;
  flex-direction: column;
}
.fastclaw-chat-workspace.is-input-resized .jcr-conversation__footer > :not(.jcr-composer) {
  flex: 0 0 auto;
}
.fastclaw-chat-workspace .jcr-composer {
  width: 100%;
  margin: 0 auto;
  background: transparent !important;
  padding: 8px 0 16px;
}
.fastclaw-chat-workspace.is-input-resized .jcr-composer {
  height: auto;
  min-height: 0;
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
}
.fastclaw-chat-workspace.is-input-resized .jcr-composer__box {
  height: auto;
  min-height: 0;
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.fastclaw-chat-workspace.is-input-resized .jcr-composer__selectors,
.fastclaw-chat-workspace.is-input-resized .jcr-composer__attachments,
.fastclaw-chat-workspace.is-input-resized .jcr-composer__toolbar {
  flex: 0 0 auto;
}

/* Workspace 输入区图片附件：边框跟随主题 --border-color（覆盖 chat-renderer 默认） */
.fastclaw-chat-workspace .jcr-composer-attachment.jcr-composer-attachment--image {
  border-color: var(--border-color) !important;
}

/*
 * 情景主题（除雾灰调）：图片附件上传中的黑色底
 * 原因：宿主把 --jcr-bg 设为 transparent 后，chat-renderer 的
 * color-mix(..., var(--jcr-bg)) 会按「透明黑」混色，整块缩略图变成近黑。
 * 这里强制卡片 / visual / el-image / spinner 背景透明。
 */
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-composer-attachment.jcr-composer-attachment--image,
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-composer-attachment.jcr-composer-attachment--image[data-status="uploading"],
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-composer-attachment.jcr-composer-attachment--image[data-status="processing"],
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-composer-attachment.jcr-composer-attachment--image[data-status="complete"],
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-composer-attachment.jcr-composer-attachment--image[data-status="error"],
[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-composer-attachment.jcr-composer-attachment--image,
[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-composer-attachment.jcr-composer-attachment--image[data-status="uploading"],
[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-composer-attachment.jcr-composer-attachment--image[data-status="processing"],
[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-composer-attachment.jcr-composer-attachment--image[data-status="complete"],
[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-composer-attachment.jcr-composer-attachment--image[data-status="error"] {
  background: transparent !important;
  box-shadow: none !important;
}
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-composer-attachment--image .jcr-composer-attachment__visual,
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-composer-attachment--image .workspace-composer-att-img,
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-composer-attachment--image .el-image,
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-composer-attachment--image .el-image__wrapper,
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-composer-attachment--image .el-image__placeholder,
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-composer-attachment--image .jcr-composer-attachment__spinner,
[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-composer-attachment--image .jcr-composer-attachment__visual,
[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-composer-attachment--image .workspace-composer-att-img,
[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-composer-attachment--image .el-image,
[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-composer-attachment--image .el-image__wrapper,
[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-composer-attachment--image .el-image__placeholder,
[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-composer-attachment--image .jcr-composer-attachment__spinner {
  background: transparent !important;
  box-shadow: none !important;
}

/* Workspace 输入区图片附件：el-image 对齐默认缩略图尺寸，点击可放大 */
.fastclaw-chat-workspace .jcr-composer-attachment--image .workspace-composer-att-img {
  width: 66px;
  height: 66px;
  border-radius: 7px;
  cursor: zoom-in;
  display: block;
  overflow: hidden;
}
/* 与问题气泡图片一致（.jcr-markdown--image img 为 zoom-in）；压过 el-image__preview 的 pointer */
.fastclaw-chat-workspace .jcr-composer__attachments .jcr-composer-attachment--image,
.fastclaw-chat-workspace .jcr-composer__attachments .jcr-composer-attachment--image .workspace-composer-att-img,
.fastclaw-chat-workspace .jcr-composer__attachments .jcr-composer-attachment--image .el-image__inner,
.fastclaw-chat-workspace .jcr-composer__attachments .jcr-composer-attachment--image .el-image__preview {
  cursor: zoom-in !important;
}
.fastclaw-chat-workspace .jcr-composer__attachments .jcr-composer-attachment--image > .jcr-composer-attachment__remove {
  cursor: pointer !important;
}
.fastclaw-chat-workspace .jcr-composer-attachment--image .workspace-composer-att-img .el-image__inner {
  width: 66px;
  height: 66px;
  object-fit: cover;
  border-radius: 7px;
}
.fastclaw-chat-workspace .jcr-composer-attachment--image .workspace-composer-att-img-error {
  width: 66px;
  height: 66px;
  border-radius: 7px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--jcr-subtle-bg, var(--bg-hover));
  color: var(--jcr-muted, var(--text-secondary));
  font-size: 11px;
  text-align: center;
  box-sizing: border-box;
  padding: 4px;
}
.fastclaw-chat-workspace .jcr-composer-attachment--image > .jcr-composer-attachment__remove {
  z-index: 2;
}
.fastclaw-chat-workspace.is-input-resized .jcr-composer__input-control {
  min-height: 44px;
  flex: 1 1 44px;
  flex-direction: column;
  overflow: hidden;
}
.fastclaw-chat-workspace.is-input-resized .jcr-composer textarea {
  min-height: 44px;
  max-height: none;
  flex: 1 1 44px;
  overflow-y: auto;
}
.fastclaw-chat-workspace .jcr-composer__box {
  border-color: var(--border-color);
  border-radius: 20px;
  background: var(--chat-input-skill-chip) !important;
  box-shadow: 0 2px 12px color-mix(in srgb, var(--text-primary) 8%, transparent);
}
.fastclaw-chat-workspace .jcr-composer__selectors {
  position: relative;
  padding-right: 16px;
  padding-left: 16px;
  padding-bottom: 4px;
  border-bottom: 0;
}
.fastclaw-chat-workspace .jcr-composer__selectors::after {
  content: "";
  position: absolute;
  right: 16px;
  bottom: 0;
  left: 16px;
  height: 1px;
  background: var(--border-color);
}
.fastclaw-chat-workspace .fastclaw-workspace-selectors { width: 100%; padding: 0; }
.fastclaw-chat-workspace .fastclaw-workspace-selectors .selectors-left {
  align-items: center;
}
.fastclaw-chat-workspace .fastclaw-workspace-selectors .model-selector .el-select__selected-item {
  padding-left: 0;
}
.fastclaw-chat-workspace .fastclaw-workspace-selectors .model-selector .el-select__wrapper,
.fastclaw-chat-workspace .fastclaw-workspace-selectors .model-selector .el-select__prefix,
.fastclaw-chat-workspace .fastclaw-workspace-selectors .model-selector .el-select__selection,
.fastclaw-chat-workspace .fastclaw-workspace-selectors .model-selector .el-select__selected-item {
  align-items: center;
}
.fastclaw-chat-workspace .fastclaw-workspace-selectors .model-selector-prefix {
  display: inline-flex;
  height: 24px;
  align-items: center;
  justify-content: center;
  line-height: 1;
}
.fastclaw-chat-workspace .fastclaw-workspace-selectors .model-selector-prefix .el-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  line-height: 1;
  transform: translateY(1px);
}
.fastclaw-chat-workspace .fastclaw-workspace-selectors .model-selector-prefix img,
.fastclaw-chat-workspace .fastclaw-workspace-selectors .model-selector-prefix svg {
  display: block;
}
.fastclaw-chat-workspace .jcr-composer textarea {
  padding-right: 16px;
  padding-left: 16px;
}
.fastclaw-chat-workspace .jcr-composer textarea::placeholder {
  color: color-mix(in srgb, var(--text-primary) 38%, transparent);
  opacity: 1;
}
.fastclaw-chat-workspace .jcr-composer__toolbar {
  padding-right: 15px;
  padding-left: 15px;
}
.fastclaw-workspace-resize-handle {
  position: absolute;
  top: 0;
  right: 16px;
  left: 16px;
  z-index: 25;
  height: 10px;
  cursor: ns-resize;
  touch-action: none;
}
/* FastPlan 任务列表显示时仍隐藏拖拽条；提示栏和引用栏不会再影响其定位。 */
.fastclaw-chat-workspace:has(.fastplan-composer-contribution-target .jcr-plan-tasks) .fastclaw-workspace-resize-handle {
  display: none;
}
.fastclaw-chat-workspace .jcr-composer__box:has(> .fastclaw-workspace-resize-handle) {
  padding-top: 6px;
}
.fastclaw-workspace-resize-handle::after {
  content: "";
  position: absolute;
  top: 4px;
  left: 50%;
  width: 40px;
  height: 3px;
  border-radius: 999px;
  background: var(--border-color);
  opacity: .65;
  transform: translateX(-50%);
  transition: background .15s, opacity .15s;
}
.fastclaw-workspace-resize-handle:hover::after,
.fastclaw-chat-workspace.is-input-resizing .fastclaw-workspace-resize-handle::after {
  background: var(--color-primary);
  opacity: 1;
}
.fastclaw-chat-workspace .jcr-composer__actions > button,
.fastclaw-chat-workspace .jcr-composer__actions > .screenshot-group > button:first-child {
  width: 30px;
  height: 30px;
  padding: 0;
}
.fastclaw-chat-workspace .jcr-composer__actions {
  gap: 2px;
}
.fastclaw-chat-workspace .jcr-composer__actions > .screenshot-group > .hide-win-btn {
  width: 16px;
  min-width: 16px;
  height: 28px;
  padding: 0 2px;
}
.fastclaw-chat-workspace .jcr-composer__submit {
  width: 36px !important;
  height: 36px !important;
  flex: 0 0 36px;
}

.fastclaw-chat-workspace .jcr-composer__submit {
  border-radius: 50%;
  /* 平时：系统主题主色（不改 chat-plan-packages） */
  color: #fff !important;
  background: var(--btn-primary-bg, var(--color-primary)) !important;
}
.fastclaw-chat-workspace .jcr-composer__submit:hover:not(:disabled),
.fastclaw-chat-workspace .jcr-composer__submit:focus-visible:not(:disabled) {
  background: var(--btn-primary-hover-bg, var(--color-primary)) !important;
  outline: none;
}
.fastclaw-chat-workspace .jcr-composer__submit:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

/* 问题目录：目录按钮 / 弹窗主题色 */
.fastclaw-chat-workspace .jcr-question-nav__directory:hover,
.fastclaw-chat-workspace .jcr-question-nav__directory:focus-visible,
.fastclaw-chat-workspace .jcr-question-nav__directory[aria-expanded="true"] {
  background: var(--dropdown-hover-color) !important;
  box-shadow: none !important;
}
.fastclaw-chat-workspace .jcr-question-nav__panel,
.fastclaw-chat-workspace [id^="jcr-question-directory-"] {
  background: var(--theme-dailog-color) !important;
  border-color: var(--border-color) !important;
  color: var(--text-primary) !important;
  /* 与主题选择弹窗 .theme-modal__body 滚动条一致 */
  scrollbar-width: thin;
  scrollbar-color: var(--border-color) transparent;
}
.fastclaw-chat-workspace .jcr-question-nav__panel::-webkit-scrollbar,
.fastclaw-chat-workspace [id^="jcr-question-directory-"]::-webkit-scrollbar {
  width: 6px;
}
.fastclaw-chat-workspace .jcr-question-nav__panel::-webkit-scrollbar-track,
.fastclaw-chat-workspace [id^="jcr-question-directory-"]::-webkit-scrollbar-track {
  background: transparent;
}
.fastclaw-chat-workspace .jcr-question-nav__panel::-webkit-scrollbar-thumb,
.fastclaw-chat-workspace [id^="jcr-question-directory-"]::-webkit-scrollbar-thumb {
  background: var(--border-color);
  border-radius: 999px;
}
.fastclaw-chat-workspace .jcr-question-nav__panel-item:hover,
.fastclaw-chat-workspace .jcr-question-nav__panel-item:focus-visible {
  background: var(--dropdown-hover-color) !important;
  color: var(--text-primary) !important;
}
.fastclaw-chat-workspace .fastclaw-submit-icon {
  width: 18px;
  height: 18px;
}
.fastclaw-chat-workspace .fastclaw-abort-icon {
  width: 16px;
  height: 16px;
}
.fastclaw-chat-workspace .jcr-message.fastclaw-fav-target {
  border-radius: 12px;
  outline: 2px solid color-mix(in srgb, var(--color-primary) 55%, transparent);
  background: color-mix(in srgb, var(--color-primary) 12%, transparent);
  transition: background-color 0.35s ease, outline-color 0.35s ease;
}
.fastclaw-workspace-avatar { width: 30px; height: 30px; border-radius: 50%; object-fit: cover; }
/* 与旧 MessageBubble 用户头像一致：32px 圆、contain，用 person.png 本体而不是线稿 SVG */
.fastclaw-user-avatar {
  display: flex;
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  overflow: hidden;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
}
.fastclaw-user-avatar img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}
.fastclaw-chat-workspace .jcr-message--user .jcr-avatar {
  width: 32px !important;
  height: 32px !important;
  flex-basis: 32px !important;
  overflow: hidden;
  background: var(--fastclaw-default-user-avatar) center / contain no-repeat !important;
}
.fastclaw-chat-workspace .jcr-message--user .jcr-user-icon {
  display: none !important;
}
.fastclaw-workspace-model-icon { color: var(--color-primary); }
.fastclaw-chat-workspace .fastclaw-composer-action {
  color: var(--text-secondary);
  transition: color .15s, background .15s;
}
.fastclaw-chat-workspace .fastclaw-composer-action--thinking:not(.is-active),
.fastclaw-chat-workspace .fastclaw-composer-action--planning:not(.is-active) {
  color: color-mix(in srgb, var(--text-secondary) 55%, transparent);
}
.fastclaw-chat-workspace .jcr-composer__actions button:not(:disabled):hover {
  color: var(--text-secondary) !important;
  background: var(--dropdown-hover-color) !important;
}
.fastclaw-chat-workspace .jcr-composer__actions button.is-active,
.fastclaw-chat-workspace .jcr-composer__actions button[aria-pressed="true"] {
  color: var(--text-primary) !important;
  background: transparent;
}
.fastclaw-chat-workspace .jcr-composer__actions button:disabled {
  opacity: .4;
  cursor: not-allowed;
}

/* 消息头 / 时间 / 操作图标：与对话正文同色，整体贴近旧版智能对话工具条 */
.fastclaw-chat-workspace .jcr-message__header,
.fastclaw-chat-workspace .jcr-message__header > span,
.fastclaw-chat-workspace .jcr-message__header time,
.fastclaw-chat-workspace .jcr-message__time,
.fastclaw-chat-workspace .jcr-action:not(.jcr-action--danger):not(.jcr-action--active):not(.jcr-action--negative),
.fastclaw-chat-workspace .jcr-more > summary,
.fastclaw-chat-workspace .jcr-action-menu > summary,
.fastclaw-chat-workspace .fastclaw-message-action-icon {
  color: var(--text-primary) !important;
}
.fastclaw-chat-workspace .jcr-action svg,
.fastclaw-chat-workspace .jcr-more > summary svg,
.fastclaw-chat-workspace .jcr-action-menu > summary svg,
.fastclaw-chat-workspace .fastclaw-message-action-icon svg {
  color: inherit;
  stroke: currentColor;
}
/* 点赞已选中：黄填充 + 保留描边轮廓（不被填充盖住） */
.fastclaw-chat-workspace .jcr-action--active .fastclaw-like-icon.is-active,
.fastclaw-chat-workspace .jcr-action--active .fastclaw-like-icon.is-active path {
  fill: #faad14 !important;
  stroke: var(--text-primary) !important;
}
/* 点踩已选中：红填充 + 保留描边轮廓 */
.fastclaw-chat-workspace .jcr-action--negative .fastclaw-dislike-icon.is-active,
.fastclaw-chat-workspace .jcr-action--negative .fastclaw-dislike-icon.is-active path,
.fastclaw-chat-workspace .jcr-action--negative [data-action-icon="dislike"] svg,
.fastclaw-chat-workspace .jcr-action--negative [data-action-icon="dislike"] svg path {
  fill: var(--color-danger, #ff4d4f) !important;
  stroke: var(--text-primary) !important;
}
/* 赞/踩按钮：去掉深色底，避免遮挡图标描边轮廓 */
.fastclaw-chat-workspace .jcr-action:has([data-action-icon="like"]),
.fastclaw-chat-workspace .jcr-action:has([data-action-icon="dislike"]) {
  background: transparent !important;
}
.fastclaw-chat-workspace .jcr-action:has([data-action-icon="like"]):hover,
.fastclaw-chat-workspace .jcr-action:has([data-action-icon="like"]):focus,
.fastclaw-chat-workspace .jcr-action:has([data-action-icon="like"]):focus-visible,
.fastclaw-chat-workspace .jcr-action:has([data-action-icon="dislike"]):hover,
.fastclaw-chat-workspace .jcr-action:has([data-action-icon="dislike"]):focus,
.fastclaw-chat-workspace .jcr-action:has([data-action-icon="dislike"]):focus-visible,
.fastclaw-chat-workspace .jcr-action--active:has([data-action-icon="like"]),
.fastclaw-chat-workspace .jcr-action--negative:has([data-action-icon="dislike"]) {
  background: transparent !important;
}
.fastclaw-chat-workspace .jcr-token-usage,
.fastclaw-chat-workspace .jcr-token-usage__summary,
.fastclaw-chat-workspace .jcr-token-usage__summary:hover,
.fastclaw-chat-workspace .jcr-token-usage__chevron,
.fastclaw-chat-workspace .jcr-token-usage__heading,
.fastclaw-chat-workspace .jcr-token-usage__details {
  color: var(--text-primary) !important;
}
.fastclaw-chat-workspace .jcr-message--assistant .jcr-actions {
  border-top: 0;
  padding-top: 0;
}
.fastclaw-chat-workspace .jcr-action-divider {
  background: color-mix(in srgb, var(--text-primary) 28%, transparent);
}
.fastclaw-chat-workspace .jcr-action__svg,
.fastclaw-chat-workspace .fastclaw-message-action-icon,
.fastclaw-chat-workspace .fastclaw-message-action-icon svg {
  width: 16px;
  height: 16px;
}
.fastclaw-chat-workspace .jcr-action:not(.jcr-action--danger):not(:disabled):hover,
.fastclaw-chat-workspace .jcr-more > summary:hover,
.fastclaw-chat-workspace .jcr-action-menu > summary:hover {
  color: var(--text-primary) !important;
  background: var(--dropdown-hover-color) !important;
}

/* 翻译中：按钮加宽以容纳「翻译中...」文案，不影响其它 jcr-action */
.fastclaw-chat-workspace .jcr-action:has(.fastclaw-message-action-icon.is-translating) {
  width: auto !important;
  min-width: 28px;
  padding: 0 8px !important;
  gap: 4px;
}

/* 问题气泡内联编辑：对齐任务规划（框内取消 | 输入 | 确认） */
.fastclaw-chat-workspace .jcr-message--user:has(.workspace-inline-user-edit) .jcr-message__main {
  width: min(760px, 100%);
  max-width: min(760px, 100%);
}
.fastclaw-chat-workspace .jcr-message--user:has(.workspace-inline-user-edit) .jcr-message__content,
.fastclaw-chat-workspace .jcr-message--user:has(.workspace-inline-user-edit) .jcr-message__meta,
.fastclaw-chat-workspace .jcr-message--user:has(.workspace-inline-user-edit) .jcr-actions {
  display: none !important;
}
.fastclaw-chat-workspace .workspace-inline-user-edit {
  width: 100%;
  max-width: 100%;
  margin: 0;
  box-sizing: border-box;
}

/* 情景主题（除雾灰调）：编辑区背景=问题气泡色；确认按钮悬浮=取消按钮悬浮；任务规划编辑区同步 */
/* FastPlan 在非雾灰情景主题下会走 data-theme=dark（--fp-surface/#27364d），须直接打 --theme-question-bubble */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-conversation,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-conversation[data-theme="dark"],
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-conversation .jcr-conversation,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-conversation .jcr-panel,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-timeline-renderer .jcr-conversation,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-timeline-renderer .jcr-panel {
  --jcr-user-bg: var(--theme-question-bubble, var(--bubble-user-bg, var(--bg-secondary))) !important;
  --fp-surface: var(--theme-question-bubble, var(--bubble-user-bg, var(--bg-secondary)));
  --jcr-fg: var(--text-primary);
  --jcr-muted: var(--text-secondary);
  --jcr-border: var(--border-color);
  --jcr-primary: var(--color-primary);
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .inline-user-edit,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .workspace-inline-user-edit,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-inline-user-edit {
  background: var(--theme-question-bubble, var(--bubble-user-bg, var(--bg-secondary))) !important;
  border: 1px solid color-mix(in srgb, var(--color-primary, var(--jcr-primary)) 14%, var(--border-color, var(--jcr-border))) !important;
  border-radius: 12px !important;
  color: var(--text-primary, var(--jcr-fg)) !important;
  box-shadow: none !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .inline-edit-textarea,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-inline-edit-input {
  background: transparent !important;
  border: 1px solid color-mix(in srgb, var(--border-color, var(--jcr-border)) 80%, transparent) !important;
  border-radius: 8px !important;
  color: var(--text-primary, var(--jcr-fg)) !important;
  box-shadow: none !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .inline-edit-textarea:focus,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-inline-edit-input:focus {
  border-color: color-mix(in srgb, var(--color-primary, var(--jcr-primary)) 55%, var(--border-color, var(--jcr-border))) !important;
  outline: none !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .inline-edit-cancel,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .inline-edit-submit,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-inline-edit-cancel,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-inline-edit-submit {
  color: var(--text-secondary, var(--jcr-muted, #64748b)) !important;
  background: transparent !important;
  border: none !important;
  box-shadow: none !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .inline-edit-cancel:hover,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .inline-edit-submit:hover:not(:disabled),
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-inline-edit-cancel:hover,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-inline-edit-submit:hover:not(:disabled) {
  background: color-mix(in srgb, var(--text-primary, var(--jcr-fg, #1f2937)) 6%, transparent) !important;
  color: var(--text-primary, var(--jcr-fg, #1f2937)) !important;
  opacity: 1 !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .inline-edit-cancel svg,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .inline-edit-submit svg,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-inline-edit-cancel svg,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-inline-edit-submit svg {
  stroke: currentColor !important;
  color: inherit !important;
}

/* 翻译 / 导出等 jcr-action 菜单弹层：强制不透明，避免正文透出（不改动 --jcr-bg，以免影响 seamless 布局） */
.fastclaw-chat-workspace .jcr-action-menu__popup,
.fastclaw-chat-workspace .jcr-more__menu {
  background: var(--theme-dailog-color, var(--bg-primary-solid, #ffffff)) !important;
  background-color: var(--theme-dailog-color, var(--bg-primary-solid, #ffffff)) !important;
  opacity: 1 !important;
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
}

/* 输入区模型 / 知识库 / 角色选择器：文字和图标与正文同色 */
.fastclaw-chat-workspace .fastclaw-workspace-selectors .el-select__wrapper {
  background: transparent !important;
  box-shadow: none !important;
}
.fastclaw-chat-workspace .fastclaw-workspace-selectors .el-select__selected-item,
.fastclaw-chat-workspace .fastclaw-workspace-selectors .el-select__placeholder,
.fastclaw-chat-workspace .fastclaw-workspace-selectors .el-select__caret,
.fastclaw-chat-workspace .fastclaw-workspace-selectors .el-select__suffix,
.fastclaw-chat-workspace .fastclaw-workspace-selectors .el-tag,
.fastclaw-chat-workspace .fastclaw-workspace-selectors .el-tag__content {
  color: var(--text-primary) !important;
}

/* 问题气泡：各主题继续走 --theme-question-bubble；仅简洁浅色略加深，避免和页面背景同色。 */
.fastclaw-chat-workspace .fastplan-conversation,
.fastclaw-chat-workspace .fastplan-conversation .jcr-conversation,
.fastclaw-chat-workspace .fastplan-conversation .jcr-panel,
.fastclaw-chat-workspace .fastplan-timeline-renderer .jcr-conversation,
.fastclaw-chat-workspace .fastplan-timeline-renderer .jcr-panel {
  --jcr-user-bg: var(--theme-question-bubble, var(--bubble-user-bg, var(--bg-secondary)));
}
html.light[data-theme-id="simple-light"] .fastclaw-chat-workspace .fastplan-conversation,
html.light[data-theme-id="simple-light"] .fastclaw-chat-workspace .fastplan-conversation .jcr-conversation,
html.light[data-theme-id="simple-light"] .fastclaw-chat-workspace .fastplan-conversation .jcr-panel,
html.light[data-theme-id="simple-light"] .fastclaw-chat-workspace .fastplan-timeline-renderer .jcr-conversation,
html.light[data-theme-id="simple-light"] .fastclaw-chat-workspace .fastplan-timeline-renderer .jcr-panel {
  --jcr-user-bg: color-mix(in srgb, var(--text-primary) 7%, var(--bg-primary, #fff));
}
.fastclaw-chat-workspace .jcr-message--user .jcr-message__content {
  border: 0 !important;
  box-shadow: none !important;
}
html.light[data-theme-id="simple-light"] .fastclaw-chat-workspace .jcr-message--user .jcr-message__content {
  background: color-mix(in srgb, var(--text-primary) 7%, var(--bg-primary, #fff)) !important;
}

/* FastPlan 规划卡（jcr-message__main）：宿主覆盖，不改 planflow-planning-ui 源码 */
.fastclaw-chat-workspace .jcr-message__main .fp-trace-row__text,
.fastclaw-chat-workspace .jcr-message__main .fp-trace-detail.is-detail,
.fastclaw-chat-workspace .jcr-message__main .fp-plan-card__metrics.is-inline,
.fastclaw-chat-workspace .jcr-message__main .fp-task__args,
.fastclaw-chat-workspace .jcr-message__main .fp-task__duration,
.fastclaw-chat-workspace .jcr-message__main .fp-trace-details,
.fastclaw-chat-workspace .jcr-message__main .fp-raw-logs,
.fastclaw-chat-workspace .jcr-message__main .fp-planning-output,
.fastclaw-chat-workspace .jcr-message__main .flex.items-center.justify-between.text-\[11px\].text-slate-400 {
  color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 90%, var(--jcr-muted, var(--text-secondary))) !important;
}
/* PlanPreview「暂无任务节点」空态文案 */
.fastclaw-chat-workspace .jcr-message__main .text-xs.text-slate-400.py-6.text-center {
  color: var(--text-primary) !important;
}

/* 情景主题（除雾灰）：问题气泡 / AI 回答里的 fc-skill-badge 白底 25% 透明 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fc-skill-badge,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .fc-skill-badge,
html[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fc-skill-badge,
html[data-theme-id="custom-scene"] .fastclaw-chat-workspace .fc-skill-badge {
  background: rgb(255 255 255 / 25%) !important;
  background-color: rgb(255 255 255 / 25%) !important;
  color:white !important;
}

html.dark .fastclaw-chat-workspace .jcr-message__main .fp-task__tokens,
html.dark .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__metrics.is-inline.is-token,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-task__tokens,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__metrics.is-inline.is-token,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-task__tokens,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__metrics.is-inline.is-token {
  color: #f8ca54 !important;
}
/* 情景主题（除雾灰调）：任务正文链接与用量文字同色，不影响雾灰 / 浅色 / 暗色 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .jcr-markdown a,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .jcr-markdown a:hover,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .jcr-markdown a,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .jcr-markdown a:hover {
  color: #f8ca54 !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .fp-raw-logs .is-error,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-raw-logs .is-error,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-raw-logs .is-error {
  color: #aa1032  !important;
}
html.light[data-theme-id="simple-light"] .fastclaw-chat-workspace .jcr-message__main .fp-task__tokens,
html.light[data-theme-id="simple-light"] .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__metrics.is-inline.is-token,
html.light[data-theme-id="scene-graphite-cyan"] .fastclaw-chat-workspace .jcr-message__main .fp-task__tokens,
html.light[data-theme-id="scene-graphite-cyan"] .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__metrics.is-inline.is-token {
  color: #6366f1 !important;
}
html.light[data-theme-id="simple-light"] .fastclaw-chat-workspace .jcr-message__main .fp-raw-logs .is-error,
html.light[data-theme-id="scene-graphite-cyan"] .fastclaw-chat-workspace .jcr-message__main .fp-raw-logs .is-error {
  color: #e11d48 !important;
}
.fastclaw-chat-workspace .jcr-message__main .fp-trace-detail.is-narrative {
  color: var(--text-primary) !important;
}
.fastclaw-chat-workspace .jcr-message__main .fp-trace-detail.is-detail:not(.is-warn) {
  border-left-color: var(--border-color) !important;
}
.fastclaw-chat-workspace .jcr-message__main .fp-trace-row__chevron-button,
.fastclaw-chat-workspace .jcr-message__main .fp-trace-row__chevron-button svg,
.fastclaw-chat-workspace .jcr-message__main .fp-task__toggle,
.fastclaw-chat-workspace .jcr-message__main .fp-task__toggle svg {
  color: var(--text-primary) !important;
  stroke: currentColor;
}
.fastclaw-chat-workspace .jcr-message__main .fp-trace-row__chevron-button:hover,
.fastclaw-chat-workspace .jcr-message__main .fp-task__toggle:hover {
  color: var(--text-primary) !important;
  background: var(--dropdown-hover-color) !important;
}
.fastclaw-chat-workspace .jcr-message__main .fp-task {
  border-color: var(--border-color) !important;
}
/* 规划清单时间轴：圆点 absolute -left-[15px]/ring 会伸出容器，被外层 overflow-x:hidden 裁切。
   注意：加大 .relative.pl-4 的 padding 不会移动绝对定位圆点（相对 padding edge），
   需在父级预留 gutter，并把圆点改到竖线内侧。 */
.fastclaw-chat-workspace .jcr-message__main .fp-plan-card,
.fastclaw-chat-workspace .jcr-message__main .fp-plan-card__details,
.fastclaw-chat-workspace .jcr-message__main [data-plan-preview-card] {
  overflow: visible;
  min-width: 0;
}
.fastclaw-chat-workspace .jcr-message__main .fp-plan-card__details.is-direct {
  padding-left: 1.25rem; /* 20px gutter：容纳原 -15px 圆点 + ring-4 */
  padding-right: 8px; /* 编辑态 ring-1 右侧不被 overflow-x:hidden 裁切 */
  box-sizing: border-box;
}
/* 情景主题（除雾灰调）：编辑态预览卡边框改为主题色，确认 ring 完整可见 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__details.is-direct [data-plan-preview-card].ring-1,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__details.is-direct [data-plan-preview-card].ring-1 {
  --tw-ring-color: rgba(255, 255, 255, 0.65) !important;
  --tw-ring-offset-shadow: 0 0 #0000 !important;
  --tw-ring-shadow: 0 0 0 1px rgba(255, 255, 255, 0.65) !important;
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.65) !important;
  outline: none !important;
}
/* 情景主题（除雾灰调）：撤销修改 悬浮底 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__details.is-direct button.px-3.py-1\.5.rounded-lg.text-slate-500:hover,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__details.is-direct button.px-3.py-1\.5.rounded-lg.text-slate-500:hover {
  background: var(--dropdown-hover-color) !important;
  background-color: var(--dropdown-hover-color) !important;
}
/* 情景主题（除雾灰调）：放弃修改 悬浮底 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__details.is-direct button.px-3.py-1\.5.rounded-lg.text-rose-500:hover,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__details.is-direct button.px-3.py-1\.5.rounded-lg.text-rose-500:hover {
  background: var(--dropdown-hover-color) !important;
  background-color: var(--dropdown-hover-color) !important;
}
/* 情景主题（除雾灰调）：任务行尾编辑/复制/删除/展开按钮悬浮底 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__details.is-direct .flex.items-center.gap-1\.5.text-xs.cursor-pointer > button:hover,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__details.is-direct .flex.items-center.gap-1\.5.text-xs.cursor-pointer > button:hover {
  background: var(--dropdown-hover-color) !important;
  background-color: var(--dropdown-hover-color) !important;
}
.fastclaw-chat-workspace .jcr-message__main [data-plan-preview-card] .pt-1,
.fastclaw-chat-workspace .jcr-message__main [data-plan-preview-card] .py-4 {
  overflow: visible;
}
.fastclaw-chat-workspace .jcr-message__main [data-plan-preview-card] .relative.pl-4.space-y-3,
.fastclaw-chat-workspace .jcr-message__main [data-plan-preview-card] .pt-1 > .relative.pl-4,
.fastclaw-chat-workspace .jcr-message__main [data-plan-preview-card] .py-4 > .relative.pl-4 {
  padding-left: 1rem !important; /* 保证阶段标题/任务文案不贴竖线 */
  overflow: visible;
}
.fastclaw-chat-workspace .jcr-message__main [data-plan-preview-card] .relative.pl-4.space-y-3 {
  padding-right: 10px;
  padding-bottom: 10px !important;
}
/* 编辑态底部操作栏：下内边距 10px；撤销修改距左侧 10px */
.fastclaw-chat-workspace .jcr-message__main [data-plan-preview-card] .pt-3.border-t.border-slate-100.flex.items-center.gap-2.min-h-\[30px\] {
  padding-bottom: 10px !important;
  padding-left: 10px !important;
}
/* 清单圆点：从 -15px 收到竖线位置（before:left-[5px]，w-2 → left≈1px） */
.fastclaw-chat-workspace .jcr-message__main [data-plan-preview-card] .absolute.-left-\[15px\] {
  left: 1px !important;
}
/* 时间轴视图圆点：原 -23px */
.fastclaw-chat-workspace .jcr-message__main [data-plan-preview-card] .absolute.-left-\[23px\] {
  left: -0.35rem !important;
}
.fastclaw-chat-workspace .jcr-message__main [data-plan-preview-card] .fp-row,
.fastclaw-chat-workspace .jcr-message__main [data-plan-preview-card] .group.relative.bg-white {
  overflow: visible;
  min-width: 0;
}
.fastclaw-chat-workspace .jcr-message__main .fp-planning-output,
.fastclaw-chat-workspace .jcr-message__main .fp-raw-logs {
  overflow-x: auto;
  overflow-wrap: anywhere;
  word-break: break-word;
}
.fastclaw-chat-workspace .jcr-message__main .fp-planning-output .jcr-markdown,
.fastclaw-chat-workspace .jcr-message__main .fp-planning-output .jcr-markdown pre,
.fastclaw-chat-workspace .jcr-message__main .fp-planning-output .jcr-markdown code {
  max-width: 100%;
  overflow-wrap: anywhere;
  word-break: break-word;
  white-space: pre-wrap;
}
/* ManualReviewPanel 确认条：边框 --border-color，文字对齐 .fp-task__executor */
.fastclaw-chat-workspace .jcr-message__main .fp-task__executor,
.fastclaw-chat-workspace .jcr-message__main .flex.items-center.gap-2.text-xs.text-gray-500.px-3.py-1\.5.bg-gray-50\/80.rounded-lg.border.border-gray-100,
.fastclaw-chat-workspace .jcr-message__main .flex.items-center.gap-2.text-xs.text-gray-500.px-3.py-1\.5.bg-gray-50\/80.rounded-lg.border.border-gray-100 .text-gray-400,
.fastclaw-chat-workspace .jcr-message__main .flex.items-center.gap-2.text-xs.text-gray-500.px-3.py-1\.5.bg-gray-50\/80.rounded-lg.border.border-gray-100 .text-gray-700,
.fastclaw-chat-workspace .jcr-message__main .mt-3.px-3.py-2.rounded-lg.border.border-slate-200.bg-slate-50.text-xs.font-medium.text-slate-500 {
  color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 76%, var(--jcr-muted, var(--text-secondary))) !important;
}
.fastclaw-chat-workspace .jcr-message__main .flex.items-center.gap-2.text-xs.text-gray-500.px-3.py-1\.5.bg-gray-50\/80.rounded-lg.border.border-gray-100,
.fastclaw-chat-workspace .jcr-message__main .mt-3.px-3.py-2.rounded-lg.border.border-slate-200.bg-slate-50.text-xs.font-medium.text-slate-500 {
  border-color: var(--border-color) !important;
}
.fastclaw-chat-workspace .jcr-message__main .fp-trace-row + .fp-trace-row {
  border-top-color: var(--border-color) !important;
}

/* 情景主题（除雾灰调）+ 黑暗主题 */
html.dark .fastclaw-chat-workspace .jcr-message__main .fp-trace-row__label,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-trace-row__label,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-trace-row__label {
  /* background: rgba(255, 255, 255, 0.45) !important; */
  background: var(--color-primary) !important; 
  color:#fff !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .fp-step-spinner,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-step-spinner,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-step-spinner {
  border-color: color-mix(in srgb, var(--color-primary) 24%, transparent) !important;
  border-top-color: var(--color-primary) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .fp-step-status:not([data-status="done"]):not([data-status="failed"]),
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-step-status:not([data-status="done"]):not([data-status="failed"]),
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-step-status:not([data-status="done"]):not([data-status="failed"]) {
  color: var(--color-primary) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .fp-task__details,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-task__details,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-task__details {
  border-color: var(--border-color) !important;
  background: var(--card-background-tmd) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__tabs,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__tabs,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__tabs {
  background: var(--card-background-tmd) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__tabs button,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__tabs button,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__tabs button {
 color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;

}
html.dark .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__tabs button.is-active,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__tabs button.is-active,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-plan-card__tabs button.is-active {
  color: #fff !important;
  background: var(--color-primary) !important;
  box-shadow: none;
}

/* FastPlan 时间轴（.py-4）宿主覆盖 */
.fastclaw-chat-workspace .jcr-message__main .py-4 .text-sm.text-gray-400.font-medium {
  color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
}
.fastclaw-chat-workspace .jcr-message__main .py-4 button.p-1.text-gray-400,
.fastclaw-chat-workspace .jcr-message__main .py-4 button.p-1.text-gray-400 svg {
  color: var(--text-primary) !important;
  stroke: currentColor;
}
.fastclaw-chat-workspace .jcr-message__main .py-4 button.p-1.text-gray-400:hover {
  color: var(--text-primary) !important;
  background: var(--dropdown-hover-color) !important;
  border-radius: 5px !important;
}
.fastclaw-chat-workspace .jcr-message__main .py-4 .absolute.left-\[27px\].top-4.bottom-8.w-0\.5.bg-gray-200 {
  background: var(--text-primary) !important;
}

/* 情景主题（除雾灰调）：时间轴节点 / 卡片 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .w-6.h-6.rounded-full.bg-indigo-600,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .w-6.h-6.rounded-full.bg-indigo-600 {
  background: var(--color-primary) !important;
  --tw-ring-color: rgba(255, 255, 255, 0.45) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .absolute.-left-\[23px\].top-4.w-2\.5.h-2\.5.rounded-full.bg-indigo-600,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .absolute.-left-\[23px\].top-4.w-2\.5.h-2\.5.rounded-full.bg-indigo-600 {
  background: var(--color-primary) !important;
  --tw-ring-color: rgba(255, 255, 255, 0.45) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .w-6.h-6.rounded-full.bg-gray-100,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .w-6.h-6.rounded-full.bg-gray-100 {
  background: rgba(255, 255, 255, 0.20) !important;
  border-color: rgba(255, 255, 255, 0.15) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .w-6.h-6.rounded-full.bg-gray-100 svg,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .w-6.h-6.rounded-full.bg-gray-100 svg {
  color: #fff !important;
  stroke: currentColor;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .absolute.left-\[27px\].top-4.bottom-8.w-0\.5.bg-gray-200,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .absolute.left-\[27px\].top-4.bottom-8.w-0\.5.bg-gray-200 {
  background: var(--border-color) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white {
  background: var(--card-background-tmd) !important;
  border-color: var(--border-color) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white:hover,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white:hover {
  background: var(--card-background-tmd) !important;
  border-color: var(--border-color) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white .jcr-model-icon.jcr-model-icon--fallback.fpp-model-icon circle,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white .jcr-model-icon.jcr-model-icon--fallback.fpp-model-icon circle {
  fill: var(--color-primary) !important;
}
.fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white .text-sm.font-semibold.text-slate-800,
.fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white .font-medium.text-slate-600 {
  color: var(--text-primary) !important;
}
.fastclaw-chat-workspace .jcr-message__main .py-4 .flex-1.font-semibold.text-base.text-gray-800 {
  color: var(--text-primary) !important;
}
/* 情景主题（除雾灰调）：时间轴阶段名输入框背景透明 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .flex-1.font-semibold.text-base.text-gray-800.bg-transparent.border-b.border-transparent.px-0\.5.py-0\.5,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 input.flex-1.font-semibold.text-base.text-gray-800,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .flex-1.font-semibold.text-base.text-gray-800.bg-transparent.border-b.border-transparent.px-0\.5.py-0\.5,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 input.flex-1.font-semibold.text-base.text-gray-800 {
  background: transparent !important;
  background-color: transparent !important;
  box-shadow: none !important;
  outline: none !important;
  border-bottom-color: var(--border-color) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 input.flex-1.font-semibold.text-base.text-gray-800:hover,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 input.flex-1.font-semibold.text-base.text-gray-800:focus,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 input.flex-1.font-semibold.text-base.text-gray-800:hover,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 input.flex-1.font-semibold.text-base.text-gray-800:focus {
  border-bottom-color: var(--border-color) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .flex-1.font-semibold.text-base.text-gray-800.bg-transparent.border-b.border-transparent.px-0\.5.py-0\.5:-webkit-autofill,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .flex-1.font-semibold.text-base.text-gray-800.bg-transparent.border-b.border-transparent.px-0\.5.py-0\.5:-webkit-autofill:hover,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .flex-1.font-semibold.text-base.text-gray-800.bg-transparent.border-b.border-transparent.px-0\.5.py-0\.5:-webkit-autofill:focus,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .flex-1.font-semibold.text-base.text-gray-800.bg-transparent.border-b.border-transparent.px-0\.5.py-0\.5:-webkit-autofill,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .flex-1.font-semibold.text-base.text-gray-800.bg-transparent.border-b.border-transparent.px-0\.5.py-0\.5:-webkit-autofill:hover,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .flex-1.font-semibold.text-base.text-gray-800.bg-transparent.border-b.border-transparent.px-0\.5.py-0\.5:-webkit-autofill:focus {
  -webkit-box-shadow: 0 0 0 1000px transparent inset !important;
  box-shadow: 0 0 0 1000px transparent inset !important;
  -webkit-text-fill-color: var(--text-primary) !important;
  transition: background-color 99999s ease-in-out 0s;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white .text-xs.text-slate-500,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white .text-slate-500,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white .text-slate-300,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white .text-xs.text-slate-500,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white .text-slate-500,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .group.relative.bg-white .text-slate-300 {
  color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .text-\[10px\].px-1\.5.py-0\.5.rounded.bg-blue-50.text-blue-700,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .text-\[10px\].px-1\.5.py-0\.5.rounded.bg-blue-50.text-blue-700 {
  background-color: color-mix(in srgb, #eff6ff 55%, transparent) !important;
}

/* 情景主题（除雾灰调）：清单 LLM / Agent 徽章背景 55% */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .text-\[10px\].font-bold.px-1\.5.py-0\.5.rounded.shrink-0.text-blue-700.bg-blue-50,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .text-\[10px\].font-bold.px-1\.5.py-0\.5.rounded.shrink-0.text-blue-700.bg-blue-50 {
  background-color: color-mix(in srgb, #eff6ff 55%, transparent) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .text-\[10px\].font-bold.px-1\.5.py-0\.5.rounded.shrink-0.text-purple-700.bg-purple-50,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .text-\[10px\].font-bold.px-1\.5.py-0\.5.rounded.shrink-0.text-purple-700.bg-purple-50 {
  background-color: color-mix(in srgb, #faf5ff 55%, transparent) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .text-\[10px\].font-semibold.px-1\.5.py-0\.5.rounded.bg-green-100.text-green-700,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .text-\[10px\].font-semibold.px-1\.5.py-0\.5.rounded.bg-green-100.text-green-700 {
  background-color: color-mix(in srgb, #dcfce7 55%, transparent) !important;
}

/* 情景主题（除雾灰调）：时间轴 py-4 辅助文案 / 操作区 / 虚线按钮 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .text-xs.text-slate-400.py-5,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .text-xs.text-slate-400.py-5 {
  color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .text-\[10px\].text-slate-400,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .text-\[10px\].text-slate-400 {
  color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .flex.items-center.justify-between.gap-2,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .flex.items-center.justify-between.gap-2 {
  background: transparent !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .p-1.text-gray-300.hover\:text-red-500.opacity-0.group-hover\:opacity-100.transition-opacity,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .p-1.text-gray-300.hover\:text-red-500.opacity-0.group-hover\:opacity-100.transition-opacity svg,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .p-1.text-gray-300.hover\:text-red-500.opacity-0.group-hover\:opacity-100.transition-opacity,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .p-1.text-gray-300.hover\:text-red-500.opacity-0.group-hover\:opacity-100.transition-opacity svg {
  color: var(--text-primary) !important;
  stroke: currentColor !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .text-\[10px\].opacity-0.group-hover\:opacity-100.text-indigo-600.transition-opacity.whitespace-nowrap,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .text-\[10px\].opacity-0.group-hover\:opacity-100.text-indigo-600.transition-opacity.whitespace-nowrap {
  color: var(--text-primary) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .text-gray-300.hover\:text-red-500.p-1.opacity-0.group-hover\:opacity-100.transition-opacity,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .text-gray-300.hover\:text-red-500.p-1.opacity-0.group-hover\:opacity-100.transition-opacity svg,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .text-gray-300.hover\:text-red-500.p-1.opacity-0.group-hover\:opacity-100.transition-opacity,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .text-gray-300.hover\:text-red-500.p-1.opacity-0.group-hover\:opacity-100.transition-opacity svg {
  color: var(--text-primary) !important;
  stroke: currentColor !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .w-full.py-1\.5.border.border-dashed.border-slate-200.rounded-lg.text-xs.text-slate-400.flex.items-center.justify-center.gap-1.transition-all,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .w-full.py-1\.5.border.border-dashed.border-slate-200.rounded-lg.text-xs.text-slate-400.flex.items-center.justify-center.gap-1.transition-all {
  border-color: var(--border-color) !important;
  color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .py-4 .w-full.py-1\.5.border.border-dashed.border-slate-200.rounded-lg.text-xs.text-slate-400.flex.items-center.justify-center.gap-1.transition-all:hover,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .py-4 .w-full.py-1\.5.border.border-dashed.border-slate-200.rounded-lg.text-xs.text-slate-400.flex.items-center.justify-center.gap-1.transition-all:hover {
  border-color: var(--border-color) !important;
  color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
}

/* 情景主题（除雾灰调）：清单视图阶段标题 / 任务数 → --text-primary */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .flex.items-center.justify-between.text-\[10px\].font-semibold.text-slate-400,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .font-mono.text-slate-300,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .flex.items-center.justify-between.text-\[10px\].font-semibold.text-slate-400,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .font-mono.text-slate-300 {
  color: var(--text-primary) !important;
}

/* 情景主题（除雾灰调）：清单任务行次要文字 / chevron */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .lucide-chevron-right.text-slate-200.shrink-0,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .lucide-chevron-right-icon.text-slate-200.shrink-0,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .font-mono.text-\[10px\].text-slate-400.shrink-0.truncate,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .font-mono.text-\[9px\].text-slate-400.shrink-0,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .font-mono.text-\[10px\].text-slate-300.shrink-0,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .font-mono.text-\[11px\].text-slate-400.shrink-0,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .lucide-chevron-right.text-slate-200.shrink-0,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .lucide-chevron-right-icon.text-slate-200.shrink-0,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .font-mono.text-\[10px\].text-slate-400.shrink-0.truncate,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .font-mono.text-\[9px\].text-slate-400.shrink-0,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .font-mono.text-\[10px\].text-slate-300.shrink-0,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .font-mono.text-\[11px\].text-slate-400.shrink-0 {
  color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
  stroke: currentColor;
}

/* 情景主题（除雾灰调）：清单任务圆点主题色，外圈白环 45% */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .absolute.-left-\[15px\].top-1\.5.w-2.h-2.rounded-full.bg-slate-300.ring-4.ring-white,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .absolute.-left-\[15px\].top-1\.5.w-2.h-2.rounded-full.bg-slate-300.ring-4.ring-white {
  background: var(--color-primary) !important;
  --tw-ring-color: rgba(255, 255, 255, 0.45) !important;
  box-shadow: 0 0 0 4px rgba(255, 255, 255, 0.45) !important;
}

/* 情景主题（除雾灰调）：清单左侧竖线改为白色 65% 透明 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .relative.pl-4.space-y-3::before,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .relative.pl-4.space-y-3::before {
  background-color: rgba(255, 255, 255, 0.65) !important;
}

/* FastPlan DAG 图（.p-4.h-[500px]）：黑暗 + 情景（除雾灰调） */
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.top-3.right-3.text-\[11px\].text-gray-400.bg-white\/80,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.top-3.right-3.text-\[11px\].text-gray-400.bg-white\/80,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.top-3.right-3.text-\[11px\].text-gray-400.bg-white\/80 {
  background: var(--card-background-tmd) !important;
  color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
}

/* 情景主题（除雾灰调）：DAG 右键菜单背景 / 边框 / 悬浮 / 分割线 / 文字 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 {
  background: var(--theme-dailog-color) !important;
  background-color: var(--theme-dailog-color) !important;
  border-color: var(--border-color) !important;
  color: var(--text-primary) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 .border-b.border-gray-100,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 hr,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 hr.border-gray-100,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 .border-b.border-gray-100,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 hr,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 hr.border-gray-100 {
  border-color: var(--border-color) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 button,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 [class*="text-"],
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 button,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 [class*="text-"] {
  color: var(--text-primary) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 button:hover,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 button:hover {
  background: var(--dropdown-hover-color) !important;
  background-color: var(--dropdown-hover-color) !important;
  color: var(--text-primary) !important;
}

html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bottom-4.left-4.bg-white\/90,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bottom-4.left-4.bg-white\/90,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bottom-4.left-4.bg-white\/90 {
  background: var(--card-background-tmd) !important;
  border-color: var(--border-color) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bottom-4.left-4 .font-medium.text-gray-700.mb-2,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bottom-4.left-4 .font-medium.text-gray-700.mb-2,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bottom-4.left-4 .font-medium.text-gray-700.mb-2 {
  color: var(--text-primary) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bottom-4.left-4 .text-gray-500,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bottom-4.left-4 .text-gray-500,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bottom-4.left-4 .text-gray-500 {
  color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bottom-4.left-4 .border-t.border-gray-100,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bottom-4.left-4 .border-t.border-gray-100,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bottom-4.left-4 .border-t.border-gray-100 {
  border-color: var(--border-color) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .phase-groups .group-rect,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .phase-groups .group-rect,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .phase-groups .group-rect {
  fill: var(--card-background-tmd) !important;
  stroke: var(--border-color) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .node-rect,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .node-rect.node-pending,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .node-rect,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .node-rect.node-pending,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .node-rect,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .node-rect.node-pending {
  fill: var(--card-background-tmd) !important;
  stroke: var(--border-color) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group > rect[width="4"],
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group > rect[width="4"],
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group > rect[width="4"] {
  fill: var(--color-primary) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .node-text,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .node-text,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .node-text {
  fill: var(--text-primary) !important;
}
/* 任务规划 DAG：node-pending 节点红框文字（Start / End / 标题）→ --text-primary；样式在宿主覆盖，不改 chat-plan-packages */
.fpp-ui .graph-container .node-group:has(.node-rect.node-pending) .node-label-title,
.fpp-ui .graph-container .node-group:has(.node-rect.node-pending) .node-text,
.fastclaw-chat-workspace .jcr-message__main .graph-container .node-group:has(.node-rect.node-pending) .node-label-title,
.fastclaw-chat-workspace .jcr-message__main .graph-container .node-group:has(.node-rect.node-pending) .node-text,
.fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group:has(.node-rect.node-pending) .node-label-title,
.fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group:has(.node-rect.node-pending) .node-text {
  color: var(--text-primary) !important;
  fill: var(--text-primary) !important;
}
/* 任务规划 DAG：节点标题（红框内名称）→ --text-primary；样式在宿主覆盖，不改 chat-plan-packages */
.fpp-ui.graph-container .node-group .node-label-title,
.fastclaw-chat-workspace .jcr-message__main .graph-container .node-group .node-label-title,
.fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .node-label-title {
  color: var(--text-primary) !important;
}
/* DAG 右键菜单 Teleport 到 body 后的主题适配（须压过 main.css 的 .dark .bg-white[class*="shadow"]） */
html body > .fpp-graph-context-menu,
.fpp-graph-context-menu.bg-white,
.fpp-graph-context-menu {
  background: var(--theme-dailog-color, #fff) !important;
  background-color: var(--theme-dailog-color, #fff) !important;
  border-color: var(--border-color) !important;
  color: var(--text-primary) !important;
  opacity: 1 !important;
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
}
html.dark body > .fpp-graph-context-menu,
html.dark .fpp-graph-context-menu,
html.dark .fpp-graph-context-menu.bg-white,
html.dark .fpp-graph-context-menu.bg-white[class*="shadow"],
html.dark .fpp-ui.fpp-graph-context-menu.bg-white.shadow-2xl,
.dark .fpp-graph-context-menu.bg-white[class*="shadow"] {
  background: var(--theme-dailog-color, rgb(42, 43, 47)) !important;
  background-color: var(--theme-dailog-color, rgb(42, 43, 47)) !important;
  opacity: 1 !important;
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
}
.fpp-graph-context-menu .text-gray-400,
.fpp-graph-context-menu .text-gray-700,
.fpp-graph-context-menu .border-gray-100 {
  color: var(--text-secondary) !important;
  border-color: var(--border-color) !important;
}
.fpp-graph-context-menu button.text-gray-700 {
  color: var(--text-primary) !important;
}
.fpp-graph-context-menu button:hover {
  background: var(--dropdown-hover-color) !important;
}
/* 情景主题（除雾灰调）：菜单文字统一白色 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fpp-graph-context-menu,
html.light[data-theme-id="custom-scene"] .fpp-graph-context-menu {
  color: #fff !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fpp-graph-context-menu .text-gray-400,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fpp-graph-context-menu .text-gray-700,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fpp-graph-context-menu .text-blue-700,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fpp-graph-context-menu .text-red-600,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fpp-graph-context-menu button,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fpp-graph-context-menu button.text-gray-700,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fpp-graph-context-menu button.text-blue-700,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fpp-graph-context-menu button.text-red-600,
html.light[data-theme-id="custom-scene"] .fpp-graph-context-menu .text-gray-400,
html.light[data-theme-id="custom-scene"] .fpp-graph-context-menu .text-gray-700,
html.light[data-theme-id="custom-scene"] .fpp-graph-context-menu .text-blue-700,
html.light[data-theme-id="custom-scene"] .fpp-graph-context-menu .text-red-600,
html.light[data-theme-id="custom-scene"] .fpp-graph-context-menu button,
html.light[data-theme-id="custom-scene"] .fpp-graph-context-menu button.text-gray-700,
html.light[data-theme-id="custom-scene"] .fpp-graph-context-menu button.text-blue-700,
html.light[data-theme-id="custom-scene"] .fpp-graph-context-menu button.text-red-600 {
  color: #fff !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fpp-graph-context-menu .border-gray-100,
html.light[data-theme-id="custom-scene"] .fpp-graph-context-menu .border-gray-100,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fpp-graph-context-menu hr,
html.light[data-theme-id="custom-scene"] .fpp-graph-context-menu hr {
  border-color: rgba(255, 255, 255, 0.15) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-circle,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-circle-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-circle,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-circle-icon,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-circle,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-circle-icon {
  color: var(--text-primary) !important;
  stroke: currentColor !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .edge:not(.edge-active):not(.edge-dependency),
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .edge:not(.edge-active):not(.edge-dependency),
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .edge:not(.edge-active):not(.edge-dependency) {
  stroke: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] #arrowhead polygon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] #arrowhead polygon,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] #arrowhead polygon {
  fill: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .edge.edge-active,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .edge.edge-active,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .edge.edge-active {
  stroke: var(--color-primary) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] #arrowhead-active polygon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] #arrowhead-active polygon,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] #arrowhead-active polygon {
  fill: var(--color-primary) !important;
}

/* 图1：阶段名「直接回答」文字 --text-primary，白底透明 */
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .phase-groups .group-label,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .phase-groups .group-label,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .phase-groups .group-label {
  fill: var(--text-primary) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .phase-groups rect:not(.group-rect),
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .phase-groups rect:not(.group-rect),
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .phase-groups rect:not(.group-rect) {
  fill: transparent !important;
}

/* 图2：LLM 工具标签背景 45% */
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group foreignObject span.inline-flex,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group foreignObject span.inline-flex,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group foreignObject span.inline-flex {
  background: color-mix(in srgb, #DBEAFE 45%, transparent) !important;
}

/* 图1：DAG 完成态勾选（内联 #5B6AF0）→ 主题色 */
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group foreignObject svg[style*="5B6AF0"],
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group foreignObject svg[style*="5B6AF0"] circle,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group foreignObject svg[style*="5B6AF0"] path,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-check-circle-2,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-check-circle-2-icon,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-circle-check,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-circle-check-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group foreignObject svg[style*="5B6AF0"],
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group foreignObject svg[style*="5B6AF0"] circle,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group foreignObject svg[style*="5B6AF0"] path,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-check-circle-2,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-check-circle-2-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-circle-check,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-circle-check-icon,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group foreignObject svg[style*="5B6AF0"],
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group foreignObject svg[style*="5B6AF0"] circle,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group foreignObject svg[style*="5B6AF0"] path,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-check-circle-2,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-check-circle-2-icon,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-circle-check,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .node-group .lucide-circle-check-icon {
  color: var(--color-primary) !important;
  stroke: currentColor !important;
}

/* FastPlan fallback 模型图标圆底 → 主题色（DAG / 时间轴 / 任务卡） */
html.dark .fastclaw-chat-workspace .jcr-model-icon.jcr-model-icon--fallback.fpp-model-icon circle,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-model-icon.jcr-model-icon--fallback.fpp-model-icon circle,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-model-icon.jcr-model-icon--fallback.fpp-model-icon circle {
  fill: var(--color-primary) !important;
}

/* 情景主题（除雾灰调）：执行任务行圆标主题色 + 标题 --text-primary */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-task .jcr-model-icon--fallback circle,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-task .fpp-model-icon circle,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-task .jcr-model-icon--fallback circle,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-task .fpp-model-icon circle {
  fill: var(--color-primary) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-task__status:not([data-status="completed"]):not([data-status="failed"]):not([data-status="skipped"]),
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-task__status:not([data-status="completed"]):not([data-status="failed"]):not([data-status="skipped"]) svg,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-task__status:not([data-status="completed"]):not([data-status="failed"]):not([data-status="skipped"]),
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-task__status:not([data-status="completed"]):not([data-status="failed"]):not([data-status="skipped"]) svg {
  color: var(--color-primary) !important;
  stroke: currentColor !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-task__prefix,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fp-task__prefix strong,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-task__prefix,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fp-task__prefix strong {
  color: var(--text-primary) !important;
}

/* 情景主题（除雾灰调）：底部任务列表圆标主题色 + 序号/标题 --text-primary（覆盖 running 紫渐变） */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-tasks__status-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__status-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-tasks__node[data-status="pending"] .jcr-plan-tasks__status-icon,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-tasks__status-icon,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__status-icon,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-tasks__node[data-status="pending"] .jcr-plan-tasks__status-icon {
  color: var(--color-primary) !important;
  stroke: currentColor !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-tasks__node[data-status="completed"] .jcr-plan-tasks__status-icon,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-tasks__node[data-status="completed"] .jcr-plan-tasks__status-icon {
  color: #16a34a !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-tasks__node[data-status="failed"] .jcr-plan-tasks__status-icon,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-tasks__node[data-status="failed"] .jcr-plan-tasks__status-icon {
  color: #dc2626 !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-tasks__sequence,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-tasks__node-title,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__sequence,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__node-title,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-tasks__sequence,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-tasks__node-title,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__sequence,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__node-title {
  color: var(--text-primary) !important;
  background: none !important;
  background-image: none !important;
  -webkit-background-clip: border-box !important;
  background-clip: border-box !important;
  animation: none !important;
}

/* 情景主题（除雾灰调）：人工确认区背景透明 + 边框 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-confirm,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-confirm {
  background: transparent !important;
  border-color: var(--border-color) !important;
  box-shadow: none !important;
}
/* 仅标题「确认执行当前规划」与危险项「取消任务」 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-confirm > strong,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-confirm__options button.is-danger b,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-confirm > strong,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-confirm__options button.is-danger b {
  color: #a50528 !important;
}
/* 情景主题（除雾灰调）：确认选项选中/悬浮高亮改用 --text-primary，不影响浅色/暗色/雾灰 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-confirm__options button.is-selected,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-confirm__options button.is-selected {
  background: color-mix(in srgb, var(--text-primary) 15%, transparent) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-plan-tasks__body,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-plan-tasks__body {
  border-top-color: var(--border-color) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-composer-contribution-target,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .fastplan-composer-contribution-target {
  border-top-color: var(--border-color) !important;
}

/* 情景主题（除雾灰调）+ 黑暗主题：生成文件卡片背景 = 问题气泡；下载按钮 = 新建对话 */
html.dark .fastclaw-chat-workspace .fastplan-conversation__files a,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-conversation__files a,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .fastplan-conversation__files a {
  background: var(--theme-question-bubble, var(--jcr-user-bg, var(--bubble-user-bg, var(--bg-secondary)))) !important;
  border-color: var(--border-color) !important;
  color: var(--text-primary) !important;
  transform: none !important;
}
html.dark .fastclaw-chat-workspace .fastplan-conversation__files a:hover,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-conversation__files a:hover,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .fastplan-conversation__files a:hover {
  background: var(--theme-question-bubble, var(--jcr-user-bg, var(--bubble-user-bg, var(--bg-secondary)))) !important;
  border-color: var(--border-color) !important;
  transform: none !important;
}
html.dark .fastclaw-chat-workspace .fastplan-conversation__file-info small,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-conversation__file-info small,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .fastplan-conversation__file-info small {
  color: var(--text-secondary) !important;
}
html.dark .fastclaw-chat-workspace .fastplan-conversation__download-action,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-conversation__download-action,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .fastplan-conversation__download-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 16px;
  background: var(--btn-primary-bg, var(--color-primary)) !important;
  color: var(--btn-primary-text, #fff) !important;
  border: 1px solid var(--btn-primary-border, transparent);
  border-radius: 4px;
  font-size: var(--font-size-md);
  font-weight: 500;
  transition: all 0.2s;
}
html.dark .fastclaw-chat-workspace .fastplan-conversation__files a:hover .fastplan-conversation__download-action,
html.dark .fastclaw-chat-workspace .fastplan-conversation__download-action:hover,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-conversation__files a:hover .fastplan-conversation__download-action,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-conversation__download-action:hover,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .fastplan-conversation__files a:hover .fastplan-conversation__download-action,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .fastplan-conversation__download-action:hover {
  background: var(--btn-primary-hover-bg, var(--color-primary)) !important;
  color: var(--btn-primary-hover-text, #fff) !important;
  border-color: var(--btn-primary-hover-border, transparent);
}
html.dark .fastclaw-chat-workspace .fastplan-conversation__download-action svg,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .fastplan-conversation__download-action svg,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .fastplan-conversation__download-action svg {
  stroke: currentColor !important;
  color: inherit !important;
}

/* 情景主题（除雾灰调）：jcr-message__main 内结果条（含失败态，不限 complete） */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message__main .fastplan-conversation__result,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message__main .fastplan-conversation__result {
  background: var(--card-background-tmd) !important;
  border: 1px solid var(--border-color) !important;
}

/* 情景主题（除雾灰调）：错误提示文字 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message-error__hint,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message-error__hint {
  color: #aa1032 !important;
}

/* 情景主题（除雾灰调）：markdown blockquote 竖线 + 文字 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-markdown blockquote,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-markdown blockquote {
  color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
  border-left-color: color-mix(in srgb, var(--jcr-fg, var(--text-primary)) 98%, var(--jcr-muted, var(--text-secondary))) !important;
}

/* 情景主题（除雾灰调）：「查看完整内容」白字；雾灰调 / 浅色 / 暗色仍用 --jcr-primary */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-show-full,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-show-full:hover,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-show-full:focus,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-show-full,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-show-full:hover,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-show-full:focus {
  color: #fff !important;
}

/* 情景主题（除雾灰调）+ 黑暗：所有代码块（含 python 的 .hljs 深色底）与 bash 同色 */
html.dark .fastclaw-chat-workspace .jcr-message.jcr-message--assistant,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message.jcr-message--assistant,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message.jcr-message--assistant {
  --jcr-code-bg: var(--jcr-user-bg, var(--theme-question-bubble));
  --md-theme-code-block-bg-color: var(--jcr-user-bg, var(--theme-question-bubble));
}
html.dark .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code,
html.dark .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code pre,
html.dark .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code pre code,
html.dark .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code .hljs,
html.dark .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code .md-editor-code-block,
html.dark .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code [class*="language-"],
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code pre,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code pre code,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code .hljs,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code .md-editor-code-block,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code [class*="language-"],
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code pre,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code pre code,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code .hljs,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code .md-editor-code-block,
html.light[data-theme-id="custom-scene"] .fastclaw-chat-workspace .jcr-message.jcr-message--assistant .md-editor-code [class*="language-"] {
  /* background: var(--jcr-code-bg) !important;
  background-color: var(--jcr-code-bg) !important; */
  border-top: 1px solid transparent !important;
}

/* 全主题：插入新任务节点弹窗（teleport 到 body；变量随主题切换） */
.fpp-ui .fixed.inset-0.z-\[200\] .bg-white.rounded-2xl.shadow-2xl.p-6.w-full.max-w-sm {
  background: var(--theme-dailog-color) !important;
  background-color: var(--theme-dailog-color) !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] .font-bold.text-gray-800.mb-4.flex.items-center.gap-2 {
  color: var(--text-primary) !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] .font-bold.text-gray-800.mb-4.flex.items-center.gap-2 .text-blue-600 {
  color: var(--color-primary) !important;
  stroke: currentColor !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] .text-xs.font-semibold.text-gray-500.uppercase.tracking-wider.block.mb-1\.5 {
  color: var(--text-primary) !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] input.w-full.px-3.py-2\.5.border.border-gray-200.rounded-xl.text-sm {
  border-radius: 4px !important;
  background: transparent !important;
  background-color: transparent !important;
  color: var(--text-primary) !important;
  border-color: var(--border-color) !important;
  box-shadow: none !important;
  --tw-ring-shadow: 0 0 #0000 !important;
  --tw-ring-color: transparent !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] input.w-full.px-3.py-2\.5.border.border-gray-200.rounded-xl.text-sm:focus,
.fpp-ui .fixed.inset-0.z-\[200\] input.w-full.px-3.py-2\.5.border.border-gray-200.rounded-xl.text-sm:focus-visible {
  border-color: var(--border-color) !important;
  box-shadow: none !important;
  outline: none !important;
  --tw-ring-shadow: 0 0 #0000 !important;
  --tw-ring-color: transparent !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] input.w-full.px-3.py-2\.5.border.border-gray-200.rounded-xl.text-sm::placeholder {
  color: color-mix(in srgb, var(--text-primary) 50%, transparent) !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] button.flex.items-center.gap-2.px-3.py-2.rounded-xl.border.text-xs.font-medium.transition-all {
  border-radius: 4px !important;
  border-color: var(--text-primary) !important;
  color: var(--text-primary) !important;
  background: transparent !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] button.flex.items-center.gap-2.px-3.py-2.rounded-xl.border.text-xs.font-medium.transition-all svg {
  color: var(--text-primary) !important;
  stroke: currentColor !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] button.flex.items-center.gap-2.px-3.py-2.rounded-xl.border.text-xs.font-medium.transition-all.border-blue-400,
.fpp-ui .fixed.inset-0.z-\[200\] button.flex.items-center.gap-2.px-3.py-2.rounded-xl.border.text-xs.font-medium.transition-all.bg-blue-50,
.fpp-ui .fixed.inset-0.z-\[200\] button.flex.items-center.gap-2.px-3.py-2.rounded-xl.border.text-xs.font-medium.transition-all.text-blue-700 {
  border-color: var(--color-primary) !important;
  color: var(--color-primary) !important;
  background: color-mix(in srgb, var(--color-primary) 10%, transparent) !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] button.flex.items-center.gap-2.px-3.py-2.rounded-xl.border.text-xs.font-medium.transition-all.border-blue-400 svg,
.fpp-ui .fixed.inset-0.z-\[200\] button.flex.items-center.gap-2.px-3.py-2.rounded-xl.border.text-xs.font-medium.transition-all.bg-blue-50 svg,
.fpp-ui .fixed.inset-0.z-\[200\] button.flex.items-center.gap-2.px-3.py-2.rounded-xl.border.text-xs.font-medium.transition-all.text-blue-700 svg {
  color: var(--color-primary) !important;
  stroke: currentColor !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] .flex.gap-2.mt-5 {
  justify-content: flex-end !important;
  gap: 8px !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] button.flex-1.px-4.py-2\.5.bg-blue-600.text-white.rounded-xl.text-sm.font-medium {
  flex: 0 0 auto !important;
  width: 25% !important;
  max-width: 120px !important;
  min-width: 76px !important;
  border-radius: 4px !important;
  color: var(--btn-primary-text, #fff) !important;
  background: var(--btn-primary-bg, var(--color-primary)) !important;
  background-color: var(--btn-primary-bg, var(--color-primary)) !important;
  border: 1px solid var(--btn-primary-border, var(--color-primary)) !important;
  box-shadow: none !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] button.flex-1.px-4.py-2\.5.bg-blue-600.text-white.rounded-xl.text-sm.font-medium:hover {
  color: var(--btn-primary-hover-text, #fff) !important;
  background: var(--btn-primary-hover-bg, var(--color-primary)) !important;
  background-color: var(--btn-primary-hover-bg, var(--color-primary)) !important;
  border-color: var(--btn-primary-hover-border, var(--color-primary)) !important;
  filter: none;
  transform: translateY(-1px);
  box-shadow: 0 10px 24px color-mix(in srgb, var(--btn-primary-hover-bg, var(--color-primary)) 28%, transparent) !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] button.flex-1.px-4.py-2\.5.border.border-gray-200.text-gray-600.rounded-xl {
  flex: 0 0 auto !important;
  width: 25% !important;
  max-width: 120px !important;
  min-width: 76px !important;
  border-radius: 4px !important;
  color: var(--btn-secondary-text, #fff) !important;
  background: var(--cancel-btn-color, transparent) !important;
  background-color: var(--cancel-btn-color, transparent) !important;
  border: 1px solid var(--btn-secondary-border, rgba(255, 255, 255, 0.55)) !important;
  box-shadow: none !important;
}
.fpp-ui .fixed.inset-0.z-\[200\] button.flex-1.px-4.py-2\.5.border.border-gray-200.text-gray-600.rounded-xl:hover {
  color: var(--btn-secondary-hover-text, #fff) !important;
  background: var(--cancel-btn-hover-color, rgba(255, 255, 255, 0.08)) !important;
  background-color: var(--cancel-btn-hover-color, rgba(255, 255, 255, 0.08)) !important;
  border-color: var(--btn-secondary-hover-border, rgba(255, 255, 255, 0.7)) !important;
}

/* 全主题：节点编辑侧栏输入框 / 按钮圆角 4px；聚焦时边框不变色 */
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input,
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea,
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select,
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button {
  border-radius: 4px !important;
}
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input:focus,
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input:focus-visible,
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea:focus,
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea:focus-visible,
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select:focus,
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select:focus-visible {
  border-color: var(--border-color, #e5e7eb) !important;
  box-shadow: none !important;
  outline: none !important;
  --tw-ring-shadow: 0 0 #0000 !important;
  --tw-ring-color: transparent !important;
}
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.text-xs.flex.items-center.gap-1.px-2.py-1.rounded-lg.transition-colors {
  border-radius: 4px !important;
}
/* 节点名称输入框底部分隔线 → --border-color（覆盖 hover 亮灰 / focus 蓝色） */
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.w-full.text-lg.font-bold.border-b-2,
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.w-full.text-lg.font-bold.border-b-2:hover,
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.w-full.text-lg.font-bold.border-b-2:focus,
.fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.w-full.text-lg.font-bold.border-b-2:focus-visible {
  border-color: var(--border-color) !important;
}

/* 全浅色主题（含浅色 / 雾灰调 / 情景）+ 下方暗色：节点编辑侧栏随主题变量变色 */
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\].bg-white,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\].bg-white {
  background: var(--theme-dailog-color) !important;
  color: var(--text-primary) !important;
  border-left: 1px solid var(--border-color) !important;
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-b.border-gray-100,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-t.border-gray-100,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-gray-100,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-gray-200,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-b.border-gray-100,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-t.border-gray-100,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-gray-100,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-gray-200 {
  border-color: var(--border-color) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-900,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-800,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-700,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-600,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-500,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-900,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-800,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-700,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-500 {
  color: var(--text-primary) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-400,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-400 {
  color: color-mix(in srgb, var(--text-primary) 55%, transparent) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-blue-500,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-blue-600,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-blue-700,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-amber-600,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-amber-700,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-violet-500,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-violet-600,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-violet-700,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:text-blue-700:hover,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:text-amber-700:hover,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:text-violet-700:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-blue-500,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-blue-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-blue-700,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-amber-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-amber-700,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-violet-500,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-violet-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-violet-700,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:text-blue-700:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:text-amber-700:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:text-violet-700:hover {
  color: var(--color-primary) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea {
  background: transparent !important;
  color: var(--text-primary) !important;
  box-shadow: none !important;
  --tw-ring-shadow: 0 0 #0000 !important;
  --tw-ring-color: transparent !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.border.border-gray-200,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea.border.border-gray-200,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.border.border-gray-200,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea.border.border-gray-200 {
  border-color: var(--border-color) !important;
  border-radius: 4px !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input:focus,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea:focus,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input:focus,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea:focus {
  background: transparent !important;
  box-shadow: none !important;
  outline: none !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.border.border-gray-200:focus,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea.border.border-gray-200:focus,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.border.border-gray-200:focus,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea.border.border-gray-200:focus {
  border-color: var(--border-color) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input::placeholder,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea::placeholder,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input::placeholder,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea::placeholder {
  color: color-mix(in srgb, var(--text-primary) 45%, transparent) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-white,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-gray-50,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-gray-50\/40,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-gray-200,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-blue-50,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-blue-50\/40,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-blue-100,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-amber-50,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-amber-100,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-violet-50,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-violet-50\/30,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-white,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-gray-50,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-gray-200,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-blue-50,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-blue-50\/40,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-blue-100,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-amber-50,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-amber-100,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-violet-50,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-violet-50\/30 {
  background: color-mix(in srgb, var(--text-primary) 6%, transparent) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-blue-100,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-blue-200,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-amber-200,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-violet-100,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-violet-200,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-blue-100,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-blue-200,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-amber-200,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-violet-100,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-violet-200 {
  border-color: var(--border-color) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-gray-100:hover,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-gray-50:hover,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-blue-50:hover,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-amber-50:hover,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-violet-50:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-gray-100:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-gray-50:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-blue-50:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-amber-50:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-violet-50:hover {
  background: var(--dropdown-hover-color, color-mix(in srgb, var(--text-primary) 10%, transparent)) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.rounded-lg.border:not(.bg-violet-600):not(.text-white),
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.rounded-xl.border:not(.bg-violet-600):not(.text-white),
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.rounded-lg.border:not(.bg-violet-600):not(.text-white),
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.rounded-xl.border:not(.bg-violet-600):not(.text-white) {
  border-radius: 4px !important;
  border-color: var(--border-color) !important;
  color: var(--text-primary) !important;
  background: transparent !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.rounded-lg.border.bg-blue-600,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.rounded-lg.border.border-blue-600,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.rounded-lg.border.text-white,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.bg-blue-600,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.bg-amber-500,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.bg-violet-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.rounded-lg.border.bg-blue-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.rounded-lg.border.border-blue-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.bg-blue-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.bg-amber-500,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.bg-violet-600 {
  border-radius: 4px !important;
  background: var(--color-primary) !important;
  border-color: var(--color-primary) !important;
  color: #fff !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.hover\:bg-gray-100,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.hover\:bg-gray-50,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.hover\:bg-gray-100,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.hover\:bg-gray-50 {
  border-radius: 4px !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.text-gray-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.text-gray-600 {
  border-radius: 4px !important;
  color: var(--btn-secondary-text, #fff) !important;
  background: var(--cancel-btn-color, transparent) !important;
  border: 1px solid var(--btn-secondary-border, rgba(255, 255, 255, 0.55)) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.text-gray-600:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.text-gray-600:hover {
  color: var(--btn-secondary-hover-text, #fff) !important;
  background: var(--cancel-btn-hover-color, rgba(255, 255, 255, 0.08)) !important;
  border-color: var(--btn-secondary-hover-border, rgba(255, 255, 255, 0.7)) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.border.border-gray-200,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.bg-blue-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.border.border-gray-200,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.bg-blue-600 {
  border-radius: 4px !important;
  color: var(--btn-primary-text, #fff) !important;
  background: var(--btn-primary-bg, var(--color-primary)) !important;
  border: 1px solid var(--btn-primary-border, var(--color-primary)) !important;
  box-shadow: none !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.border.border-gray-200:hover,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.bg-blue-600:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.border.border-gray-200:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.bg-blue-600:hover {
  color: var(--btn-primary-hover-text, #fff) !important;
  background: var(--btn-primary-hover-bg, var(--color-primary)) !important;
  border-color: var(--btn-primary-hover-border, var(--color-primary)) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] code,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] code {
  background: color-mix(in srgb, var(--text-primary) 10%, transparent) !important;
  color: var(--text-primary) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .h-1.w-full,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .h-1.w-full {
  background: var(--color-primary) !important;
}

/* 浅色（含雾灰）+ 暗色：类型标题 / 未保存徽标 / 图标 / 选择节点等链接 → --text-primary；输入框背景始终透明 */
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-xs.font-semibold.tracking-wide.uppercase.text-blue-700,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-xs.font-semibold.tracking-wide.uppercase.text-amber-700,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-xs.font-semibold.tracking-wide.uppercase.text-violet-700,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-xs.font-semibold.tracking-wide.uppercase.text-blue-700,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-xs.font-semibold.tracking-wide.uppercase.text-amber-700,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-xs.font-semibold.tracking-wide.uppercase.text-violet-700,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-xs.font-semibold.tracking-wide.uppercase.text-blue-700,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-xs.font-semibold.tracking-wide.uppercase.text-amber-700,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-xs.font-semibold.tracking-wide.uppercase.text-violet-700 {
  color: var(--text-primary) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-\[10px\].px-1\.5.py-0\.5.bg-amber-100.text-amber-600.rounded-full.font-medium,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-\[10px\].px-1\.5.py-0\.5.bg-amber-100.text-amber-600.rounded-full.font-medium,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-\[10px\].px-1\.5.py-0\.5.bg-amber-100.text-amber-600.rounded-full.font-medium {
  color: var(--text-primary) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-blue-50,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-amber-50,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-violet-50,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-blue-50,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-amber-50,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-violet-50,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-blue-50,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-amber-50,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-violet-50 {
  color: var(--text-primary) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-blue-50 svg,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-amber-50 svg,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-violet-50 svg,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-blue-50 .text-blue-700,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-amber-50 .text-amber-700,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-violet-50 .text-violet-700,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-blue-50 svg,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-amber-50 svg,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-violet-50 svg,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-blue-50 .text-blue-700,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-amber-50 .text-amber-700,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-violet-50 .text-violet-700,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-blue-50 svg,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-amber-50 svg,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-violet-50 svg,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-blue-50 .text-blue-700,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-amber-50 .text-amber-700,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-violet-50 .text-violet-700 {
  color: var(--text-primary) !important;
  stroke: currentColor !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.text-xs.flex.items-center.gap-1.px-2.py-1.rounded-lg.transition-colors,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.text-xs.flex.items-center.gap-1.px-2.py-1.rounded-lg.transition-colors:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.text-xs.flex.items-center.gap-1.px-2.py-1.rounded-lg.transition-colors,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.text-xs.flex.items-center.gap-1.px-2.py-1.rounded-lg.transition-colors:hover,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.text-xs.flex.items-center.gap-1.px-2.py-1.rounded-lg.transition-colors,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.text-xs.flex.items-center.gap-1.px-2.py-1.rounded-lg.transition-colors:hover {
  color: var(--text-primary) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.text-xs.flex.items-center.gap-1.px-2.py-1.rounded-lg.transition-colors svg,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.text-xs.flex.items-center.gap-1.px-2.py-1.rounded-lg.transition-colors svg,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.text-xs.flex.items-center.gap-1.px-2.py-1.rounded-lg.transition-colors svg {
  color: var(--text-primary) !important;
  stroke: currentColor !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.bg-white,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.bg-gray-50,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea.bg-gray-50,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input:focus,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea:focus,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.bg-white,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.bg-gray-50,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea.bg-gray-50,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input:focus,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea:focus,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.bg-white,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.bg-gray-50,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea.bg-gray-50,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input:focus,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea:focus {
  background: transparent !important;
  background-color: transparent !important;
}

/* 浅色（含雾灰）+ 暗色：工具浏览分段 / flex gap-2 选中态 / 勾选框 / 下拉框 */
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.items-center.gap-1.bg-gray-100.rounded-lg.p-0\.5.w-fit,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.items-center.gap-1.bg-gray-100.rounded-lg.p-0\.5.w-fit,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.items-center.gap-1.bg-gray-100.rounded-lg.p-0\.5.w-fit {
  background: var(--card-background-tmd) !important;
  background-color: var(--card-background-tmd) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.items-center.gap-1.bg-gray-100.rounded-lg.p-0\.5.w-fit button.bg-white,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.items-center.gap-1.bg-gray-100.rounded-lg.p-0\.5.w-fit button.text-violet-700,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.items-center.gap-1.bg-gray-100.rounded-lg.p-0\.5.w-fit button.bg-white,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.items-center.gap-1.bg-gray-100.rounded-lg.p-0\.5.w-fit button.text-violet-700,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.items-center.gap-1.bg-gray-100.rounded-lg.p-0\.5.w-fit button.bg-white,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.items-center.gap-1.bg-gray-100.rounded-lg.p-0\.5.w-fit button.text-violet-700 {
  background: var(--color-primary) !important;
  background-color: var(--color-primary) !important;
  color: #fff !important;
  box-shadow: none !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.bg-violet-600,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.bg-blue-600,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.bg-amber-500,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.border-violet-600.text-white,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.border-blue-600.text-white,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2.flex-wrap button.bg-violet-600,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2.flex-wrap button.bg-blue-600,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2.flex-wrap button.bg-amber-500,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2.flex-wrap button.border-blue-600.text-white,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.bg-violet-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.bg-blue-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.bg-amber-500,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.border-violet-600.text-white,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.border-blue-600.text-white,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2.flex-wrap button.bg-violet-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2.flex-wrap button.bg-blue-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2.flex-wrap button.bg-amber-500,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2.flex-wrap button.border-blue-600.text-white,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.bg-violet-600,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.bg-blue-600,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.bg-amber-500,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.border-violet-600.text-white,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2 button.border-blue-600.text-white,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2.flex-wrap button.bg-violet-600,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2.flex-wrap button.bg-blue-600,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2.flex-wrap button.bg-amber-500,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex.gap-2.flex-wrap button.border-blue-600.text-white {
  background: var(--color-primary) !important;
  background-color: var(--color-primary) !important;
  border-color: var(--color-primary) !important;
  color: #fff !important;
}
/* 执行方式选中 / 测试执行：图标与文字同为白色 */
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-t.border-gray-100.pt-5 button.bg-violet-600 svg,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-t.border-gray-100.pt-5 button.text-white svg,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex.items-center.gap-2.px-4.py-2.bg-violet-600 svg,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-t.border-gray-100.pt-5 button.bg-violet-600 svg,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-t.border-gray-100.pt-5 button.text-white svg,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex.items-center.gap-2.px-4.py-2.bg-violet-600 svg,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-t.border-gray-100.pt-5 button.bg-violet-600 svg,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-t.border-gray-100.pt-5 button.text-white svg,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex.items-center.gap-2.px-4.py-2.bg-violet-600 svg {
  color: #fff !important;
  stroke: #fff !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input[type="checkbox"].w-4.h-4.text-violet-600,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input[type="checkbox"].w-4.h-4.text-violet-600,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input[type="checkbox"].w-4.h-4.text-violet-600 {
  accent-color: var(--color-primary) !important;
  color: var(--color-primary) !important;
}
/* 浅色（含雾灰）+ 暗色：opt-in base-select，才能改下拉窗口边框与 option 悬浮色 */
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select::picker(select),
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select::picker(select),
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select::picker(select) {
  appearance: base-select;
  -webkit-appearance: base-select;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select.w-full.px-4.py-2\.5.border.border-gray-200.rounded-xl.text-sm,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select.w-full.px-4.py-2\.5.border.border-gray-200.rounded-xl.text-sm,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select.w-full.px-4.py-2\.5.border.border-gray-200.rounded-xl.text-sm {
  background: transparent !important;
  background-color: transparent !important;
  border: 1px solid var(--border-color) !important;
  border-color: var(--border-color) !important;
  color: var(--text-primary) !important;
  border-radius: 4px !important;
  box-shadow: none !important;
  --tw-ring-shadow: 0 0 #0000 !important;
  --tw-ring-color: transparent !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select.w-full.px-4.py-2\.5.border.border-gray-200.rounded-xl.text-sm:focus,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select.w-full.px-4.py-2\.5.border.border-gray-200.rounded-xl.text-sm:focus,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select.w-full.px-4.py-2\.5.border.border-gray-200.rounded-xl.text-sm:focus {
  background: transparent !important;
  border-color: var(--border-color) !important;
  box-shadow: none !important;
  outline: none !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select::picker(select),
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select::picker(select),
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select::picker(select) {
  border: 1px solid var(--border-color) !important;
  background: var(--theme-dailog-color) !important;
  background-color: var(--theme-dailog-color) !important;
  color: var(--text-primary) !important;
  border-radius: 4px !important;
  padding: 4px !important;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select option,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select option,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select option {
  background: transparent !important;
  color: var(--text-primary) !important;
  border-radius: 4px !important;
  padding: 6px 10px !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select option:hover,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select option:checked,
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select option:focus,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select option:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select option:checked,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select option:focus,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select option:hover,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select option:checked,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] select option:focus {
  background: var(--dropdown-hover-color) !important;
  background-color: var(--dropdown-hover-color) !important;
  color: var(--text-primary) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex-shrink-0.p-1\.5.text-gray-300.hover\:text-red-500.hover\:bg-red-50.rounded-lg.transition-colors,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex-shrink-0.p-1\.5.text-gray-300.hover\:text-red-500.hover\:bg-red-50.rounded-lg.transition-colors,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex-shrink-0.p-1\.5.text-gray-300.hover\:text-red-500.hover\:bg-red-50.rounded-lg.transition-colors {
  color: var(--text-primary) !important;
  background: transparent !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex-shrink-0.p-1\.5.text-gray-300.hover\:text-red-500.hover\:bg-red-50.rounded-lg.transition-colors svg,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex-shrink-0.p-1\.5.text-gray-300.hover\:text-red-500.hover\:bg-red-50.rounded-lg.transition-colors svg,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex-shrink-0.p-1\.5.text-gray-300.hover\:text-red-500.hover\:bg-red-50.rounded-lg.transition-colors svg {
  color: var(--text-primary) !important;
  stroke: currentColor !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex-shrink-0.p-1\.5.text-gray-300.hover\:text-red-500.hover\:bg-red-50.rounded-lg.transition-colors:hover,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex-shrink-0.p-1\.5.text-gray-300.hover\:text-red-500.hover\:bg-red-50.rounded-lg.transition-colors:hover,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex-shrink-0.p-1\.5.text-gray-300.hover\:text-red-500.hover\:bg-red-50.rounded-lg.transition-colors:hover {
  color: var(--text-primary) !important;
  background: var(--dropdown-hover-color) !important;
  background-color: var(--dropdown-hover-color) !important;
}
html.light .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex-shrink-0.p-1\.5.text-gray-300.hover\:text-red-500.hover\:bg-red-50.rounded-lg.transition-colors:hover svg,
html.light[data-theme-id="custom-scene"] .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex-shrink-0.p-1\.5.text-gray-300.hover\:text-red-500.hover\:bg-red-50.rounded-lg.transition-colors:hover svg,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.flex-shrink-0.p-1\.5.text-gray-300.hover\:text-red-500.hover\:bg-red-50.rounded-lg.transition-colors:hover svg {
  color: var(--text-primary) !important;
  stroke: currentColor !important;
}

/*
 * 暗色主题：全局 main.css 有
 *   .dark .bg-white[class*="shadow"] { background: rgba(255,255,255,0.05) !important; }
 * FastPlan 的 DAG 右键菜单 / 节点编辑侧栏 / 插入弹窗都是 bg-white + shadow-*，
 * 会被打成近乎透明。这里用实心 --theme-dailog-color 盖回去。
 */
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 {
  background: var(--theme-dailog-color) !important;
  background-color: var(--theme-dailog-color) !important;
  border-color: var(--border-color) !important;
  color: var(--text-primary) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 .border-b.border-gray-100,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 hr,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 hr.border-gray-100 {
  border-color: var(--border-color) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 button,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 [class*="text-"] {
  color: var(--text-primary) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 button.text-blue-600,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 .text-blue-600 {
  color: var(--color-primary) !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 button.text-red-500,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 .text-red-500,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 button.text-red-600,
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 .text-red-600 {
  color: #e11d48 !important;
}
html.dark .fastclaw-chat-workspace .jcr-message__main .p-4.h-\[500px\] .absolute.bg-white.border.border-gray-200.rounded-xl.shadow-2xl.z-50 button:hover {
  background: var(--dropdown-hover-color) !important;
  background-color: var(--dropdown-hover-color) !important;
  color: var(--text-primary) !important;
}

html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\].bg-white {
  background: var(--theme-dailog-color) !important;
  background-color: var(--theme-dailog-color) !important;
  color: var(--text-primary) !important;
  border-left: 1px solid var(--border-color) !important;
  box-shadow: 0 24px 70px rgba(0, 0, 0, 0.45) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-b.border-gray-100,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-t.border-gray-100,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-gray-100,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-gray-200,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-blue-100,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-blue-200 {
  border-color: var(--border-color) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-900,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-800,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-700,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-600,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-500 {
  color: var(--text-primary) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-gray-400 {
  color: color-mix(in srgb, var(--text-primary) 55%, transparent) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-blue-600,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-blue-700,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-amber-600,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-amber-700,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-violet-500,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-violet-600,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-violet-700 {
  color: var(--color-primary) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-white,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-gray-50,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-gray-50\/40,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-gray-100,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-gray-200,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-blue-50,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-blue-50\/40,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-blue-100,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-amber-50,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-amber-100,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-violet-50,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-violet-50\/30,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-violet-50\/60,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .bg-violet-200 {
  background: color-mix(in srgb, var(--text-primary) 6%, transparent) !important;
  background-color: color-mix(in srgb, var(--text-primary) 6%, transparent) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-amber-200,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-violet-100,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .border-violet-200 {
  border-color: var(--border-color) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] code.bg-gray-100,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] code.bg-blue-100,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] code {
  background: color-mix(in srgb, var(--text-primary) 10%, transparent) !important;
  color: var(--text-primary) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-xs.text-amber-600.bg-amber-50.border.border-amber-200 {
  color: var(--text-primary) !important;
  background: color-mix(in srgb, var(--text-primary) 6%, transparent) !important;
  border-color: var(--border-color) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .text-\[10px\].px-1\.5.py-0\.5.bg-amber-100.text-amber-600.rounded-full.font-medium {
  color: var(--text-primary) !important;
  background: color-mix(in srgb, var(--text-primary) 6%, transparent) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-blue-50,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-amber-50,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-violet-50 {
  background: color-mix(in srgb, var(--text-primary) 6%, transparent) !important;
  color: var(--text-primary) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-blue-50 svg,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-amber-50 svg,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg.bg-violet-50 svg,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg .text-blue-700,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg .text-amber-700,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .p-1\.5.rounded-lg .text-violet-700 {
  color: var(--text-primary) !important;
  stroke: currentColor !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea {
  background: transparent !important;
  color: var(--text-primary) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] input.border.border-gray-200,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] textarea.border.border-gray-200 {
  border-color: var(--border-color) !important;
  border-radius: 4px !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-gray-100:hover,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-gray-50:hover,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-blue-50:hover,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-amber-50:hover,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-violet-50:hover,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .hover\:bg-red-50:hover {
  background: var(--dropdown-hover-color, color-mix(in srgb, var(--text-primary) 10%, transparent)) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .h-1.w-full {
  background: var(--color-primary) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.bg-blue-600,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.bg-amber-500,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.bg-violet-600,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] button.border-blue-600.text-white {
  background: var(--color-primary) !important;
  border-color: var(--color-primary) !important;
  color: #fff !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.text-gray-600 {
  border-radius: 4px !important;
  color: var(--btn-secondary-text, #fff) !important;
  background: var(--cancel-btn-color, transparent) !important;
  border: 1px solid var(--btn-secondary-border, rgba(255, 255, 255, 0.55)) !important;
}
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.border.border-gray-200,
html.dark .fpp-ui .fixed.top-0.right-0.h-full.z-\[100\] .flex-shrink-0.border-t button.px-4.py-2.text-sm.bg-blue-600 {
  border-radius: 4px !important;
  color: var(--btn-primary-text, #fff) !important;
  background: var(--btn-primary-bg, var(--color-primary)) !important;
  border: 1px solid var(--btn-primary-border, var(--color-primary)) !important;
}

/* 供应商参数表单：聚焦不改边框色；API Key 用自定义显隐，隐藏浏览器原生眼睛 */
.jcr-parameter-form {
  --parameter-border: var(--jcr-parameter-border, #ffdb77);
  display: grid;
  gap: 10px;
  padding: 16px;
  border: 1px solid var(--parameter-border);
  border-radius: 8px;
  background: var(--jcr-parameter-bg, color-mix(in srgb, #fff3c4 35%, var(--jcr-bg, #fff)));
  color: var(--jcr-fg, #303133);
  min-width: 0;
}
.jcr-parameter-form__description {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin: 0 0 2px;
  color: var(--jcr-parameter-notice, #c86a10);
  font-size: 13px;
  line-height: 1.6;
}
.jcr-parameter-form__notice-icon {
  flex: 0 0 16px;
  width: 16px;
  height: 16px;
  margin-top: 2px;
  color: #e6a23c;
}
.jcr-parameter-form__field {
  display: grid;
  grid-template-columns: 68px minmax(0, 1fr);
  align-items: center;
  gap: 12px;
  font-size: 13px;
}
.jcr-parameter-form__field > span {
  white-space: nowrap;
}
.jcr-parameter-form__field input {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  height: 34px;
  border: 1px solid var(--parameter-border);
  border-radius: 5px;
  background: var(--jcr-bg, #fff);
  color: inherit;
  padding: 6px 12px;
  font: inherit;
}
.jcr-parameter-form__field input:hover,
.jcr-parameter-form__field input:focus,
.jcr-parameter-form__field input:focus-visible,
.jcr-parameter-form__field input:active {
  outline: none !important;
  box-shadow: none !important;
  border-color: var(--parameter-border) !important;
}
.jcr-parameter-form input[type="password"]::-ms-reveal,
.jcr-parameter-form input[type="password"]::-ms-clear,
.jcr-parameter-form input[type="password"]::-webkit-credentials-auto-fill-button,
.jcr-parameter-form input[type="password"]::-webkit-strong-password-auto-fill-button,
.jcr-parameter-form input[type="password"]::-webkit-password-toggle-button,
.jcr-parameter-form input[type="password"]::-webkit-caps-lock-indicator {
  display: none !important;
  visibility: hidden !important;
  pointer-events: none !important;
  appearance: none !important;
  -webkit-appearance: none !important;
  width: 0 !important;
  height: 0 !important;
  margin: 0 !important;
  padding: 0 !important;
  opacity: 0 !important;
}
.fastclaw-api-key-wrap {
  position: relative;
  min-width: 0;
}
.fastclaw-api-key-wrap input {
  padding-right: 36px;
}
.fastclaw-api-key-toggle {
  position: absolute;
  top: 50%;
  right: 6px;
  z-index: 1;
  display: flex;
  width: 24px;
  height: 24px;
  align-items: center;
  justify-content: center;
  padding: 0 !important;
  border: 0 !important;
  border-radius: 4px;
  background: transparent !important;
  color: var(--text-secondary, #909399);
  cursor: pointer;
  transform: translateY(-50%);
}
.fastclaw-api-key-toggle:hover,
.fastclaw-api-key-toggle:focus,
.fastclaw-api-key-toggle:focus-visible {
  color: var(--text-primary, #303133);
  background: transparent !important;
  outline: none;
  box-shadow: none;
}
.jcr-parameter-form__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.jcr-parameter-form__actions button {
  border: 1px solid var(--jcr-border, #dcdfe6);
  border-radius: 5px;
  padding: 7px 16px;
  font: inherit;
  font-size: 13px;
  color: inherit;
  background: var(--jcr-bg, #fff);
  cursor: pointer;
}
.jcr-parameter-form__actions button[type=submit] {
  background: var(--jcr-primary, #409eff);
  border-color: transparent;
  color: white;
}
.jcr-parameter-form__actions button:disabled {
  opacity: .5;
  cursor: not-allowed;
}
.jcr-parameter-form__error {
  color: var(--jcr-danger, #dc2626);
  margin: 0;
  font-size: 13px;
}

</style>
