import { Link, Outlet, createFileRoute } from '@tanstack/react-router'
import { UserX } from 'lucide-react'
import { SettingsLayout, settingsLinkClass } from '@/components/layout/settings-layout'
import { BackLink } from '@/components/ui/back-link'
import { Badge } from '@/components/ui/badge'
import { EmptyState } from '@/components/ui/empty-state'
import { PageHeader } from '@/components/ui/page-header'
import { Skeleton } from '@/components/ui/skeleton'
import { timeAgo } from '@/lib/format'
import { useUserDetail } from '@/features/users/use-user-detail'

const sections = [
  { to: '/admin/users/$user', label: 'Sign-in', exact: true },
  { to: '/admin/users/$user/sessions', label: 'Sessions' },
  { to: '/admin/users/$user/tokens', label: 'API tokens' },
  { to: '/admin/users/$user/danger', label: 'Danger zone', others: true },
] as const

export const Route = createFileRoute('/_app/admin/users/$user')({
  head: ({ params }) => ({ meta: [{ title: `${params.user} · Users · bird` }] }),
  component: UserDetail,
})

function UserDetail() {
  const { name, user, users, isSelf } = useUserDetail()
  if (users.isPending) return <Skeleton className="h-48 w-full" />
  if (!user) {
    return (
      <EmptyState
        icon={UserX}
        title={`There is no user ${name}`}
        action={
          <Link to="/admin/users" className="text-sm text-fg-muted underline decoration-line-strong underline-offset-4 hover:text-fg">
            Back to users
          </Link>
        }
      />
    )
  }
  return (
    <div className="flex flex-col gap-8">
      <div className="flex flex-col gap-3">
        <BackLink to="/admin/users">Users</BackLink>
        <PageHeader
          eyebrow="User"
          title={
            <span className="flex flex-wrap items-center gap-3">
              {user.name}
              <Badge tone={user.role === 'admin' ? 'info' : 'neutral'}>{user.role}</Badge>
            </span>
          }
          description={`Added ${timeAgo(user.created_at)}.`}
        />
      </div>
      <SettingsLayout
        label="User"
        nav={sections
          .filter((section) => !('others' in section) || !isSelf)
          .map((section) => (
            <li key={section.to}>
              <Link to={section.to} params={{ user: user.name }} activeOptions={{ exact: 'exact' in section }} className={settingsLinkClass}>
                {section.label}
              </Link>
            </li>
          ))}
      >
        <Outlet />
      </SettingsLayout>
    </div>
  )
}
