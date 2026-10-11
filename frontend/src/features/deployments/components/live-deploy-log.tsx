import type { Scope } from '@/lib/api/paths'
import { liveKey, useLiveDeploy } from '../live'
import { DeployLog } from './deploy-log'

// the live log of a deploy someone is waiting on in a dialog
export function LiveDeployLog({ scope, service }: { scope: Scope; service: string }) {
  const deploy = useLiveDeploy(liveKey(scope, service))
  return <DeployLog lines={deploy?.lines ?? []} live={deploy?.running ?? true} />
}
