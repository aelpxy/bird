import { describe, expect, it } from 'vitest'
import { mapColumns } from './graph'

const names = (columns: { name: string }[][]) => columns.map((column) => column.map((service) => service.name))

describe('service map columns', () => {
  it('puts readers before what they read', () => {
    const services = [
      { name: 'postgres' },
      { name: 'web', references: ['api'] },
      { name: 'api', references: ['postgres', 'valkey'] },
      { name: 'valkey' },
      { name: 'worker', references: ['postgres'] },
    ]
    expect(names(mapColumns(services))).toEqual([['web'], ['api', 'worker'], ['postgres', 'valkey']])
  })

  it('ignores references to missing services and survives a cycle', () => {
    expect(names(mapColumns([{ name: 'a', references: ['gone'] }]))).toEqual([['a']])
    expect(names(mapColumns([{ name: 'a', references: ['b'] }, { name: 'b', references: ['a'] }])).flat().sort()).toEqual(['a', 'b'])
  })
})
