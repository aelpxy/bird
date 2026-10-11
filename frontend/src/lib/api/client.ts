import type { components } from './schema.gen'

export type Schemas = components['schemas']

// birdd answered with an error status; `message` is its error text
export class ApiError extends Error {
  readonly status: number
  readonly logs: string[]

  constructor(status: number, message: string, logs: string[] = []) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.logs = logs
  }
}

// the request never got an answer, so nothing is known about what birdd did
export class NetworkError extends Error {
  constructor(cause: unknown) {
    super('Could not reach birdd', { cause })
    this.name = 'NetworkError'
  }
}

type RequestOptions = {
  method?: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'
  body?: unknown
  signal?: AbortSignal
}

// same-origin, so the session cookie goes along and the browser adds the Origin birdd checks
export async function api<T>(
  path: string,
  { method = 'GET', body, signal }: RequestOptions = {},
): Promise<T> {
  let response: Response
  try {
    response = await fetch(path, {
      method,
      signal,
      credentials: 'same-origin',
      headers: body === undefined ? undefined : { 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    })
  } catch (cause) {
    if (signal?.aborted) throw cause
    throw new NetworkError(cause)
  }
  if (!response.ok) throw await toApiError(response)
  // some successes carry no body, like 204 and the 202 a scale answers with
  const text = await response.text()
  return (text ? JSON.parse(text) : undefined) as T
}

async function toApiError(response: Response): Promise<ApiError> {
  const text = await response.text()
  try {
    const body = JSON.parse(text) as Schemas['ErrorBody']
    return new ApiError(response.status, body.error, body.logs ?? [])
  } catch {
    return new ApiError(response.status, text || response.statusText)
  }
}

export function isUnauthorized(error: unknown): boolean {
  return error instanceof ApiError && error.status === 401
}

// reads a newline-delimited JSON stream, like followed logs, until it ends or `signal` aborts
export async function streamJsonLines<T>(
  path: string,
  onItem: (item: T) => void,
  signal: AbortSignal,
  init: { method?: 'GET' | 'POST'; body?: BodyInit; contentType?: string; accept?: string } = {},
): Promise<void> {
  let response: Response
  try {
    response = await fetch(path, {
      signal,
      credentials: 'same-origin',
      method: init.method ?? 'GET',
      body: init.body,
      headers: {
        ...(init.contentType ? { 'Content-Type': init.contentType } : {}),
        ...(init.accept ? { Accept: init.accept } : {}),
      },
    })
  } catch (cause) {
    if (signal.aborted) return
    throw new NetworkError(cause)
  }
  if (!response.ok || !response.body) throw await toApiError(response)
  const reader = response.body.pipeThrough(new TextDecoderStream()).getReader()
  let buffered = ''
  try {
    for (;;) {
      const { value, done } = await reader.read()
      if (done) return
      buffered += value
      const lines = buffered.split('\n')
      buffered = lines.pop() ?? ''
      for (const line of lines) {
        if (line.trim()) onItem(JSON.parse(line) as T)
      }
    }
  } catch (cause) {
    if (signal.aborted) return
    throw new NetworkError(cause)
  }
}
