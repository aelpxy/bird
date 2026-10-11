const KEY = /^[A-Za-z_][A-Za-z0-9_]{0,254}$/

export type DotenvResult = { ok: true; values: Record<string, string> } | { ok: false; error: string }

// KEY=value lines as a .env file has them: comments, `export`, and quoted values with \n escapes
export function parseDotenv(text: string): DotenvResult {
  const values: Record<string, string> = {}
  const lines = text.split(/\r?\n/)
  for (const [index, raw] of lines.entries()) {
    const line = raw.trim()
    if (line === '' || line.startsWith('#')) continue
    const assignment = line.startsWith('export ') ? line.slice(7).trimStart() : line
    const equals = assignment.indexOf('=')
    if (equals === -1) return { ok: false, error: `Line ${index + 1} has no "=": ${line}` }
    const key = assignment.slice(0, equals).trim()
    if (!KEY.test(key)) return { ok: false, error: `Line ${index + 1}: ${key || 'the name'} is not a valid variable name.` }
    values[key] = unquote(assignment.slice(equals + 1).trim())
  }
  return { ok: true, values }
}

function unquote(value: string): string {
  if (value.length >= 2 && value.startsWith('"') && value.endsWith('"')) {
    return value.slice(1, -1).replace(/\\(["\\n])/g, (_, char: string) => (char === 'n' ? '\n' : char))
  }
  if (value.length >= 2 && value.startsWith("'") && value.endsWith("'")) return value.slice(1, -1)
  // an unquoted value ends at a comment
  const comment = value.search(/\s#/)
  return comment === -1 ? value : value.slice(0, comment).trimEnd()
}
