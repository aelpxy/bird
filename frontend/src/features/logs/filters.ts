import { stripAnsi } from '@/lib/ansi'
import { parseFields, type Fields } from './fields'
import { detectLevel, LEVEL_RANK, type LogLevel } from './level'
import { parseQuery } from './query'
import type { LogLine } from './use-log-stream'

export type StreamFilter = 'all' | 'stdout' | 'stderr'

export type LogFilters = {
  query: string
  regex: boolean
  stream: StreamFilter
  // lines below this level are hidden; lines without a level always show
  minLevel: LogLevel | 'all'
  hiddenMachines: ReadonlySet<string>
  // a stretch of time picked on the chart, in unix milliseconds
  window?: { from: number; to: number }
}

export type ShownLine = LogLine & { plain: string; level: LogLevel | undefined; fields?: Fields }

export type FilterResult = { lines: ShownLine[]; matcher: RegExp | null; error?: string }

export function prepare(line: LogLine): ShownLine {
  const plain = stripAnsi(line.text)
  return { ...line, plain, level: detectLevel(plain), fields: parseFields(plain) }
}

export function filterLines(lines: ShownLine[], filters: LogFilters): FilterResult {
  const query = parseQuery(filters.query, filters.regex)
  if (query.error) return { lines: [], matcher: null, error: query.error }
  const min = filters.minLevel === 'all' ? -1 : LEVEL_RANK[filters.minLevel]
  const { window } = filters
  const shown = lines.filter((line) => {
    if (filters.stream !== 'all' && line.stream !== filters.stream) return false
    if (filters.hiddenMachines.has(line.machine)) return false
    if (min >= 0 && line.level && LEVEL_RANK[line.level] < min) return false
    if (window && (line.at == null || line.at < window.from || line.at > window.to)) return false
    return query.test(line)
  })
  return { lines: shown, matcher: query.highlight }
}
