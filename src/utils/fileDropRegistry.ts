export interface DropTarget {
  path: string
  enabled: () => boolean
  session: () => unknown
  receive: (paths: string[], isCurrent: () => boolean) => Promise<void>
}

export function createFileDropRegistry() {
  const targets = new Map<symbol, DropTarget>()
  const current = (path: string) => [...targets.entries()].reverse().find(([, t]) => t.path === path && t.enabled())
  return {
    register(target: DropTarget) {
      const id = Symbol()
      targets.set(id, target)
      return () => { targets.delete(id) }
    },
    accepts: (path: string) => !!current(path),
    async dispatch(path: string, paths: string[]) {
      const match = current(path)
      if (!match) return
      const [id, target] = match
      const session = target.session()
      await target.receive(paths, () => targets.get(id) === target && target.enabled() && target.session() === session)
    },
  }
}

export const fileDropRegistry = createFileDropRegistry()
