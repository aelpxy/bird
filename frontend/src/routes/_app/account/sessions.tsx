import { createFileRoute } from '@tanstack/react-router'
import { SessionsList } from '@/features/account/components/sessions-list'

export const Route = createFileRoute('/_app/account/sessions')({ component: Sessions })

function Sessions() {
  const { session } = Route.useRouteContext()
  return <SessionsList user={session.name} />
}
