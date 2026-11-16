import { createRouter, createWebHistory } from 'vue-router'
import { resolveStartupRoute } from '@/utils/tabSession'
import { redirectDuplicateConversation } from '@/utils/chatNavigation'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    // 直接进上次激活 tab，避免 / → /chat → /chat/:id 两次挂载 ChatView 导致闪白
    { path: '/', redirect: () => resolveStartupRoute() },
    { path: '/chat', name: 'Chat', component: () => import('@/views/ChatRouteView.vue'), meta: { title: 'sidebar.smartChat', group: 'ai', keepAlive: true } },
    { path: '/chat/:iid', name: 'ChatInstance', component: () => import('@/views/ChatRouteView.vue'), meta: { title: 'sidebar.smartChat', group: 'ai', keepAlive: true } },

    { path: '/alltool', name: 'alltool', component: () => import('@/views/AllToolView.vue'), meta: { title: 'sidebar.allTools', group: 'feature' } },
    { path: '/skills', name: 'skills', component: () => import('@/views/SkillsView.vue'), meta: { title: 'sidebar.skillsManage', group: 'feature' } },
    { path: '/mcp', name: 'mcp', component: () => import('@/views/McpView.vue'), meta: { title: 'sidebar.mcpService', group: 'feature' } },
    { path: '/wsp-admin', name: 'wsp', component: () => import('@/views/WspView.vue'), meta: { title: 'sidebar.wspManage', group: 'feature' } },

    { path: '/knowledge', name: 'Knowledge', component: () => import('@/views/KnowledgeView.vue'), meta: { title: 'sidebar.knowledge', group: 'knowledge' } },
    { path: '/knowledge-bases/:id', name: 'KnowledgeBaseDetail', component: () => import('@/views/knowledge/KnowledgeBaseDetail.vue'), meta: { title: 'sidebar.knowledgeData', group: 'knowledge' } },
    { path: '/files', name: 'Files', component: () => import('@/views/files/FileList.vue'), meta: { title: 'sidebar.fileManage', group: 'knowledge' } },
    { path: '/governance', name: 'Governance', component: () => import('@/views/governance/GovernanceList.vue'), meta: { title: 'sidebar.govDoc', group: 'knowledge' } },
    { path: '/governance/:id', name: 'GovernanceDetail', component: () => import('@/views/governance/GovernanceDetail.vue'), meta: { title: 'sidebar.govDetail', group: 'knowledge' } },
    { path: '/chunk-tool', name: 'ChunkTool', component: () => import('@/views/chunk-tool/ChunkTool.vue'), meta: { title: 'sidebar.sliceTool', group: 'knowledge' } },
    { path: '/chunk-viewer', name: 'ChunkViewer', component: () => import('@/views/knowledge/components/ChunkViewer.vue'), meta: { title: 'knowledge.chunkViewer', group: 'knowledge' } },
    { path: '/tasks', name: 'Tasks', component: () => import('@/views/tasks/TaskList.vue'), meta: { title: 'sidebar.taskManage', group: 'knowledge' } },

    { path: '/models', name: 'Models', component: () => import('@/views/ModelsView.vue'), meta: { title: 'sidebar.modelManage', group: 'feature' } },
    { path: '/dashboard', name: 'Dashboard', component: () => import('@/views/DashboardView.vue'), meta: { title: 'sidebar.callDashboard', group: 'feature' } },
    { path: '/settings', name: 'Settings', component: () => import('@/views/SettingsView.vue'), meta: { title: 'sidebar.settings', group: 'system' } },
    { path: '/:pathMatch(.*)*', redirect: '/chat' },
  ],
})

router.beforeEach(redirectDuplicateConversation)

export default router
