import { describe, expect, it } from 'vitest'
import { messageOf, parseFields } from './fields'

describe('log fields', () => {
  it('reads JSON lines, flattening nested objects', () => {
    const fields = parseFields('{"level":"error","msg":"boom","http":{"status":500},"ok":false}')
    expect(fields).toEqual({ level: 'error', msg: 'boom', 'http.status': '500', ok: 'false' })
    expect(messageOf(fields ?? {})).toBe('boom')
  })

  it('reads key=value lines with quoted values', () => {
    expect(parseFields('time=12:00 level=info msg="user signed in" user=ana')).toEqual({ time: '12:00', level: 'info', msg: 'user signed in', user: 'ana' })
  })

  it('finds pairs after a leading time and level', () => {
    expect(parseFields('2026-10-11T22:06:52Z INFO request: done latency=0 ms status=200 path=/health')).toEqual({ latency: '0', status: '200', path: '/health' })
  })

  it('leaves prose and broken JSON alone', () => {
    expect(parseFields('GET / 200 took a=b')).toBeUndefined()
    expect(parseFields('{not json')).toBeUndefined()
    expect(parseFields('[1,2]')).toBeUndefined()
  })
})
