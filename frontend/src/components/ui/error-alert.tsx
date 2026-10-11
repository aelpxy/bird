import { ApiError } from '@/lib/api/client'
import { errorMessage } from '@/lib/toast'
import { Alert } from './alert'

// birdd's message, plus the app's last log lines when a deploy failed its health check
export function ErrorAlert({ error }: { error: unknown }) {
  const logs = error instanceof ApiError ? error.logs : []
  return (
    <Alert>
      <span className="block">{errorMessage(error)}</span>
      {logs.length > 0 && (
        <details className="mt-2">
          <summary className="cursor-pointer font-mono text-2xs tracking-[0.08em] text-fg-muted uppercase hover:text-fg">Last log lines</summary>
          <pre className="mt-2 max-h-48 overflow-auto border border-line bg-sunken p-2 font-mono text-xs whitespace-pre-wrap text-fg-muted">{logs.join('\n')}</pre>
        </details>
      )}
    </Alert>
  )
}
