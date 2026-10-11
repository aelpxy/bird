import { Link } from '@tanstack/react-router'
import { TabNav, tabLinkClass } from '@/components/layout/tab-nav'
import type { Scope } from '@/lib/api/paths'

const tabs = [
  { to: '/projects/$project/$environment/services/$service', label: 'Overview', exact: true },
  { to: '/projects/$project/$environment/services/$service/deployments', label: 'Deployments' },
  { to: '/projects/$project/$environment/services/$service/logs', label: 'Logs' },
  { to: '/projects/$project/$environment/services/$service/variables', label: 'Variables' },
  { to: '/projects/$project/$environment/services/$service/terminal', label: 'Terminal' },
  { to: '/projects/$project/$environment/services/$service/cron', label: 'Cron' },
  { to: '/projects/$project/$environment/services/$service/backups', label: 'Backups', needsVolumes: true },
  { to: '/projects/$project/$environment/services/$service/settings', label: 'Settings' },
] as const

export function ServiceTabs({ scope, service, hasVolumes }: { scope: Scope; service: string; hasVolumes: boolean }) {
  return (
    <TabNav label="Service sections">
      {tabs.filter((tab) => hasVolumes || !('needsVolumes' in tab)).map((tab) => (
        <li key={tab.to}>
          <Link to={tab.to} params={{ ...scope, service }} activeOptions={{ exact: 'exact' in tab }} className={tabLinkClass}>
            {tab.label}
          </Link>
        </li>
      ))}
    </TabNav>
  )
}
