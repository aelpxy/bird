import { z } from 'zod'
import { wholeNumber } from '@/lib/validation'
import type { ProxySettings } from './api'

// what birdd uses when a setting is left out
export const DEFAULT_MAX_BODY_MB = 100
export const DEFAULT_TIMEOUT_SECS = 60
const SUGGESTED_PER_SECOND = 10
const SUGGESTED_BURST = 20
const HEADER_NAME = /^[A-Za-z0-9!#$%&'*+.^_`|~-]+$/

// one header per line as `name: value`, the way they look on the wire
export function parseHeaders(text: string): { headers: Record<string, string> } | { error: string } {
  const headers: Record<string, string> = {}
  for (const [index, raw] of text.split('\n').entries()) {
    const line = raw.trim()
    if (!line) continue
    const colon = line.indexOf(':')
    const name = colon > 0 ? line.slice(0, colon).trim().toLowerCase() : ''
    if (!HEADER_NAME.test(name)) return { error: `Line ${index + 1}: write it as name: value, like cache-control: no-store.` }
    headers[name] = line.slice(colon + 1).trim()
  }
  return { headers }
}

export function formatHeaders(headers: Record<string, string> | undefined): string {
  return Object.entries(headers ?? {})
    .map(([name, value]) => `${name}: ${value}`)
    .join('\n')
}

export const proxySchema = z.object({
  rateLimited: z.boolean(),
  perSecond: wholeNumber(1, 10_000, 'requests per second'),
  burst: wholeNumber(1, 100_000, 'the burst'),
  maxBodyMb: wholeNumber(1, 10_240, 'the body limit'),
  timeoutSecs: wholeNumber(1, 3_600, 'the timeout'),
  securityHeaders: z.boolean(),
  headers: z.string().refine((text) => 'headers' in parseHeaders(text), {
    error: (issue) => {
      const parsed = parseHeaders(String(issue.input))
      return 'error' in parsed ? parsed.error : 'Check the headers.'
    },
  }),
})

export type ProxyForm = z.infer<typeof proxySchema>

export function toForm(settings: ProxySettings): ProxyForm {
  return {
    rateLimited: Boolean(settings.rate_limit),
    perSecond: settings.rate_limit?.per_second ?? SUGGESTED_PER_SECOND,
    burst: settings.rate_limit?.burst ?? SUGGESTED_BURST,
    maxBodyMb: settings.max_body_mb ?? DEFAULT_MAX_BODY_MB,
    timeoutSecs: settings.timeout_secs ?? DEFAULT_TIMEOUT_SECS,
    securityHeaders: settings.security_headers ?? false,
    headers: formatHeaders(settings.headers),
  }
}

// defaults go back as unset, so a later change of birdd's defaults still reaches this app
export function toSettings(form: ProxyForm, maintenance: boolean): ProxySettings {
  const parsed = parseHeaders(form.headers)
  return {
    rate_limit: form.rateLimited ? { per_second: form.perSecond, burst: form.burst } : null,
    max_body_mb: form.maxBodyMb === DEFAULT_MAX_BODY_MB ? null : form.maxBodyMb,
    timeout_secs: form.timeoutSecs === DEFAULT_TIMEOUT_SECS ? null : form.timeoutSecs,
    security_headers: form.securityHeaders,
    headers: 'headers' in parsed ? parsed.headers : {},
    maintenance,
  }
}
