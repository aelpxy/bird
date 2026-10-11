import { Link, Outlet, createFileRoute } from '@tanstack/react-router'
import { TabNav, tabLinkClass } from '@/components/layout/tab-nav'
import { PageHeader } from '@/components/ui/page-header'

const tabs = [
  { to: '/account', label: 'Password', exact: true },
  { to: '/account/two-factor', label: 'Two-factor' },
  { to: '/account/sessions', label: 'Sessions' },
  { to: '/account/tokens', label: 'API tokens' },
] as const

export const Route = createFileRoute('/_app/account')({
  head: () => ({ meta: [{ title: 'Account · bird' }] }),
  component: AccountLayout,
})

function AccountLayout() {
  const { session } = Route.useRouteContext()
  return (
    <div className="flex flex-col gap-6">
      <PageHeader eyebrow={session.role} title="Account" description={`Signed in as ${session.name}.`} />
      <TabNav label="Account sections">
        {tabs.map((tab) => (
          <li key={tab.to}>
            <Link to={tab.to} activeOptions={{ exact: 'exact' in tab }} className={tabLinkClass}>
              {tab.label}
            </Link>
          </li>
        ))}
      </TabNav>
      <Outlet />
    </div>
  )
}
