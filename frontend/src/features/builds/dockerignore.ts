// the same rules the bird CLI applies, so a folder uploaded here builds like `bird deploy --build`
type Rule = { exception: boolean; parts: string[] }

export class DockerIgnore {
  private readonly rules: Rule[]

  constructor(text: string) {
    const rules = text
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line !== '' && !line.startsWith('#'))
      .map((line): Rule => {
        const exception = line.startsWith('!')
        const pattern = exception ? line.slice(1).trim() : line
        return { exception, parts: pattern.split('/').filter((part) => part !== '' && part !== '.') }
      })
      .filter((rule) => rule.parts.length > 0)
    // character classes and escapes are not supported, so such a file excludes nothing
    const unsupported = rules.some((rule) => rule.parts.some((part) => /[[\\]/.test(part)))
    this.rules = unsupported ? [] : rules
  }

  excludes(path: string): boolean {
    const parts = path.split('/')
    let excluded = false
    for (const rule of this.rules) {
      let matched = false
      for (let depth = 1; depth <= parts.length && !matched; depth += 1) {
        matched = globParts(rule.parts, parts.slice(0, depth))
      }
      if (matched) excluded = !rule.exception
    }
    return excluded
  }
}

function globParts(pattern: string[], path: string[]): boolean {
  const [first, ...rest] = pattern
  if (first === undefined) return path.length === 0
  if (first === '**') {
    for (let skip = 0; skip <= path.length; skip += 1) {
      if (globParts(rest, path.slice(skip))) return true
    }
    return false
  }
  const [head, ...tail] = path
  return head !== undefined && globSegment(first, head) && globParts(rest, tail)
}

function globSegment(pattern: string, text: string): boolean {
  if (pattern === '') return text === ''
  const [first, rest] = [pattern[0], pattern.slice(1)]
  if (first === '*') {
    for (let skip = 0; skip <= text.length; skip += 1) {
      if (globSegment(rest, text.slice(skip))) return true
    }
    return false
  }
  if (text === '') return false
  return (first === '?' || first === text[0]) && globSegment(rest, text.slice(1))
}
