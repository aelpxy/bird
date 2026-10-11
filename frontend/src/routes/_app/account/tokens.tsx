import { createFileRoute } from '@tanstack/react-router'
import { TokensPanel } from '@/features/account/components/tokens-panel'

export const Route = createFileRoute('/_app/account/tokens')({ component: Tokens })

function Tokens() {
  const { session } = Route.useRouteContext()
  return <TokensPanel user={session.name} />
}
