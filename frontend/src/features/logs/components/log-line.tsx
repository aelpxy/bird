import { memo, type ReactNode } from 'react'
import { parseAnsi, type AnsiColor, type AnsiSegment } from '@/lib/ansi'
import type { LogLevel } from '../level'
import { messageOf } from '../fields'
import type { ShownLine } from '../filters'

const ansiColors: Record<AnsiColor, string> = {
  black: 'text-fg-faint',
  red: 'text-bad',
  green: 'text-ok',
  yellow: 'text-warn',
  blue: 'text-info',
  magenta: 'text-ansi-magenta',
  cyan: 'text-ansi-cyan',
  white: 'text-fg',
}

const levelStyles: Record<LogLevel, { mark: string; row: string }> = {
  error: { mark: 'bg-bad', row: 'bg-bad/6' },
  warn: { mark: 'bg-warn', row: 'bg-warn/5' },
  info: { mark: 'bg-transparent', row: '' },
  debug: { mark: 'bg-transparent', row: 'opacity-70' },
}

const time = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit', fractionalSecondDigits: 3, hour12: false })
const full = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'medium' })

// already shown by the time and level columns, so a structured line need not repeat them
const QUIET_KEYS = new Set(['msg', 'message', 'event', 'level', 'lvl', 'severity', 'time', 'ts', 'timestamp', '@timestamp'])

type LogLineProps = {
  line: ShownLine
  machineColor: string
  matcher: RegExp | null
  wrap: boolean
  showTime: boolean
  expanded: boolean
  onToggle: (id: number) => void
}

export const LogLineRow = memo(function LogLineRow({ line, machineColor, matcher, wrap, showTime, expanded, onToggle }: LogLineProps) {
  const level = line.level ? levelStyles[line.level] : undefined
  const message = line.fields ? messageOf(line.fields) : undefined
  return (
    <div
      role="button"
      tabIndex={-1}
      aria-expanded={expanded}
      onClick={() => {
        if (!window.getSelection()?.toString()) onToggle(line.id)
      }}
      className={`group relative flex cursor-default gap-3 pr-4 pl-3 hover:bg-raised ${expanded ? 'bg-raised' : (level?.row ?? '')}`}
    >
      <span aria-hidden className={`absolute inset-y-0 left-0 w-0.5 ${level?.mark ?? 'bg-transparent'}`} />
      {showTime && (
        <time
          className="tabular shrink-0 text-fg-faint select-none"
          dateTime={line.at ? new Date(line.at).toISOString() : undefined}
          title={line.at ? full.format(line.at) : 'No time recorded'}
        >
          {line.at ? time.format(line.at) : '··:··:··.···'}
        </time>
      )}
      <span className={`w-12 shrink-0 truncate select-none ${machineColor}`} title={line.machine}>
        {line.machine.slice(0, 6)}
      </span>
      {line.stream === 'stderr' && (
        <span className="shrink-0 text-bad/80 select-none" title="Written to stderr">
          err
        </span>
      )}
      <span className={`min-w-0 ${wrap ? 'break-all whitespace-pre-wrap' : 'whitespace-pre'}`}>
        {line.fields && message !== undefined ? (
          <>
            {highlight(message, matcher)}
            {Object.entries(line.fields)
              .filter(([key]) => !QUIET_KEYS.has(key))
              .map(([key, value]) => (
                <span key={key} className="text-fg-faint">
                  {' '}
                  {key}=<span className="text-fg-muted">{highlight(value, matcher)}</span>
                </span>
              ))}
          </>
        ) : (
          parseAnsi(line.text).map((segment, index) => <Segment key={index} segment={segment} matcher={matcher} />)
        )}
      </span>
    </div>
  )
})

function Segment({ segment, matcher }: { segment: AnsiSegment; matcher: RegExp | null }) {
  const className = [
    segment.color ? ansiColors[segment.color] : '',
    segment.bold ? 'font-semibold' : '',
    segment.dim ? 'opacity-60' : '',
    segment.italic ? 'italic' : '',
    segment.underline ? 'underline' : '',
  ]
    .filter(Boolean)
    .join(' ')
  return <span className={className || undefined}>{highlight(segment.text, matcher)}</span>
}

function highlight(text: string, matcher: RegExp | null): ReactNode {
  if (!matcher) return text
  const parts: ReactNode[] = []
  let last = 0
  for (const match of text.matchAll(new RegExp(matcher.source, 'gi'))) {
    if (match[0] === '') break
    parts.push(text.slice(last, match.index), (
      <mark key={match.index} className="bg-warn text-canvas">
        {match[0]}
      </mark>
    ))
    last = match.index + match[0].length
  }
  parts.push(text.slice(last))
  return parts
}
