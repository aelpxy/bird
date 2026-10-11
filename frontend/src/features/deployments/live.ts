import { useSyncExternalStore } from 'react'
import type { Scope } from '@/lib/api/paths'
import type { DeployLogLine } from './api'

// deploys this browser started, by service, so any page of that service can show their log
export type LiveDeploy = { lines: DeployLogLine[]; running: boolean; startedAt: number; error?: string }

const deploys = new Map<string, LiveDeploy>()
const listeners = new Set<() => void>()

export function liveKey(scope: Scope, service: string): string {
  return `${scope.project}/${scope.environment}/${service}`
}

function set(key: string, deploy: LiveDeploy | undefined) {
  if (deploy) deploys.set(key, deploy)
  else deploys.delete(key)
  for (const listener of listeners) listener()
}

export function startLive(key: string) {
  set(key, { lines: [], running: true, startedAt: Date.now() })
}

export function pushLive(key: string, line: DeployLogLine) {
  const current = deploys.get(key)
  if (current) set(key, { ...current, lines: [...current.lines, line] })
}

export function endLive(key: string, error?: string) {
  const current = deploys.get(key)
  // a deploy that worked has nothing left to read; a failed one stays until dismissed
  set(key, error && current ? { ...current, running: false, error } : undefined)
}

// birdd's newest step says what the deploy is doing; app and build output is too noisy for a summary
export function currentStep(deploy: LiveDeploy): string | undefined {
  return deploy.lines.findLast((line) => line.kind === 'bird')?.text
}

export function dismissLive(key: string) {
  set(key, undefined)
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function liveDeploy(key: string): LiveDeploy | undefined {
  return deploys.get(key)
}

export function useLiveDeploy(key: string): LiveDeploy | undefined {
  return useSyncExternalStore(subscribe, () => liveDeploy(key))
}
