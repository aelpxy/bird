import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'
import { scopePath, type Scope } from '@/lib/api/paths'

export type Template = Schemas['TemplateSummary']

export const templatesQuery = queryOptions({
  queryKey: ['templates'],
  queryFn: ({ signal }) => api<Template[]>('/v1/templates', { signal }),
  staleTime: Infinity,
})

export function deployTemplate(scope: Scope, template: string, name?: string) {
  const body: Schemas['CreateFromTemplate'] = { name: name || null }
  return api<Schemas['TemplateDeployResponse']>(
    `${scopePath(scope)}/templates/${encodeURIComponent(template)}/deploy`,
    { method: 'POST', body },
  )
}
