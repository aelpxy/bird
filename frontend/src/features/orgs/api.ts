import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'

export type OrgRole = Schemas['OrgRole']
export type Member = Schemas['OrgMember']

const org = (name: string, rest = '') => `/v1/orgs/${encodeURIComponent(name)}${rest}`

export const membersQuery = (name: string) =>
  queryOptions({
    queryKey: ['orgs', name, 'members'],
    queryFn: ({ signal }) => api<Member[]>(org(name, '/members'), { signal }),
  })

export function createOrg(name: string) {
  const body: Schemas['CreateOrg'] = { name }
  return api<Schemas['OrgSummary']>('/v1/orgs', { method: 'POST', body })
}

export function removeOrg(name: string) {
  return api<void>(org(name), { method: 'DELETE' })
}

export function setMember(name: string, user: string, role: OrgRole) {
  const body: Schemas['SetMember'] = { role }
  return api<void>(org(name, `/members/${encodeURIComponent(user)}`), { method: 'PUT', body })
}

export function removeMember(name: string, user: string) {
  return api<void>(org(name, `/members/${encodeURIComponent(user)}`), { method: 'DELETE' })
}

export const ORG_ROLES: { value: OrgRole; label: string; description: string }[] = [
  { value: 'owner', label: 'Owner', description: 'Everything, including deleting the org' },
  { value: 'admin', label: 'Admin', description: 'Manages members and projects' },
  { value: 'member', label: 'Member', description: 'Works in the org’s projects' },
]
