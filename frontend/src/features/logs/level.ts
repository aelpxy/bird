export type LogLevel = 'error' | 'warn' | 'info' | 'debug'

const WORDS: [RegExp, LogLevel][] = [
  [/\b(fatal|panic|critical|crit|emerg|alert|error|err)\b/i, 'error'],
  [/\b(warning|warn)\b/i, 'warn'],
  [/\b(info|notice)\b/i, 'info'],
  [/\b(debug|trace)\b/i, 'debug'],
]

// how severe a line is, from the common ways apps write it: JSON fields, key=value, or a leading word
export function detectLevel(text: string): LogLevel | undefined {
  const field = /["']?(?:level|severity|lvl)["']?\s*[:=]\s*["']?([a-z]+)/i.exec(text)?.[1]
  if (field) return fromWord(field)
  // only look near the start, where loggers put the level, so a message mentioning "error" stays as it is
  const head = text.slice(0, 48)
  for (const [pattern, level] of WORDS) {
    if (pattern.test(head) && /^\W*(\S+\s+){0,4}\W*(fatal|panic|critical|crit|emerg|alert|error|err|warning|warn|info|notice|debug|trace)\b/i.test(head)) return level
  }
  return undefined
}

function fromWord(word: string): LogLevel | undefined {
  for (const [pattern, level] of WORDS) if (pattern.test(word)) return level
  return undefined
}

export const LEVEL_RANK: Record<LogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3 }
