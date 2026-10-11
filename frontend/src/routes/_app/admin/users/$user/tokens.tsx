import { Section } from '@/components/ui/section'
import { TokensPanel } from '@/features/account/components/tokens-panel'
import { createFileRoute } from '@tanstack/react-router'
import { useUserDetail } from '@/features/users/use-user-detail'

export const Route = createFileRoute('/_app/admin/users/$user/tokens')({ component: Tokens })

function Tokens() {
  const { user } = useUserDetail()
  if (!user) return null
  return (
      <Section title="API tokens" description="Tokens act as this user, with their access.">
        <TokensPanel user={user.name} />
      </Section>
  )
}
