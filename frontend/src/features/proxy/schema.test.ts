import { describe, expect, it } from 'vitest'
import type { ProxySettings } from './api'
import { DEFAULT_MAX_BODY_MB, parseHeaders, proxySchema, toForm, toSettings } from './schema'

const unset: ProxySettings = { rate_limit: null, max_body_mb: null, timeout_secs: null, security_headers: false, headers: {}, maintenance: false, cloudflare: false }

describe('proxy settings form', () => {
  it('reads headers one per line and lowercases their names', () => {
    expect(parseHeaders('Cache-Control: no-store\n\nx-robots-tag:  noindex ')).toEqual({
      headers: { 'cache-control': 'no-store', 'x-robots-tag': 'noindex' },
    })
    expect(parseHeaders('no colon here')).toEqual({ error: expect.stringContaining('Line 1') })
  })

  it('round-trips settings and sends defaults back as unset', () => {
    const form = toForm({ ...unset, cloudflare: true, rate_limit: { per_second: 5, burst: 9 }, timeout_secs: 300, headers: { a: 'b' } })
    expect(form.maxBodyMb).toBe(DEFAULT_MAX_BODY_MB)
    expect(toSettings(form, true)).toEqual({
      rate_limit: { per_second: 5, burst: 9 },
      max_body_mb: null,
      timeout_secs: 300,
      security_headers: false,
      headers: { a: 'b' },
      maintenance: true,
      cloudflare: true,
    })
  })

  it('reports a bad header line as a field error', () => {
    const result = proxySchema.safeParse({ ...toForm(unset), headers: 'oops' })
    expect(result.success).toBe(false)
  })
})
