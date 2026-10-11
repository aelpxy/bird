import { describe, expect, it } from 'vitest'
import { parseAnsi, stripAnsi } from './ansi'

describe('parseAnsi', () => {
  it('splits text where the style changes', () => {
    expect(parseAnsi('plain \u001b[1;31mERROR\u001b[0m done')).toEqual([
      { text: 'plain ' },
      { text: 'ERROR', bold: true, color: 'red' },
      { text: ' done' },
    ])
  })

  it('reads bright colors and resets parts of a style', () => {
    expect(parseAnsi('\u001b[92mok\u001b[39m\u001b[2m dim\u001b[22m')).toEqual([
      { text: 'ok', color: 'green' },
      { text: ' dim', dim: true },
    ])
  })

  it('drops sequences that do not style text', () => {
    expect(parseAnsi('\u001b[2K\u001b[1Gprogress 50%')).toEqual([{ text: 'progress 50%' }])
    expect(stripAnsi('\u001b]0;title\u0007\u001b[33mwarn\u001b[m')).toBe('warn')
  })

  it('maps the first 16 of 256 colors and ignores the rest', () => {
    expect(parseAnsi('\u001b[38;5;4mblue\u001b[38;5;200mpink')).toEqual([
      { text: 'blue', color: 'blue' },
      { text: 'pink' },
    ])
  })
})
