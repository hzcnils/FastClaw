import type { ChatThemeTokens } from 'chat-renderer'

/** Values remain CSS variables so all six host themes and runtime switching update without remounting. */
export const FASTCLAW_CHAT_THEME_TOKENS: ChatThemeTokens = {
  background: 'var(--bg-primary)',
  surface: 'var(--theme-card-bg, var(--bg-primary))',
  subtleBackground: 'var(--bg-secondary, color-mix(in srgb, var(--bg-primary) 92%, var(--text-primary)))',
  foreground: 'var(--text-primary)',
  muted: 'var(--text-secondary)',
  border: 'var(--border-color)',
  primary: 'var(--color-primary)',
  userBackground: 'var(--theme-question-bubble, var(--bubble-user-bg, var(--bg-secondary)))',
  assistantBackground: 'transparent',
  danger: 'var(--color-danger, #f56c6c)',
  codeBackground: 'var(--code-bg, color-mix(in srgb, var(--bg-primary) 95%, var(--text-primary) 5%))',
  radius: '12px',
  fontSize: '14px',
  bubbleMaxWidth: '85%',
}

