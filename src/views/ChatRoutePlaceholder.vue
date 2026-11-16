<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import logoSvg from '@/assets/logo.svg?raw'
import { appLogo } from '@/api/branding'

const { t } = useI18n()
</script>

<template>
  <div
    class="chat-route-placeholder"
    role="status"
    aria-live="polite"
    aria-busy="true"
    :aria-label="t('common.loading')"
  >
    <aside class="placeholder-aside" aria-hidden="true">
      <div class="placeholder-aside-head">
        <div class="placeholder-new-chat">
          <span class="placeholder-plus">+</span>
          <span>{{ t('chat.newChat') }}</span>
        </div>
        <div class="placeholder-collapse">
          <span />
          <span />
          <span />
        </div>
      </div>

      <div class="placeholder-history-title">{{ t('chat.history') }}</div>
      <div class="placeholder-history-list">
        <span class="placeholder-line placeholder-line--long" />
        <span class="placeholder-line placeholder-line--medium" />
        <span class="placeholder-line placeholder-line--short" />
      </div>
    </aside>

    <main class="placeholder-main" aria-hidden="true">
      <div class="placeholder-welcome">
        <img v-if="appLogo" :src="appLogo" alt="" class="placeholder-logo" />
        <span v-else v-html="logoSvg" class="placeholder-logo placeholder-logo-svg" />
        <span class="placeholder-welcome-line placeholder-welcome-line--title" />
        <span class="placeholder-welcome-line placeholder-welcome-line--subtitle" />
      </div>

      <div class="placeholder-composer">
        <span class="placeholder-composer-text">{{ t('chat.routePlaceholder') }}</span>
        <div class="placeholder-composer-actions">
          <span />
          <span />
          <span class="placeholder-send" />
        </div>
      </div>
    </main>
  </div>
</template>

<style scoped>
.chat-route-placeholder {
  display: flex;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  color: var(--text-primary);
  background: var(--bg-primary, #fff);
  pointer-events: none;
  user-select: none;
}

.placeholder-aside {
  width: 234px;
  flex: 0 0 234px;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  border-right: 1px solid var(--chat-divider-color, var(--border-color));
  background: var(--bg-submenu, var(--bg-primary, #fff));
}

.placeholder-aside-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px;
}

.placeholder-new-chat {
  height: 33px;
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  box-sizing: border-box;
  border-radius: 4px;
  font-size: var(--font-size-md, 14px);
  font-weight: 500;
  color: var(--btn-primary-text, #fff);
  background: var(--btn-primary-bg, var(--color-primary));
}

.placeholder-plus {
  font-size: 20px;
  font-weight: 300;
  line-height: 1;
}

.placeholder-collapse {
  width: 33px;
  height: 33px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 3px;
  box-sizing: border-box;
  border: 1px solid var(--border-color);
  border-radius: 4px;
}

.placeholder-collapse span {
  width: 14px;
  height: 1px;
  border-radius: 1px;
  background: var(--text-secondary);
}

.placeholder-history-title {
  padding: 4px 16px 8px;
  font-size: var(--font-size-lg, 16px);
  color: var(--text-secondary);
}

.placeholder-history-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 12px 18px;
}

.placeholder-line,
.placeholder-welcome-line {
  display: block;
  height: 10px;
  border-radius: 999px;
  background: var(--bg-hover, rgba(127, 127, 127, 0.14));
  animation: chat-route-pulse 1.25s ease-in-out infinite;
}

.placeholder-line--long { width: 82%; }
.placeholder-line--medium { width: 68%; }
.placeholder-line--short { width: 74%; }

.placeholder-main {
  position: relative;
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  padding: 48px 24px 112px;
  background: var(--bg-primary, #fff);
}

.placeholder-welcome {
  width: min(520px, 70%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
  transform: translateY(-8vh);
}

.placeholder-logo {
  width: 58px;
  height: 58px;
  display: block;
  flex-shrink: 0;
  margin-bottom: 4px;
  color: var(--color-primary);
  object-fit: contain;
}

.placeholder-logo-svg :deep(svg) {
  width: 100%;
  height: 100%;
  display: block;
}

.placeholder-welcome-line--title {
  width: min(320px, 70%);
  height: 17px;
}

.placeholder-welcome-line--subtitle {
  width: min(480px, 92%);
  height: 11px;
  animation-delay: 90ms;
}

.placeholder-composer {
  position: absolute;
  left: 50%;
  bottom: 24px;
  width: min(760px, calc(100% - 48px));
  min-height: 88px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  box-sizing: border-box;
  padding: 14px 16px 12px;
  border: 1px solid var(--border-color);
  border-radius: 14px;
  background: var(--bg-secondary, var(--bg-primary, #fff));
  box-shadow: 0 4px 18px rgba(0, 0, 0, 0.05);
  transform: translateX(-50%);
}

.placeholder-composer-text {
  overflow: hidden;
  font-size: var(--font-size-base, 14px);
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--text-tertiary, var(--text-secondary));
  opacity: 0.72;
}

.placeholder-composer-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  align-self: flex-end;
}

.placeholder-composer-actions span {
  width: 22px;
  height: 22px;
  display: block;
  border-radius: 50%;
  background: var(--bg-hover, rgba(127, 127, 127, 0.14));
}

.placeholder-composer-actions .placeholder-send {
  background: var(--color-primary);
  opacity: 0.7;
}

@keyframes chat-route-pulse {
  0%, 100% { opacity: 0.48; }
  50% { opacity: 0.9; }
}

@media (max-width: 720px) {
  .placeholder-aside { display: none; }
}

@media (prefers-reduced-motion: reduce) {
  .placeholder-line,
  .placeholder-welcome-line {
    animation: none;
  }
}
</style>
