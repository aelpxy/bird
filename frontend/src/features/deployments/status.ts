import type { Tone } from '@/components/ui/badge'
import type { Deployment } from './api'

export const deploymentStatuses: Record<Deployment['status'], { label: string; tone: Tone; dot: string }> = {
  pending: { label: 'Pending', tone: 'neutral', dot: 'bg-fg-faint' },
  deploying: { label: 'Deploying', tone: 'info', dot: 'bg-info' },
  active: { label: 'Live', tone: 'ok', dot: 'bg-ok' },
  superseded: { label: 'Superseded', tone: 'neutral', dot: 'bg-fg-faint' },
  failed: { label: 'Failed', tone: 'bad', dot: 'bg-bad' },
}

// the tag is what changes between deploys, so it is what a row leads with
export function splitImage(image: string): { repository: string; tag: string } {
  const at = image.indexOf('@')
  if (at >= 0) return { repository: image.slice(0, at), tag: image.slice(at + 1, at + 20) }
  const slash = image.lastIndexOf('/')
  const colon = image.lastIndexOf(':')
  return colon > slash ? { repository: image.slice(0, colon), tag: image.slice(colon + 1) } : { repository: image, tag: 'latest' }
}

export function shortId(id: string): string {
  return id.slice(0, 8)
}

// the deploy in progress first, else the one serving traffic, else the newest
export function preferredDeployment(deployments: Deployment[]): Deployment | undefined {
  return deployments.find((d) => d.status === 'deploying') ?? deployments.find((d) => d.status === 'active') ?? deployments[0]
}
