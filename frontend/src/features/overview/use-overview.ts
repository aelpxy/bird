import { useQueries, useQuery } from '@tanstack/react-query'
import type { Scope } from '@/lib/api/paths'
import { projectsQuery, type Project } from '@/features/projects/api'
import { servicesQuery, type Service } from '@/features/services/api'
import { runningMachines, serviceStatus } from '@/features/services/status'

export type PlacedService = { scope: Scope; service: Service }

export type Overview = {
  projects: Project[]
  services: PlacedService[]
  attention: PlacedService[]
  recent: PlacedService[]
  machines: { running: number; wanted: number }
  loading: boolean
  error: unknown
}

const RECENT = 6

// every service in every environment, fetched side by side and kept live
export function useOverview(): Overview {
  const projects = useQuery(projectsQuery)
  const scopes: Scope[] = (projects.data ?? []).flatMap((project) =>
    project.environments.map((environment) => ({ project: project.name, environment })),
  )
  const results = useQueries({ queries: scopes.map((scope) => servicesQuery(scope)) })
  const services = results.flatMap((result, index) => (result.data ?? []).map((service) => ({ scope: scopes[index], service })))

  const attention = services.filter(({ service }) => {
    const status = serviceStatus(service)
    return status.tone === 'bad' || status.tone === 'warn' || Boolean(service.failed_deploy)
  })
  const recent = services
    .filter(({ service }) => service.deployment)
    .sort((a, b) => (b.service.deployment?.created_at ?? 0) - (a.service.deployment?.created_at ?? 0))
    .slice(0, RECENT)
  const running = services.filter(({ service }) => service.state === 'running')
  const machines = {
    running: running.reduce((sum, { service }) => sum + runningMachines(service.deployment?.machines ?? []), 0),
    wanted: running.reduce((sum, { service }) => sum + service.replicas, 0),
  }

  return {
    projects: projects.data ?? [],
    services,
    attention,
    recent,
    machines,
    loading: projects.isPending || results.some((result) => result.isPending),
    error: projects.error ?? results.find((result) => result.error)?.error ?? null,
  }
}
