import type { LogLevel } from './level'

export type Bucket = { start: number; end: number; error: number; warn: number; other: number }

// line counts over time by severity, in equal slices between the first and the last line
export function buckets(lines: { at?: number | null; level?: LogLevel }[], count: number): Bucket[] {
  // a loop, since spreading tens of thousands of times into Math.min overflows the stack
  let first = Infinity
  let last = -Infinity
  for (const line of lines) {
    if (line.at == null) continue
    if (line.at < first) first = line.at
    if (line.at > last) last = line.at
  }
  if (first === Infinity) return []
  const width = Math.max(1, (last - first + 1) / count)
  const result: Bucket[] = Array.from({ length: count }, (_, index) => ({ start: first + index * width, end: first + (index + 1) * width, error: 0, warn: 0, other: 0 }))
  for (const line of lines) {
    if (line.at == null) continue
    const bucket = result[Math.min(count - 1, Math.floor((line.at - first) / width))]
    if (line.level === 'error') bucket.error += 1
    else if (line.level === 'warn') bucket.warn += 1
    else bucket.other += 1
  }
  return result
}
