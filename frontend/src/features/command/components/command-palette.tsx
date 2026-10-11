import { Dialog } from '@base-ui/react/dialog'
import { useNavigate } from '@tanstack/react-router'
import { Search } from 'lucide-react'
import { useEffect, useMemo, useState, type KeyboardEvent } from 'react'
import { backdropClass, palettePopupClass } from '@/components/ui/overlay-styles'
import type { Session } from '@/features/auth/api'
import { useOverview } from '@/features/overview/use-overview'
import { filter, type Item } from '../filter'

// opened with ⌘K or Ctrl+K anywhere, or the search button in the header
export function CommandPalette({ session, open, onOpenChange }: { session: Session; open: boolean; onOpenChange: (open: boolean) => void }) {
  useEffect(() => {
    function onKey(event: globalThis.KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        onOpenChange(!open)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open, onOpenChange])

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Backdrop className={backdropClass} />
        <Dialog.Popup aria-label="Go to" className={palettePopupClass}>
          {open && <Results session={session} onDone={() => onOpenChange(false)} />}
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  )
}

function Results({ session, onDone }: { session: Session; onDone: () => void }) {
  const items = useItems(session)
  const [query, setQuery] = useState('')
  const [active, setActive] = useState(0)
  const matches = useMemo(() => filter(items, query), [items, query])
  const selected = Math.min(active, Math.max(0, matches.length - 1))

  function choose(item: Item | undefined) {
    if (!item) return
    onDone()
    void item.go()
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      const step = event.key === 'ArrowDown' ? 1 : -1
      setActive((selected + step + matches.length) % Math.max(1, matches.length))
    } else if (event.key === 'Enter') {
      event.preventDefault()
      choose(matches[selected])
    }
  }

  return (
    <>
      <div className="flex items-center gap-3 border-b border-line px-4">
        <Search aria-hidden className="size-4 shrink-0 text-fg-faint" />
        <input
          autoFocus
          role="combobox"
          aria-expanded
          aria-controls="command-results"
          aria-activedescendant={matches[selected] ? `command-${matches[selected].id}` : undefined}
          aria-label="Go to a service, project or page"
          placeholder="Go to a service, project or page"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value)
            setActive(0)
          }}
          onKeyDown={onKeyDown}
          className="h-12 w-full bg-transparent text-sm text-fg outline-none placeholder:text-fg-faint"
        />
      </div>
      <ul id="command-results" role="listbox" aria-label="Results" className="overflow-y-auto py-1">
        {matches.length === 0 && <li className="px-4 py-6 text-center text-sm text-fg-faint">Nothing matches.</li>}
        {matches.map((item, index) => (
          <li
            key={item.id}
            id={`command-${item.id}`}
            role="option"
            aria-selected={index === selected}
            onMouseMove={() => setActive(index)}
            onClick={() => choose(item)}
            className="flex cursor-default items-center justify-between gap-4 px-4 py-2 text-sm text-fg-muted aria-selected:bg-raised aria-selected:text-fg"
          >
            <span className="truncate">{item.label}</span>
            <span className="shrink-0 font-mono text-2xs text-fg-faint">{item.detail}</span>
          </li>
        ))}
      </ul>
    </>
  )
}

function useItems(session: Session): Item[] {
  const navigate = useNavigate()
  const overview = useOverview()
  return useMemo(() => {
    const pages: Item[] = [
      { id: 'page-projects', label: 'Projects', detail: 'page', go: () => navigate({ to: '/' }) },
      { id: 'page-orgs', label: 'Orgs', detail: 'page', go: () => navigate({ to: '/orgs' }) },
      { id: 'page-account', label: 'Account', detail: 'page', go: () => navigate({ to: '/account' }) },
      { id: 'page-tokens', label: 'API tokens', detail: 'account', go: () => navigate({ to: '/account/tokens' }) },
    ]
    if (session.role === 'admin') {
      pages.push(
        { id: 'page-registries', label: 'Registries', detail: 'page', go: () => navigate({ to: '/registries' }) },
        { id: 'page-users', label: 'Users', detail: 'page', go: () => navigate({ to: '/admin/users' }) },
      )
    }
    const services: Item[] = overview.services.map(({ scope, service }) => ({
      id: `service-${scope.project}-${scope.environment}-${service.name}`,
      label: service.name,
      detail: `${scope.project} / ${scope.environment}`,
      go: () => navigate({ to: '/projects/$project/$environment/services/$service', params: { ...scope, service: service.name } }),
    }))
    const environments: Item[] = overview.projects.flatMap((project) =>
      project.environments.map((environment) => ({
        id: `environment-${project.name}-${environment}`,
        label: `${project.name} / ${environment}`,
        detail: 'environment',
        go: () => navigate({ to: '/projects/$project/$environment', params: { project: project.name, environment } }),
      })),
    )
    return [...services, ...environments, ...pages]
  }, [navigate, overview.services, overview.projects, session.role])
}
