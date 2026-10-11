import { Link, useMatchRoute } from '@tanstack/react-router'
import { ArrowRight, X } from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import { useEffect, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { buttonClass } from '@/components/ui/button-styles'
import { IconButton } from '@/components/ui/icon-button'
import { Panel } from '@/components/ui/panel'
import { Spinner } from '@/components/ui/spinner'
import { spring } from '@/lib/motion'
import type { Scope } from '@/lib/api/paths'
import { currentStep, dismissLive, liveKey, useLiveDeploy } from '../live'

const deploymentsRoute = '/projects/$project/$environment/services/$service/deployments'

function useElapsed(since: number, running: boolean): string {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!running) return
    const timer = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(timer)
  }, [running])
  const seconds = Math.max(0, Math.floor((now - since) / 1000))
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
}

// one line above a service's tabs while a deploy started here runs, and after it fails; the log lives in Deployments
export function DeployStatusBar({ scope, service }: { scope: Scope; service: string }) {
  const key = liveKey(scope, service)
  const deploy = useLiveDeploy(key)
  const elapsed = useElapsed(deploy?.startedAt ?? 0, deploy?.running ?? false)
  const onDeployments = useMatchRoute()({ to: deploymentsRoute, params: { ...scope, service } })
  const step = deploy?.running ? (currentStep(deploy) ?? 'Starting the deploy') : deploy?.error
  // it drops in from above the tabs and leaves the same way
  return (
    <AnimatePresence initial={false}>
      {deploy && (
        <motion.div key="deploy" initial={{ opacity: 0, y: -6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }} transition={spring}>
          <Panel tone={deploy.error ? 'danger' : 'default'} padded={false} className="flex h-12 items-center gap-3 px-4">
            {deploy.running ? <Spinner className="size-3.5 shrink-0 text-fg-muted" /> : <Badge tone="bad">Failed</Badge>}
            <p className="flex min-w-0 flex-1 items-baseline gap-2 text-ui" role="status">
              {deploy.running && <span className="shrink-0 font-medium text-fg">Deploying</span>}
              <span className={`truncate ${deploy.running ? 'text-fg-muted' : 'text-fg'}`} title={step}>
                {step}
              </span>
            </p>
            {deploy.running && <span className="tabular shrink-0 font-mono text-xs text-fg-faint">{elapsed}</span>}
            {!onDeployments && (
              <Link to={deploymentsRoute} params={{ ...scope, service }} className={buttonClass('ghost', 'sm')}>
                View logs
                <ArrowRight aria-hidden className="size-3.5" />
              </Link>
            )}
            {!deploy.running && (
              <IconButton label="Dismiss" onClick={() => dismissLive(key)}>
                <X aria-hidden className="size-4" />
              </IconButton>
            )}
          </Panel>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
