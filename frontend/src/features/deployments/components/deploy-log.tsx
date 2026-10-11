import { useLayoutEffect, useRef } from 'react'
import { parseAnsi } from '@/lib/ansi'
import type { DeployLogLine } from '../api'

const time = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false })

function label(line: DeployLogLine): string {
  switch (line.kind) {
    case 'bird':
      return 'bird'
    case 'build':
      return 'build'
    case 'release':
      return 'release'
    case 'app':
      return line.machine ? line.machine.slice(0, 8) : 'app'
  }
}

// a deploy's story: birdd's own steps stand out, everything the app or build printed is quieter
export function DeployLog({ lines, live = false, tall = false, empty }: { lines: DeployLogLine[]; live?: boolean; tall?: boolean; empty?: string }) {
  const box = useRef<HTMLDivElement>(null)
  const pinned = useRef(true)
  useLayoutEffect(() => {
    if (box.current && pinned.current) box.current.scrollTop = box.current.scrollHeight
  }, [lines])
  if (lines.length === 0) {
    return <p className="border border-line bg-sunken px-3 py-2.5 font-mono text-xs text-fg-faint">{empty ?? (live ? 'Waiting for birdd…' : 'This deploy kept no log.')}</p>
  }
  return (
    <div
      ref={box}
      role="log"
      aria-live={live ? 'polite' : undefined}
      aria-label="Deploy log"
      tabIndex={0}
      onScroll={(event) => {
        const target = event.currentTarget
        // following the newest line stops once someone scrolls up to read
        pinned.current = target.scrollHeight - target.scrollTop - target.clientHeight < 24
      }}
      className={`${tall ? 'h-[min(36rem,60vh)]' : 'max-h-96'} overflow-auto border border-line bg-sunken py-2 font-mono text-xs leading-5`}
    >
      {lines.map((line) => (
        <div key={line.seq} className="flex gap-3 px-3 hover:bg-raised">
          <time className="tabular shrink-0 text-fg-faint select-none" dateTime={new Date(line.at).toISOString()}>
            {time.format(line.at)}
          </time>
          <span className={`w-16 shrink-0 truncate select-none ${line.kind === 'bird' ? 'text-info' : 'text-fg-faint'}`}>{label(line)}</span>
          <span className={`min-w-0 break-all whitespace-pre-wrap ${line.kind === 'bird' ? 'text-fg' : 'text-fg-muted'}`}>
            {parseAnsi(line.text)
              .map((segment) => segment.text)
              .join('')}
          </span>
        </div>
      ))}
    </div>
  )
}
