import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'

export type Project = Schemas['ProjectSummary']
export type Org = Schemas['OrgSummary']

export const projectsQuery = queryOptions({
  queryKey: ['projects'],
  queryFn: ({ signal }) => api<Project[]>('/v1/projects', { signal }),
})

export const orgsQuery = queryOptions({
  queryKey: ['orgs'],
  queryFn: ({ signal }) => api<Org[]>('/v1/orgs', { signal }),
})

export function createProject(body: Schemas['CreateProject']) {
  return api<Project>('/v1/projects', { method: 'POST', body })
}

export function createEnvironment(project: string, body: Schemas['CreateEnvironment']) {
  return api<string>(`/v1/projects/${encodeURIComponent(project)}/environments`, {
    method: 'POST',
    body,
  })
}

// production when there is one, since that is what people usually come to look at
export function preferredEnvironment(project: Project): string | undefined {
  return project.environments.includes('production') ? 'production' : project.environments[0]
}

export function removeProject(project: string) {
  return api<void>(`/v1/projects/${encodeURIComponent(project)}`, { method: 'DELETE' })
}

export function removeEnvironment(project: string, environment: string) {
  return api<void>(
    `/v1/projects/${encodeURIComponent(project)}/environments/${encodeURIComponent(environment)}`,
    { method: 'DELETE' },
  )
}
