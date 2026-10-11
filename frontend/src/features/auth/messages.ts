import { ApiError, NetworkError } from '@/lib/api/client'

// birdd only knows users named like this, so anything else cannot sign in
export const USERNAME_PATTERN = /^[a-z][a-z0-9-]{0,62}$/

export type SignInStep = 'password' | 'code'

export const WRONG_CREDENTIALS = 'Wrong username or password.'

export function signInErrorMessage(error: unknown, step: SignInStep): string {
  if (error instanceof NetworkError) {
    return "Can't reach birdd. Check your connection and try again."
  }
  if (error instanceof ApiError) {
    if (error.status === 401) {
      return step === 'password' ? WRONG_CREDENTIALS : "That code didn't work. Check it and try again."
    }
    if (error.status === 429) return asSentence(error.message)
  }
  return 'Something went wrong. Try again in a moment.'
}

function asSentence(text: string): string {
  const capitalized = text.charAt(0).toUpperCase() + text.slice(1)
  return capitalized.endsWith('.') ? capitalized : `${capitalized}.`
}
