import { describe, expect, it } from 'vitest'
import { detectLevel } from './level'

describe('detectLevel', () => {
  it('reads structured fields', () => {
    expect(detectLevel('{"level":"error","msg":"boom"}')).toBe('error')
    expect(detectLevel('time=2026 level=warn msg="slow query"')).toBe('warn')
    expect(detectLevel('{"severity":"INFO","message":"up"}')).toBe('info')
  })

  it('reads a leading word, the way most loggers print it', () => {
    expect(detectLevel('ERROR: connection refused')).toBe('error')
    expect(detectLevel('[WARN] disk almost full')).toBe('warn')
    expect(detectLevel('2026/10/11 03:16:40 [notice] 2#2: start worker')).toBe('info')
    expect(detectLevel('12:00:01 DEBUG cache miss')).toBe('debug')
  })

  it('leaves lines that only mention a level in passing', () => {
    expect(detectLevel('GET /api/errors/list 200 12ms user=ada and some more text after it')).toBeUndefined()
    expect(detectLevel('listening on :8080')).toBeUndefined()
  })
})
