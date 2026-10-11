import { describe, expect, it } from 'vitest'
import type { Machine, Service } from './api'
import { serviceStatus } from './status'

const machine = (state: Machine['state']): Machine => ({ id: crypto.randomUUID(), state, updated_at: 0 })

const service = (overrides: Partial<Service> = {}): Service => ({
  name: 'web',
  image: 'nginx',
  port: 80,
  replicas: 2,
  state: 'running',
  health: 'http',
  health_timeout: 60,
  memory: 1024,
  cpus: 1000,
  domains: [],
  volumes: [],
  deployment: { id: 'd', created_at: 0, status: 'active', machines: [machine('running'), machine('running')] },
  ...overrides,
})

describe('serviceStatus', () => {
  it('is running when every machine is', () => {
    expect(serviceStatus(service()).label).toBe('Running')
  })

  it('is degraded when only some machines run', () => {
    const deployment = { id: 'd', created_at: 0, status: 'active' as const, machines: [machine('running'), machine('failed')] }
    expect(serviceStatus(service({ deployment })).label).toBe('Degraded')
  })

  it('puts the desired state first', () => {
    expect(serviceStatus(service({ state: 'stopped' })).label).toBe('Stopped')
  })

  it('tells a failed first deploy from one never tried', () => {
    expect(serviceStatus(service({ deployment: null })).label).toBe('Not deployed')
    expect(serviceStatus(service({ deployment: null, failed_deploy: { id: 'f', created_at: 0 } })).label).toBe('Deploy failed')
  })

  it('is deploying while a deploy runs', () => {
    const deployment = { id: 'd', created_at: 0, status: 'deploying' as const, machines: [] }
    expect(serviceStatus(service({ deployment }))).toMatchObject({ label: 'Deploying', busy: true })
  })
})
