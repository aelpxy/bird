import { stripAnsi } from '@/lib/ansi'
import { detectLevel, LEVEL_RANK, type LogLevel } from './level'
import type { LogLine } from './use-log-stream'

export type StreamFilter = 'all' | 'stdout' | 'stderr'

export type LogFilters = {
  query: string
  regex: boolean
  stream: StreamFilter
  // lines below this level are hidden; lines without a level always show
  minLevel: LogLevel | 'all'
  hiddenMachines: ReadonlySet<string>
}

export type ShownLine = LogLine & { plain: string; level: LogLevel | undefined }

export type FilterResult = { lines: ShownLine[]; matcher: RegExp | null; error?: string }

export function prepare(line: LogLine): ShownLine {
  const plain = stripAnsi(line.text)
  return { ...line, plain, level: detectLevel(plain) }
}

export function matcherFor(query: string, regex: boolean): { matcher: RegExp | null; error?: string } {
  if (!query) return { matcher: null }
  if (!regex) return { matcher: new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi') }
  try {
    return { matcher: new RegExp(query, 'gi') }
  } catch (cause) {
    return { matcher: null, error: cause instanceof Error ? cause.message : 'Invalid pattern' }
  }
}

export function filterLines(lines: ShownLine[], filters: LogFilters): FilterResult {
  const { matcher, error } = matcherFor(filters.query, filters.regex)
  if (error) return { lines: [], matcher: null, error }
  const min = filters.minLevel === 'all' ? -1 : LEVEL_RANK[filters.minLevel]
  const shown = lines.filter((line) => {
    if (filters.stream !== 'all' && line.stream !== filters.stream) return false
    if (filters.hiddenMachines.has(line.machine)) return false
    if (min >= 0 && line.level && LEVEL_RANK[line.level] < min) return false
    if (matcher) {
      matcher.lastIndex = 0
      return matcher.test(line.plain)
    }
    return true
  })
  return { lines: shown, matcher }
}
