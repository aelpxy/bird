import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { Building2, LayoutGrid, List, PackageOpen, Plus, Search, Settings, Users } from 'lucide-react'
import type { ReactNode } from 'react'
import { Logo } from '@/components/layout/logo'
import { eyebrowClass, focusRing } from '@/components/ui/control-styles'
import { Skeleton } from '@/components/ui/skeleton'
import type { Scope } from '@/lib/api/paths'
import type { Session } from '@/features/auth/api'
import { UserMenu } from '@/features/auth/components/user-menu'
import { servicesQuery } from '@/features/services/api'
import { NewServiceDialog } from '@/features/services/components/new-service-dialog'
import { serviceStatus, toneDots } from '@/features/services/status'
import { useCurrentScope } from '../use-current-scope'
import { ScopePickers } from './scope-pickers'

const rowClass = `flex h-8 items-center gap-2.5 px-2.5 text-sm text-fg-muted transition-colors hover:bg-raised hover:text-fg data-[status=active]:bg-raised data-[status=active]:text-fg ${focusRing}`

export function Sidebar({ session, onSearch }: { session: Session; onSearch: () => void }) {
  const current = useCurrentScope()
  const { scope } = current
  const admin = session.role === 'admin'
  return (
    <div className="flex h-full flex-col">
      <div className="flex h-12 shrink-0 items-center justify-between gap-2 border-b border-line px-4">
        <Link to="/" aria-label="bird, overview" className={focusRing}>
          <Logo />
        </Link>
        <button
          type="button"
          onClick={onSearch}
          aria-label="Go to a service, project or page"
          aria-keyshortcuts="Meta+K Control+K"
          className={`pressable flex h-7 items-center gap-2 border border-line px-2 text-fg-faint transition-colors hover:border-line-strong hover:text-fg ${focusRing}`}
        >
          <Search aria-hidden className="size-3.5" />
          <kbd className="font-mono text-2xs">⌘K</kbd>
        </button>
      </div>

      <nav aria-label="Main" className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-3 py-4">
        <Link to="/" activeOptions={{ exact: true }} className={rowClass}>
          <LayoutGrid aria-hidden className="size-4" />
          Overview
        </Link>

        <ScopePickers current={current} />

        {scope && (
          <>
            <Group label="Services">
              <ServiceLinks scope={scope} />
              <NewServiceDialog
                scope={scope}
                trigger={
                  <button type="button" className={`${rowClass} w-full text-fg-faint`}>
                    <Plus aria-hidden className="size-4" />
                    New service
                  </button>
                }
              />
            </Group>
            <Group label={scope.project}>
              <Link to="/projects/$project/$environment" params={scope} activeOptions={{ exact: true }} className={rowClass}>
                <List aria-hidden className="size-4" />
                All services
              </Link>
              <Link to="/projects/$project/-/settings" params={{ project: scope.project }} className={rowClass}>
                <Settings aria-hidden className="size-4" />
                Project settings
              </Link>
            </Group>
          </>
        )}
      </nav>

      <div className="flex shrink-0 flex-col gap-0.5 border-t border-line px-3 py-3">
        <Link to="/orgs" className={rowClass}>
          <Building2 aria-hidden className="size-4" />
          Orgs
        </Link>
        {admin && (
          <>
            <Link to="/registries" className={rowClass}>
              <PackageOpen aria-hidden className="size-4" />
              Registries
            </Link>
            <Link to="/admin/users" className={rowClass}>
              <Users aria-hidden className="size-4" />
              Users
            </Link>
          </>
        )}
        <div className="mt-2">
          <UserMenu session={session} />
        </div>
      </div>
    </div>
  )
}

function Group({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className={`mb-1 truncate px-2.5 ${eyebrowClass}`}>{label}</span>
      {children}
    </div>
  )
}

function ServiceLinks({ scope }: { scope: Scope }) {
  const services = useQuery(servicesQuery(scope))
  if (services.isPending) return <Skeleton className="mx-2.5 h-4 w-24" />
  if (services.isError) return <p className="px-2.5 text-xs text-fg-faint">Couldn&apos;t load services.</p>
  return (
    <>
      {services.data.map((service) => {
        const status = serviceStatus(service)
        return (
          <Link key={service.name} to="/projects/$project/$environment/services/$service" params={{ ...scope, service: service.name }} className={rowClass}>
            <span aria-hidden className={`size-2 shrink-0 ${toneDots[status.tone]} ${status.busy ? 'animate-pulse' : ''}`} />
            <span className="truncate">{service.name}</span>
            <span className="sr-only">, {status.label}</span>
          </Link>
        )
      })}
    </>
  )
}
