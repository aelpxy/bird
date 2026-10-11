import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { sessionQuery } from '@/features/auth/api'
import { TwoFactorPanel } from '@/features/account/components/two-factor-panel'

export const Route = createFileRoute('/_app/account/two-factor')({ component: TwoFactor })

function TwoFactor() {
  const { session: initial } = Route.useRouteContext()
  const { data: session = initial } = useQuery(sessionQuery)
  return session ? <TwoFactorPanel session={session} /> : null
}
