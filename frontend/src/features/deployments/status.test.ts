import { describe, expect, it } from 'vitest'
import type { Deployment } from './api'
import { preferredDeployment, splitImage } from './status'

const deployment = (id: string, status: Deployment['status']): Deployment => ({ id, status, image: 'web:1', port: 80, variables: 0, created_at: 0 })

describe('deployment presentation', () => {
  it('splits images into repository and tag', () => {
    expect(splitImage('docker.io/library/nginx:alpine')).toEqual({ repository: 'docker.io/library/nginx', tag: 'alpine' })
    expect(splitImage('localhost:5000/web')).toEqual({ repository: 'localhost:5000/web', tag: 'latest' })
    expect(splitImage('ghcr.io/a/b@sha256:0123456789abcdef0123')).toEqual({ repository: 'ghcr.io/a/b', tag: 'sha256:0123456789ab' })
  })

  it('opens on the deploy in progress, else a newest one that failed, else the live one', () => {
    expect(preferredDeployment([deployment('a', 'failed'), deployment('b', 'active')])?.id).toBe('a')
    expect(preferredDeployment([deployment('a', 'superseded'), deployment('b', 'active')])?.id).toBe('b')
    expect(preferredDeployment([deployment('a', 'deploying'), deployment('b', 'active')])?.id).toBe('a')
    expect(preferredDeployment([deployment('a', 'failed')])?.id).toBe('a')
  })
})
