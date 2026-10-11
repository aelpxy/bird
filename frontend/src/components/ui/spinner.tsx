import { LoaderCircle } from 'lucide-react'

export function Spinner({ className = 'size-3.5' }: { className?: string }) {
  return <LoaderCircle aria-hidden className={`animate-spin ${className}`} />
}
