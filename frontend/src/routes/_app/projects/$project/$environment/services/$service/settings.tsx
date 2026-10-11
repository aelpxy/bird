import { Link, Outlet, createFileRoute } from '@tanstack/react-router'
import { SettingsLayout, settingsLinkClass } from '@/components/layout/settings-layout'
import { useServiceScope } from '@/features/services/use-service-scope'

const sections = [
  { to: '/projects/$project/$environment/services/$service/settings', label: 'Deployment', exact: true },
  { to: '/projects/$project/$environment/services/$service/settings/domains', label: 'Domains' },
  { to: '/projects/$project/$environment/services/$service/settings/volumes', label: 'Volumes' },
  { to: '/projects/$project/$environment/services/$service/settings/scaling', label: 'Scaling' },
  { to: '/projects/$project/$environment/services/$service/settings/danger', label: 'Danger zone' },
] as const

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/settings')({
  component: SettingsSections,
})

function SettingsSections() {
  const { scope, service } = useServiceScope()
  return (
    <SettingsLayout
      label="Settings"
      nav={sections.map((section) => (
        <li key={section.to}>
          <Link to={section.to} params={{ ...scope, service }} activeOptions={{ exact: 'exact' in section }} className={settingsLinkClass}>
            {section.label}
          </Link>
        </li>
      ))}
    >
      <Outlet />
    </SettingsLayout>
  )
}
