import type { Fields } from './fields'
import type { LogLevel } from './level'

export type Queryable = { plain: string; level?: LogLevel; machine: string; stream: string; fields?: Fields }

export type Query = { test: (line: Queryable) => boolean; highlight: RegExp | null; error?: string }

type Term = { negate: boolean; key?: string; value: string }

const TERM = /(-)?(?:([A-Za-z_@][\w.@-]*):)?("(?:[^"\\]|\\.)*"|\S+)/g
const everything: Query = { test: () => true, highlight: null }

// words must all appear; key:value matches a field (key:* that it exists), -term excludes, "quotes" keep spaces
export function parseQuery(input: string, regex: boolean): Query {
  const text = input.trim()
  if (!text) return everything
  if (regex) {
    try {
      const pattern = new RegExp(text, 'i')
      return { test: (line) => pattern.test(line.plain), highlight: new RegExp(text, 'gi') }
    } catch (cause) {
      return { test: () => false, highlight: null, error: cause instanceof Error ? cause.message : 'Invalid pattern' }
    }
  }
  const terms: Term[] = [...text.matchAll(TERM)].map(([, minus, key, raw]) => ({ negate: minus === '-', key, value: unquote(raw).toLowerCase() }))
  const words = terms.filter((term) => !term.key && !term.negate && term.value).map((term) => escape(term.value))
  return {
    test: (line) => terms.every((term) => matches(line, term) !== term.negate),
    highlight: words.length > 0 ? new RegExp(words.join('|'), 'gi') : null,
  }
}

function matches(line: Queryable, { key, value }: Term): boolean {
  if (!key) return line.plain.toLowerCase().includes(value)
  switch (key.toLowerCase()) {
    case 'level':
      return line.level !== undefined && line.level.startsWith(value)
    case 'machine':
      return line.machine.toLowerCase().startsWith(value)
    case 'stream':
      return line.stream.startsWith(value)
  }
  const field = line.fields?.[key]
  if (field === undefined) return false
  return value === '*' || field.toLowerCase().includes(value)
}

function unquote(raw: string): string {
  return raw.length >= 2 && raw.startsWith('"') && raw.endsWith('"') ? raw.slice(1, -1).replace(/\\(.)/g, '$1') : raw
}

function escape(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

// a term for a field value, quoted when it has spaces, so clicking a field can add it to the query
export function fieldTerm(key: string, value: string, negate = false): string {
  const quoted = /[\s"]/.test(value) || value === '' ? JSON.stringify(value) : value
  return `${negate ? '-' : ''}${key}:${quoted}`
}
