import { useQuery } from '@tanstack/react-query'
import { useParams } from '@tanstack/react-router'
import { useEffect } from 'react'
import type { Scope } from '@/lib/api/paths'
import { orgsQuery, preferredEnvironment, projectsQuery, type Project } from '@/features/projects/api'

const STORAGE_KEY = 'bird.scope'

export type CurrentScope = { orgs: string[]; org?: string; projects: Project[]; project?: Project; scope?: Scope }

function remembered(): Partial<Scope> {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}')
    return typeof value === 'object' && value !== null ? (value as Partial<Scope>) : {}
  } catch {
    return {}
  }
}

// the project and environment in the address, else the last ones visited, so pages outside a project keep the sidebar's place
export function useCurrentScope(): CurrentScope {
  const params: { project?: string; environment?: string; org?: string } = useParams({ strict: false })
  const { data: projects = [] } = useQuery(projectsQuery)
  const { data: orgList = [] } = useQuery(orgsQuery)
  const last = remembered()
  const project =
    projects.find((candidate) => candidate.name === params.project) ??
    (params.org ? projects.find((candidate) => candidate.org === params.org) : undefined) ??
    projects.find((candidate) => candidate.name === last.project) ??
    projects[0]
  const wanted = project?.name === params.project ? params.environment : project?.name === last.project ? last.environment : undefined
  const environment = project && (wanted && project.environments.includes(wanted) ? wanted : preferredEnvironment(project))
  const orgs = [...new Set([...orgList.map((org) => org.name), ...projects.map((candidate) => candidate.org)])].sort()

  useEffect(() => {
    if (params.project && params.environment) {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ project: params.project, environment: params.environment }))
    }
  }, [params.project, params.environment])

  return {
    orgs,
    org: params.org ?? project?.org ?? orgs[0],
    projects,
    project,
    scope: project && environment ? { project: project.name, environment } : undefined,
  }
}
