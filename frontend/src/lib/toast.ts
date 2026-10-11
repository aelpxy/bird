import { Toast } from '@base-ui/react/toast'
import { ApiError, NetworkError } from './api/client'

// one manager for the whole app, so code outside components can raise toasts too
export const toastManager = Toast.createToastManager()

export function notifySuccess(title: string, description?: string) {
  toastManager.add({ title, description, type: 'success' })
}

export function notifyError(title: string, error: unknown) {
  toastManager.add({ title, description: errorMessage(error), type: 'error', priority: 'high' })
}

// birdd's messages are written for people, so they are shown as they come
export function errorMessage(error: unknown): string {
  if (error instanceof NetworkError) return "Can't reach birdd. Check your connection and try again."
  if (error instanceof ApiError) {
    const text = error.message.charAt(0).toUpperCase() + error.message.slice(1)
    return text.endsWith('.') ? text : `${text}.`
  }
  // errors raised here, like a missing Dockerfile, are already written for people
  if (error instanceof Error && error.message) return error.message
  return 'Something went wrong. Try again in a moment.'
}
