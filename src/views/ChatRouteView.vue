<script setup lang="ts">
import { defineAsyncComponent, provide } from 'vue'
import ChatRoutePlaceholder from './ChatRoutePlaceholder.vue'
import { CHAT_VIEW_IDENTITY, useChatViewIdentity } from '@/modules/chat/viewIdentity'

// App 已提供缓存记录身份；独立路由宿主也必须在异步子组件加载前捕获。
provide(CHAT_VIEW_IDENTITY, useChatViewIdentity())

const ChatView = defineAsyncComponent({
  loader: () => import('./ChatView.vue'),
  loadingComponent: ChatRoutePlaceholder,
  delay: 0,
})
</script>

<template>
  <!-- Start mounting immediately. The previous rAF + timer gate postponed every
       cold ChatView by at least one paint and amplified an already blocked main
       thread into a longer blank interval. defineAsyncComponent still renders
       ChatRoutePlaceholder while the code chunk itself is loading. -->
  <ChatView />
</template>
