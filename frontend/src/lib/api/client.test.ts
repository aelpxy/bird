import { afterEach, describe, expect, it, vi } from 'vitest'
import { api, ApiError } from './client'

function respond(status: number, body: string) {
  vi.stubGlobal('fetch', vi.fn(async () => new Response(status === 204 ? null : body, { status })))
}

afterEach(() => vi.unstubAllGlobals())

describe('api', () => {
  it('accepts successes without a body, like the 202 scaling answers with', async () => {
    respond(202, '')
    await expect(api('/v1/x', { method: 'PUT', body: {} })).resolves.toBeUndefined()
    respond(204, '')
    await expect(api('/v1/x', { method: 'DELETE' })).resolves.toBeUndefined()
  })

  it('parses json bodies', async () => {
    respond(200, '{"name":"web"}')
    await expect(api('/v1/x')).resolves.toEqual({ name: 'web' })
  })

  it("turns birdd's error body into an ApiError", async () => {
    respond(409, '{"error":"another operation on web is in progress","logs":[]}')
    await expect(api('/v1/x')).rejects.toEqual(new ApiError(409, 'another operation on web is in progress'))
  })
})
