export type Fields = Record<string, string>

const MESSAGE_KEYS = ['msg', 'message', 'event']
const LOGFMT_PAIR = /([A-Za-z_][\w.-]*)=("(?:[^"\\]|\\.)*"|\S*)/g

// a JSON object or a key=value line becomes fields; anything else has none
export function parseFields(plain: string): Fields | undefined {
  const text = plain.trim()
  if (text.startsWith('{')) return fromJson(text)
  return fromLogfmt(text)
}

// what the line says, without the fields around it
export function messageOf(fields: Fields): string | undefined {
  for (const key of MESSAGE_KEYS) if (fields[key] !== undefined) return fields[key]
  return undefined
}

function fromJson(text: string): Fields | undefined {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    return undefined
  }
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const fields: Fields = {}
  flatten(value as Record<string, unknown>, '', fields)
  return fields
}

// nested objects become dotted keys, like http.status
function flatten(object: Record<string, unknown>, prefix: string, into: Fields) {
  for (const [key, value] of Object.entries(object)) {
    const name = prefix ? `${prefix}.${key}` : key
    if (typeof value === 'object' && value !== null && !Array.isArray(value)) flatten(value as Record<string, unknown>, name, into)
    else into[name] = typeof value === 'string' ? value : JSON.stringify(value)
  }
}

// at least two pairs and starting with one, so prose that mentions a=b stays prose
function fromLogfmt(text: string): Fields | undefined {
  const pairs = [...text.matchAll(LOGFMT_PAIR)]
  if (pairs.length < 2 || pairs[0].index !== 0) return undefined
  const fields: Fields = {}
  for (const [, key, raw] of pairs) fields[key] = raw.startsWith('"') ? unquote(raw) : raw
  return fields
}

function unquote(raw: string): string {
  try {
    const value: unknown = JSON.parse(raw)
    return typeof value === 'string' ? value : raw
  } catch {
    return raw.slice(1, -1)
  }
}
