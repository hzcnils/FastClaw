/** Browser speech is window-global: only the current owned request may cancel it. */
export interface SpeechPort {
  create(text: string): SpeechSynthesisUtterance
  speak(utterance: SpeechSynthesisUtterance): void
  cancel(): void
}

export function browserSpeechPort(): SpeechPort | null {
  if (typeof window === 'undefined' || !window.speechSynthesis || typeof SpeechSynthesisUtterance === 'undefined') return null
  const synthesis = window.speechSynthesis
  return { create: text => new SpeechSynthesisUtterance(text), speak: u => synthesis.speak(u), cancel: () => synthesis.cancel() }
}

export function createSpeechPlayback() {
  let active: { stop(): void } | null = null
  return {
    play(port: SpeechPort, text: string, finished: () => void) {
      const utterance = port.create(text)
      utterance.lang = 'zh-CN'
      active?.stop()
      function finish(cancel: boolean) {
        if (active !== request) return
        active = null
        utterance.onend = null; utterance.onerror = null
        try { if (cancel) port.cancel() }
        catch { /* Native engine cancellation failure must not retain UI ownership. */ }
        finally { finished() }
      }
      const request = { stop: () => finish(true) }
      active = request
      utterance.onend = () => finish(false)
      utterance.onerror = () => finish(false)
      try { port.speak(utterance) }
      catch (error) { request.stop(); throw error }
      return request
    },
  }
}

export const chatSpeechPlayback = createSpeechPlayback()
