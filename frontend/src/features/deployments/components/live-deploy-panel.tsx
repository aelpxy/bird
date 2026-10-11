import { X } from 'lucide-react'
import { IconButton } from '@/components/ui/icon-button'
import { Panel } from '@/components/ui/panel'
import { Spinner } from '@/components/ui/spinner'
import type { Scope } from '@/lib/api/paths'
import { dismissLive, liveKey, useLiveDeploy } from '../live'
import { DeployLog } from './deploy-log'

// shown above a service's tabs while a deploy started from this browser runs, and after it fails
export function LiveDeployPanel({ scope, service }: { scope: Scope; service: string }) {
  const key = liveKey(scope, service)
  const deploy = useLiveDeploy(key)
  if (!deploy) return null
  return (
    <Panel tone={deploy.error ? 'danger' : 'default'} className="flex flex-col gap-3">
      <div className="flex items-center gap-3">
        {deploy.running && <Spinner />}
        <p className="flex-1 text-sm font-medium text-fg">{deploy.running ? `Deploying ${service}` : `The deploy failed: ${deploy.error}`}</p>
        {!deploy.running && (
          <IconButton label="Dismiss the deploy log" onClick={() => dismissLive(key)}>
            <X aria-hidden className="size-4" />
          </IconButton>
        )}
      </div>
      <DeployLog lines={deploy.lines} live={deploy.running} />
    </Panel>
  )
}

// the live log of a deploy someone is waiting on in a dialog
export function LiveDeployLog({ scope, service }: { scope: Scope; service: string }) {
  const deploy = useLiveDeploy(liveKey(scope, service))
  return <DeployLog lines={deploy?.lines ?? []} live={deploy?.running ?? true} />
}
