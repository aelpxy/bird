import { useEffect, useState } from 'react'
import type { Scope } from '@/lib/api/paths'
import { deploymentLog, followDeploymentLog, type Deployment, type DeployLogLine } from './api'

export type LoadedLog = { lines: DeployLogLine[]; following: boolean; loading: boolean; error?: unknown }

// a deploy still running is followed until it ends; a finished one is read once. Callers key the
// component by deployment, so the state starts fresh for each one
export function useDeploymentLog(scope: Scope, service: string, deployment: Deployment): LoadedLog {
  const running = deployment.status === 'deploying'
  const [log, setLog] = useState<LoadedLog>({ lines: [], following: running, loading: true })
  useEffect(() => {
    const controller = new AbortController()
    const fail = (error: unknown) => {
      if (!controller.signal.aborted) setLog((current) => ({ ...current, following: false, loading: false, error }))
    }
    if (running) {
      followDeploymentLog(
        scope,
        service,
        deployment.id,
        (line) => setLog((current) => ({ ...current, loading: false, lines: [...current.lines, line] })),
        controller.signal,
      )
        .then(() => {
          if (!controller.signal.aborted) setLog((current) => ({ ...current, loading: false, following: false }))
        })
        .catch(fail)
    } else {
      deploymentLog(scope, service, deployment.id, controller.signal)
        .then((saved) => setLog({ lines: saved.lines, following: false, loading: false }))
        .catch(fail)
    }
    return () => controller.abort()
  }, [scope, service, deployment.id, running])
  return log
}
