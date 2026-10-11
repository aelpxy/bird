import { memo, type ReactNode } from 'react'
import { parseAnsi, type AnsiColor, type AnsiSegment } from '@/lib/ansi'
import type { LogLevel } from '../level'
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

export const MACHINE_COLORS = ['text-info', 'text-ansi-magenta', 'text-warn', 'text-ok', 'text-ansi-cyan']

const time = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit', fractionalSecondDigits: 3, hour12: false })
const full = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'medium' })

type LogLineProps = {
  line: ShownLine
  machineColor: string
  matcher: RegExp | null
  wrap: boolean
  showTime: boolean
}

export const LogLineRow = memo(function LogLineRow({ line, machineColor, matcher, wrap, showTime }: LogLineProps) {
  const level = line.level ? levelStyles[line.level] : undefined
  return (
    <div className={`group relative flex gap-3 pr-4 pl-3 hover:bg-raised ${level?.row ?? ''}`}>
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
        {parseAnsi(line.text).map((segment, index) => (
          <Segment key={index} segment={segment} matcher={matcher} />
        ))}
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
