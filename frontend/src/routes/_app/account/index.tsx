import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { sessionQuery } from '@/features/auth/api'
import { PasswordForm } from '@/features/account/components/password-form'

export const Route = createFileRoute('/_app/account/')({ component: Password })

function Password() {
  const { session: initial } = Route.useRouteContext()
  const { data: session = initial } = useQuery(sessionQuery)
  return session ? <PasswordForm session={session} /> : null
}
