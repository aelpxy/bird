import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useParams, useRouteContext } from '@tanstack/react-router'
import { usersQuery } from './api'

// the user an admin page is about, and whether that is the admin themselves
export function useUserDetail() {
  const { user: name } = useParams({ from: '/_app/admin/users/$user' })
  const { session } = useRouteContext({ from: '/_app' })
  const queryClient = useQueryClient()
  const users = useQuery(usersQuery)
  const user = users.data?.find((candidate) => candidate.name === name)
  return {
    name,
    user,
    users,
    isSelf: name === session.name,
    refresh: () => queryClient.invalidateQueries({ queryKey: usersQuery.queryKey }),
  }
}
