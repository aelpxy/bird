import { describe, expect, it } from 'vitest'
import { parseDotenv } from './dotenv'

describe('parseDotenv', () => {
  it('reads the usual .env forms', () => {
    const text = [
      '# settings',
      'PORT=3000',
      'export NODE_ENV=production',
      'GREETING="hello world"',
      "RAW='a \\n b'",
      'MULTI="line one\\nline two"',
      'URL=${{postgres.DATABASE_URL}} # from the database',
      'EMPTY=',
      '',
    ].join('\n')
    expect(parseDotenv(text)).toEqual({
      ok: true,
      values: {
        PORT: '3000',
        NODE_ENV: 'production',
        GREETING: 'hello world',
        RAW: 'a \\n b',
        MULTI: 'line one\nline two',
        URL: '${{postgres.DATABASE_URL}}',
        EMPTY: '',
      },
    })
  })

  it('names the line that is wrong', () => {
    expect(parseDotenv('A=1\nnot a pair')).toEqual({ ok: false, error: 'Line 2 has no "=": not a pair' })
    expect(parseDotenv('1BAD=x')).toEqual({ ok: false, error: 'Line 1: 1BAD is not a valid variable name.' })
  })
})
