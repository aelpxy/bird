import { RotateCw, TriangleAlert } from 'lucide-react'
import { errorMessage } from '@/lib/toast'
import { Button } from './button'

type ErrorStateProps = {
  title?: string
  error: unknown
  onRetry?: () => void
}

export function ErrorState({ title = "Couldn't load this", error, onRetry }: ErrorStateProps) {
  return (
    <div role="alert" className="flex flex-col items-center border border-bad/40 bg-bad/5 px-6 py-12 text-center">
      <TriangleAlert aria-hidden className="size-6 text-bad" />
      <p className="mt-3 text-sm font-medium text-fg">{title}</p>
      <p className="mt-1 max-w-md text-sm text-fg-muted">{errorMessage(error)}</p>
      {onRetry && (
        <Button variant="secondary" size="sm" onClick={onRetry} className="mt-5">
          <RotateCw aria-hidden className="size-3.5" />
          Try again
        </Button>
      )}
    </div>
  )
}
