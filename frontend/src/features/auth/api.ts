import { queryOptions } from '@tanstack/react-query'
import { api, isUnauthorized, type Schemas } from '@/lib/api/client'

export type Session = Schemas['Whoami']

export type Credentials = {
  username: string
  password: string
  code?: string
}

export type SignInResult = 'signed_in' | 'two_factor_required'

// who is signed in, or null when nobody is; the cookie itself is unreadable from scripts
export const sessionQuery = queryOptions({
  queryKey: ['session'],
  queryFn: async ({ signal }): Promise<Session | null> => {
    try {
      return await api<Session>('/v1/me', { signal })
    } catch (error) {
      if (isUnauthorized(error)) return null
      throw error
    }
  },
  staleTime: 5 * 60_000,
})

export async function signIn(credentials: Credentials): Promise<SignInResult> {
  const body: Schemas['LoginRequest'] = { ...credentials, cookie: true }
  const response = await api<Schemas['LoginResponse']>('/v1/login', {
    method: 'POST',
    body,
  })
  switch (response.status) {
    case 'two_factor_required':
      return 'two_factor_required'
    case 'signed_in':
    case 'signed_in_with_cookie':
      return 'signed_in'
  }
}

export async function signOut(): Promise<void> {
  try {
    await api('/v1/logout', { method: 'POST' })
  } catch (error) {
    // an expired session is already signed out
    if (!isUnauthorized(error)) throw error
  }
}
