import { useEffect, useState } from 'react'
import { Spinner } from '@/components/ui/spinner'

// a deploy is one long request, so the elapsed time is the progress people can see
export function DeployStatus({ label }: { label: string }) {
  const [seconds, setSeconds] = useState(0)
  useEffect(() => {
    const started = Date.now()
    const timer = window.setInterval(() => setSeconds(Math.floor((Date.now() - started) / 1000)), 1000)
    return () => window.clearInterval(timer)
  }, [])
  return (
    <div role="status" className="flex items-center gap-3 border border-line bg-sunken px-4 py-3">
      <Spinner />
      <span className="flex-1 text-ui text-fg">{label}</span>
      <span className="tabular font-mono text-2xs text-fg-faint">{seconds}s</span>
    </div>
  )
}
