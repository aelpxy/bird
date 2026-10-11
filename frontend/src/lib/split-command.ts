export type SplitResult = { ok: true; args: string[] } | { ok: false; error: string }

// splits like a shell: spaces separate, quotes group, a backslash escapes the next character;
// nothing is expanded, since birdd runs the arguments as given
export function splitCommand(input: string): SplitResult {
  const args: string[] = []
  let current = ''
  let started = false
  let quote: '"' | "'" | null = null

  for (let index = 0; index < input.length; index += 1) {
    const char = input[index]
    if (quote === "'") {
      if (char === "'") quote = null
      else current += char
      continue
    }
    if (char === '\\') {
      const next = input[index + 1]
      if (next === undefined) return { ok: false, error: 'The command ends with a lone backslash.' }
      current += next
      started = true
      index += 1
      continue
    }
    if (quote === '"') {
      if (char === '"') quote = null
      else current += char
      continue
    }
    if (char === '"' || char === "'") {
      quote = char
      started = true
    } else if (/\s/.test(char)) {
      if (started) args.push(current)
      current = ''
      started = false
    } else {
      current += char
      started = true
    }
  }
  if (quote) return { ok: false, error: `A ${quote === '"' ? 'double' : 'single'} quote is never closed.` }
  if (started) args.push(current)
  if (args.length === 0) return { ok: false, error: 'Enter a command to run.' }
  return { ok: true, args }
}
