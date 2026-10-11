import { describe, expect, it } from 'vitest'
import { filter, type Item } from './filter'

const item = (label: string, detail: string): Item => ({ id: label + detail, label, detail, go: () => undefined })
const items = [item('web', 'shop / production'), item('web', 'shop / staging'), item('postgres', 'shop / production'), item('Orgs', 'page')]

describe('command palette filter', () => {
  it('shows everything for an empty query', () => {
    expect(filter(items, '  ')).toHaveLength(4)
  })

  it('matches every word in any order across name and place', () => {
    expect(filter(items, 'staging web').map((found) => found.detail)).toEqual(['shop / staging'])
    expect(filter(items, 'ORGS')).toEqual([items[3]])
    expect(filter(items, 'web redis')).toEqual([])
  })
})
