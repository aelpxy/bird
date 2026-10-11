import { describe, expect, it } from 'vitest'
import { buckets } from './histogram'

describe('log histogram', () => {
  it('counts lines per slice by severity and skips untimed ones', () => {
    const result = buckets(
      [
        { at: 0, level: 'error' },
        { at: 4, level: 'warn' },
        { at: 5 },
        { at: 9, level: 'info' },
        { at: null, level: 'error' },
      ],
      2,
    )
    expect(result.map(({ error, warn, other }) => [error, warn, other])).toEqual([
      [1, 1, 0],
      [0, 0, 2],
    ])
    expect(result[0].start).toBe(0)
    expect(result[1].end).toBe(10)
  })

  it('has nothing to draw without times', () => {
    expect(buckets([{ at: null }], 10)).toEqual([])
  })
})
