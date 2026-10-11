import type { Tone } from '@/components/ui/badge'
import type { Machine, Service } from './api'

export type ServiceStatus = { label: string; tone: Tone; busy: boolean }

// one word for the list, from the desired state, the deploy in progress and the machines
export function serviceStatus(service: Service): ServiceStatus {
  const deployment = service.deployment
  if (service.state === 'stopped') return { label: 'Stopped', tone: 'neutral', busy: false }
  if (!deployment) {
    return service.failed_deploy
      ? { label: 'Deploy failed', tone: 'bad', busy: false }
      : { label: 'Not deployed', tone: 'neutral', busy: false }
  }
  if (deployment.status === 'deploying' || deployment.status === 'pending') {
    return { label: 'Deploying', tone: 'info', busy: true }
  }
  const running = runningMachines(deployment.machines)
  if (running === 0) return { label: 'Down', tone: 'bad', busy: false }
  if (running < service.replicas) return { label: 'Degraded', tone: 'warn', busy: true }
  return { label: 'Running', tone: 'ok', busy: false }
}

export function runningMachines(machines: Machine[]): number {
  return machines.filter((machine) => machine.state === 'running').length
}

export const machineTones: Record<Machine['state'], Tone> = {
  created: 'neutral',
  starting: 'info',
  running: 'ok',
  stopping: 'warn',
  stopped: 'neutral',
  failed: 'bad',
  destroyed: 'neutral',
}
