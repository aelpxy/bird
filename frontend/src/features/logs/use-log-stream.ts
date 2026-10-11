import { useEffect, useRef, useState } from 'react'
import type { Scope } from '@/lib/api/paths'
import { followLogs, type LogEntry } from './api'

const TAIL_LINES = 300
// enough to scroll back through, few enough that rendering stays quick
export const MAX_LINES = 2_000
const FLUSH_MS = 100
const RETRY_MS = [1_000, 2_000, 5_000, 10_000]

export type StreamState = 'connecting' | 'live' | 'reconnecting'
export type LogLine = LogEntry & { id: number }

// follows a service's logs, batching new lines so a chatty app does not re-render per line
export function useLogStream({ project, environment }: Scope, service: string) {
  const [lines, setLines] = useState<LogLine[]>([])
  const [state, setState] = useState<StreamState>('connecting')
  const pending = useRef<LogLine[]>([])
  const nextId = useRef(0)

  useEffect(() => {
    const controller = new AbortController()
    let attempt = 0
    let first = true
    const flush = window.setInterval(() => {
      if (pending.current.length === 0) return
      const batch = pending.current
      pending.current = []
      setLines((current) => [...current, ...batch].slice(-MAX_LINES))
    }, FLUSH_MS)

    async function run() {
      while (!controller.signal.aborted) {
        // after a reconnect only new lines are wanted, the recent ones are already shown
        const tail = first ? TAIL_LINES : 0
        try {
          setState(first ? 'connecting' : 'reconnecting')
          let opened = false
          await followLogs(
            { project, environment },
            service,
            tail,
            (entry) => {
              if (!opened) {
                opened = true
                attempt = 0
                setState('live')
              }
              pending.current.push({ ...entry, id: nextId.current++ })
            },
            controller.signal,
          )
        } catch {
          // shown as reconnecting; the loop tries again below
        }
        first = false
        if (controller.signal.aborted) return
        setState('reconnecting')
        const delay = RETRY_MS[Math.min(attempt, RETRY_MS.length - 1)]
        attempt += 1
        await new Promise((resolve) => window.setTimeout(resolve, delay))
      }
    }

    void run()
    return () => {
      controller.abort()
      window.clearInterval(flush)
    }
  }, [project, environment, service])

  return { lines, state, clear: () => setLines([]) }
}
