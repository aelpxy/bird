import { useEffect, useEffectEvent, useLayoutEffect, useRef, useState } from 'react'
import type { Scope } from '@/lib/api/paths'
import { errorMessage } from '@/lib/toast'
import { runCommand } from '@/features/terminal/api'
import type { CommandOptions, Target } from '@/features/terminal/connect'
import type { SessionEnd } from './terminal-session'

type Chunk = { id: number; stderr: boolean; text: string }

type OutputSessionProps = {
  scope: Scope
  service: string
  target: Target
  command: string[]
  options: CommandOptions
  onEnd: (end: SessionEnd) => void
}

// output as the command writes it; leaving the page stops the command
export function OutputSession({ scope, service, target, command, options, onEnd }: OutputSessionProps) {
  const [chunks, setChunks] = useState<Chunk[]>([])
  const box = useRef<HTMLPreElement>(null)
  const reportEnd = useEffectEvent(onEnd)
  const { project, environment } = scope
  const commandKey = JSON.stringify(command)
  const { machine, skipEntrypoint } = options

  useEffect(() => {
    const controller = new AbortController()
    let next = 0
    let ended = false
    runCommand(
      { project, environment },
      service,
      target,
      JSON.parse(commandKey) as string[],
      { machine, skipEntrypoint },
      (event) => {
        if (event.type === 'output') {
          const chunk = { id: next++, stderr: event.stream === 'stderr', text: event.text }
          setChunks((current) => [...current, chunk])
        } else {
          ended = true
          reportEnd(event.type === 'exited' ? { kind: 'exit', code: event.code } : { kind: 'error', message: event.error })
        }
      },
      controller.signal,
    ).then(
      () => {
        if (!ended && !controller.signal.aborted) reportEnd({ kind: 'error', message: 'The command ended without an exit code.' })
      },
      (error: unknown) => reportEnd({ kind: 'error', message: errorMessage(error) }),
    )
    return () => controller.abort()
  }, [project, environment, service, target, commandKey, machine, skipEntrypoint])

  useLayoutEffect(() => {
    if (box.current) box.current.scrollTop = box.current.scrollHeight
  }, [chunks])

  return (
    <pre
      ref={box}
      aria-label="Command output"
      tabIndex={0}
      className="h-full overflow-auto p-2 font-mono text-xs leading-5 whitespace-pre-wrap text-fg focus-visible:outline-none"
    >
      {chunks.length === 0 ? (
        <span className="text-fg-faint">Waiting for output…</span>
      ) : (
        chunks.map((chunk) => (
          <span key={chunk.id} className={chunk.stderr ? 'text-bad' : undefined}>
            {chunk.text}
          </span>
        ))
      )}
    </pre>
  )
}
