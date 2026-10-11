import { describe, expect, it } from 'vitest'
import { fieldTerm, parseQuery, type Queryable } from './query'

const line = (plain: string, extra: Partial<Queryable> = {}): Queryable => ({ plain, machine: '01a2b3c4', stream: 'stdout', ...extra })

describe('log query', () => {
  const api = line('GET /api 500', { level: 'error', fields: { status: '500', path: '/api/users', user: 'ana lee' } })
  const home = line('GET / 200', { level: 'info', fields: { status: '200', path: '/' } })

  it('needs every word, in any order and case', () => {
    expect(parseQuery('500 get', false).test(api)).toBe(true)
    expect(parseQuery('500 get', false).test(home)).toBe(false)
  })

  it('matches fields, their existence and the built-in keys', () => {
    expect(parseQuery('status:500', false).test(api)).toBe(true)
    expect(parseQuery('path:/api', false).test(home)).toBe(false)
    expect(parseQuery('user:*', false).test(home)).toBe(false)
    expect(parseQuery('user:"ana lee"', false).test(api)).toBe(true)
    expect(parseQuery('level:err machine:01a stream:stdout', false).test(api)).toBe(true)
  })

  it('excludes with a minus', () => {
    expect(parseQuery('-status:200', false).test(home)).toBe(false)
    expect(parseQuery('GET -500', false).test(home)).toBe(true)
  })

  it('highlights only the plain words', () => {
    expect(parseQuery('status:500 get', false).highlight?.source).toBe('get')
  })

  it('treats the whole query as a pattern in regex mode and reports a bad one', () => {
    expect(parseQuery('5\\d\\d$', true).test(api)).toBe(true)
    expect(parseQuery('(', true).error).toBeTruthy()
  })

  it('writes field terms that read back', () => {
    const term = fieldTerm('user', 'ana lee')
    expect(term).toBe('user:"ana lee"')
    expect(parseQuery(term, false).test(api)).toBe(true)
    expect(fieldTerm('status', '500', true)).toBe('-status:500')
  })
})
