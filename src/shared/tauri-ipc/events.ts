import type { Event, UnlistenFn } from '@tauri-apps/api/event'

export type EventListenerAdapter = <T>(
  eventName: string,
  handler: (event: Event<T>) => void,
) => Promise<UnlistenFn>

const tauriListen: EventListenerAdapter = async <T>(eventName: string, handler: (event: Event<T>) => void) => {
  const { listen } = await import('@tauri-apps/api/event')
  return listen<T>(eventName, handler)
}
let listenerAdapter: EventListenerAdapter = tauriListen

export function setEventListenerAdapter(adapter: EventListenerAdapter): void {
  listenerAdapter = adapter
}

export function resetEventListenerAdapter(): void {
  listenerAdapter = tauriListen
}

export function listenEvent<T>(
  eventName: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  return listenerAdapter<T>(eventName, event => handler(event.payload))
}
