import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'

export type SessionInfo = Schemas['SessionSummary']
export type TokenInfo = Schemas['TokenSummary']
export type IssuedToken = Schemas['IssuedToken']

const user = (name: string, rest = '') => `/v1/users/${encodeURIComponent(name)}${rest}`


export function setPassword(name: string, body: Schemas['SetPassword']) {
  return api<void>(user(name, '/password'), { method: 'PUT', body })
}

export function startTwoFactor(name: string) {
  return api<Schemas['TwoFactorSetup']>(user(name, '/two-factor'), { method: 'POST' })
}

export function confirmTwoFactor(name: string, code: string) {
  const body: Schemas['TwoFactorCode'] = { code }
  return api<Schemas['RecoveryCodes']>(user(name, '/two-factor/confirm'), { method: 'POST', body })
}

// admins turning it off for someone else leave the password out
export function disableTwoFactor(name: string, password?: string) {
  const body: Schemas['DisableTwoFactor'] = { password }
  return api<void>(user(name, '/two-factor/disable'), { method: 'POST', body })
}

export const sessionsQuery = (name: string) =>
  queryOptions({
    queryKey: ['users', name, 'sessions'],
    queryFn: ({ signal }) => api<SessionInfo[]>(user(name, '/sessions'), { signal }),
  })

export function endSession(name: string, id: string) {
  return api<void>(user(name, `/sessions/${encodeURIComponent(id)}`), { method: 'DELETE' })
}

export const tokensQuery = (name: string) =>
  queryOptions({
    queryKey: ['users', name, 'tokens'],
    queryFn: ({ signal }) => api<TokenInfo[]>(user(name, '/tokens'), { signal }),
  })

export function createToken(name: string, body: Schemas['CreateToken']) {
  return api<IssuedToken>(user(name, '/tokens'), { method: 'POST', body })
}

export function revokeToken(name: string, token: string) {
  return api<void>(user(name, `/tokens/${encodeURIComponent(token)}`), { method: 'DELETE' })
}
