import { useLayoutEffect, useRef } from 'react'
import { Spinner } from '@/components/ui/spinner'
import type { SourcePhase } from '@/features/builds/deploy-from-source'

const labels: Record<SourcePhase, string> = {
  packing: 'Packing the folder…',
  building: 'Building the image…',
  deploying: 'Deploying the new image…',
}

export type Progress = { phase: SourcePhase; detail?: string; lines: string[]; failed?: boolean }

export function BuildProgress({ progress }: { progress: Progress }) {
  const log = useRef<HTMLPreElement>(null)
  useLayoutEffect(() => {
    if (log.current) log.current.scrollTop = log.current.scrollHeight
  }, [progress.lines])
  return (
    <div className="flex flex-col gap-2">
      {progress.failed ? (
        <p className="text-sm text-fg-muted">Output before it stopped:</p>
      ) : (
        <p role="status" className="flex items-center gap-2 text-sm text-fg-muted">
          <Spinner />
          {labels[progress.phase]}
          {progress.detail && <span className="truncate text-xs text-fg-faint">{progress.detail}</span>}
        </p>
      )}
      {progress.lines.length > 0 && (
        <pre
          ref={log}
          aria-label="Build output"
          tabIndex={0}
          className="max-h-56 overflow-auto border border-line bg-sunken p-2.5 font-mono text-xs leading-5 whitespace-pre-wrap text-fg-muted"
        >
          {progress.lines.join('\n')}
        </pre>
      )}
    </div>
  )
}
