// where a service lives; every service endpoint is under its project and environment
export type Scope = { project: string; environment: string }

const encode = encodeURIComponent

export function scopePath({ project, environment }: Scope): string {
  return `/v1/projects/${encode(project)}/environments/${encode(environment)}`
}

export function servicePath(scope: Scope, service: string, rest = ''): string {
  return `${scopePath(scope)}/services/${encode(service)}${rest}`
}
