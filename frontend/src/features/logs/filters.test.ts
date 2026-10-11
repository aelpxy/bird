import { describe, expect, it } from 'vitest'
import { filterLines, prepare, type LogFilters } from './filters'

const line = (id: number, text: string, extra: { stream?: 'stdout' | 'stderr'; machine?: string; at?: number } = {}) =>
  prepare({ id, text, stream: extra.stream ?? 'stdout', machine: extra.machine ?? 'm1', at: extra.at ?? id * 1000 })

const lines = [
  line(1, 'INFO started'),
  line(2, '\u001b[31mERROR\u001b[0m failed to connect', { stream: 'stderr' }),
  line(3, 'WARN slow query', { machine: 'm2' }),
  line(4, 'plain text'),
]

const none: LogFilters = { query: '', regex: false, stream: 'all', minLevel: 'all', hiddenMachines: new Set() }
const ids = (filters: Partial<LogFilters>) => filterLines(lines, { ...none, ...filters }).lines.map((shown) => shown.id)

describe('filterLines', () => {
  it('searches the text without color codes, ignoring case', () => {
    expect(ids({ query: 'error f' })).toEqual([2])
  })

  it('treats the query literally unless regex is on', () => {
    expect(ids({ query: 's.ow' })).toEqual([])
    expect(ids({ query: 's.ow', regex: true })).toEqual([3])
    expect(filterLines(lines, { ...none, query: '(', regex: true }).error).toBeTruthy()
  })

  it('filters by stream, machine and level, keeping lines without a level', () => {
    expect(ids({ stream: 'stderr' })).toEqual([2])
    expect(ids({ hiddenMachines: new Set(['m1']) })).toEqual([3])
    expect(ids({ minLevel: 'warn' })).toEqual([2, 3, 4])
  })

  it('keeps only lines inside a picked stretch of time', () => {
    expect(ids({ window: { from: 2000, to: 3000 } })).toEqual([2, 3])
  })
})
