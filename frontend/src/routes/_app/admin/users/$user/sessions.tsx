import { Section } from '@/components/ui/section'
import { SessionsList } from '@/features/account/components/sessions-list'
import { createFileRoute } from '@tanstack/react-router'
import { useUserDetail } from '@/features/users/use-user-detail'

export const Route = createFileRoute('/_app/admin/users/$user/sessions')({ component: Sessions })

function Sessions() {
  const { user } = useUserDetail()
  if (!user) return null
  return (
      <Section title="Sessions" description="Where they are signed in to the dashboard.">
        <SessionsList user={user.name} />
      </Section>
  )
}
