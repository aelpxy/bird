// the color and style codes apps write to terminals, read so logs show them instead of escape junk

export type AnsiColor = 'black' | 'red' | 'green' | 'yellow' | 'blue' | 'magenta' | 'cyan' | 'white'

export type AnsiStyle = { color?: AnsiColor; bold?: boolean; dim?: boolean; italic?: boolean; underline?: boolean }

export type AnsiSegment = AnsiStyle & { text: string }

const COLORS: AnsiColor[] = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white']

// any escape sequence: CSI (ESC [ ... final byte) or OSC (ESC ] ... BEL or ST); matching control characters is the point
// oxlint-disable-next-line no-control-regex
const ESCAPE = /\u001b\[([0-9;?]*)([@-~])|\u001b\][^\u0007\u001b]*(?:\u0007|\u001b\\)/g

export function parseAnsi(text: string): AnsiSegment[] {
  const segments: AnsiSegment[] = []
  let style: AnsiStyle = {}
  let last = 0
  for (const match of text.matchAll(ESCAPE)) {
    if (match.index > last) segments.push({ ...style, text: text.slice(last, match.index) })
    last = match.index + match[0].length
    // only "m" sequences style text; cursor moves and the like are dropped
    if (match[2] === 'm') style = applySgr(style, match[1] ?? '')
  }
  if (last < text.length) segments.push({ ...style, text: text.slice(last) })
  return segments
}

function applySgr(current: AnsiStyle, params: string): AnsiStyle {
  const codes = params === '' ? [0] : params.split(';').map(Number)
  let style = { ...current }
  for (let index = 0; index < codes.length; index += 1) {
    const code = codes[index]
    if (code === 0) style = {}
    else if (code === 1) style.bold = true
    else if (code === 2) style.dim = true
    else if (code === 3) style.italic = true
    else if (code === 4) style.underline = true
    else if (code === 22) style = { ...style, bold: false, dim: false }
    else if (code === 23) style.italic = false
    else if (code === 24) style.underline = false
    else if (code === 39) style.color = undefined
    else if (code >= 30 && code <= 37) style.color = COLORS[code - 30]
    else if (code >= 90 && code <= 97) style.color = COLORS[code - 90]
    // 256-color and true color pick the nearest of the basic eight, or none
    else if (code === 38) {
      if (codes[index + 1] === 5) {
        const n = codes[index + 2] ?? -1
        style.color = n >= 0 && n < 16 ? COLORS[n % 8] : undefined
        index += 2
      } else if (codes[index + 1] === 2) {
        index += 4
        style.color = undefined
      }
    }
  }
  return style
}

export function stripAnsi(text: string): string {
  return text.replace(ESCAPE, '')
}
