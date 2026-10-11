import { Menu } from '@base-ui/react/menu'
import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { Check, ChevronsUpDown, Plus, Settings } from 'lucide-react'
import { useState, type ReactNode } from 'react'
import { focusRing } from '@/components/ui/control-styles'
import { menuItemClass, menuPopupClass, menuSeparatorClass } from '@/components/ui/overlay-styles'
import type { Scope } from '@/lib/api/paths'
import { preferredEnvironment, projectsQuery } from '@/features/projects/api'
import { NewEnvironmentDialog } from './new-environment-dialog'

const itemClass = `${menuItemClass} justify-between gap-6`

// projects / project / environment, each a menu to jump to a sibling
export function ScopeSwitcher({ scope }: { scope: Scope }) {
  const { data: projects = [] } = useQuery(projectsQuery)
  const current = projects.find((project) => project.name === scope.project)
  const [creating, setCreating] = useState(false)

  return (
    <nav aria-label="Project and environment" className="flex flex-wrap items-center gap-1 border-b border-line pb-3">
      <Link to="/" className={`px-1.5 py-1 font-mono text-2xs tracking-[0.08em] text-fg-faint uppercase transition-colors hover:text-fg ${focusRing}`}>
        Projects
      </Link>
      <Slash />
      <Picker kind="Project" value={scope.project}>
        {projects.map((project) => {
          const environment = preferredEnvironment(project)
          return (
            <Menu.Item
              key={project.name}
              className={itemClass}
              render={
                environment ? (
                  <Link to="/projects/$project/$environment" params={{ project: project.name, environment }} />
                ) : (
                  <Link to="/projects/$project" params={{ project: project.name }} />
                )
              }
            >
              {project.name}
              {project.name === scope.project && <Check aria-hidden className="size-3.5" />}
            </Menu.Item>
          )
        })}
        <Menu.Separator className={menuSeparatorClass} />
        <Menu.Item className={menuItemClass} render={<Link to="/projects/$project/-/settings" params={{ project: scope.project }} />}>
          <Settings aria-hidden className="size-3.5" />
          Project settings
        </Menu.Item>
      </Picker>
      <Slash />
      <Picker kind="Environment" value={scope.environment}>
        {(current?.environments ?? [scope.environment]).map((environment) => (
          <Menu.Item
            key={environment}
            className={itemClass}
            render={<Link to="/projects/$project/$environment" params={{ project: scope.project, environment }} />}
          >
            {environment}
            {environment === scope.environment && <Check aria-hidden className="size-3.5" />}
          </Menu.Item>
        ))}
        <Menu.Separator className={menuSeparatorClass} />
        <Menu.Item className={menuItemClass} onClick={() => setCreating(true)}>
          <Plus aria-hidden className="size-3.5" />
          New environment
        </Menu.Item>
      </Picker>
      <NewEnvironmentDialog project={scope.project} open={creating} onOpenChange={setCreating} />
    </nav>
  )
}

function Slash() {
  return (
    <span aria-hidden className="font-mono text-fg-faint/60">
      /
    </span>
  )
}

function Picker({ kind, value, children }: { kind: string; value: string; children: ReactNode }) {
  return (
    <Menu.Root>
      <Menu.Trigger
        aria-label={`${kind} ${value}`}
        className={`flex h-7 items-center gap-1.5 border border-transparent px-2 text-sm font-medium text-fg transition-colors select-none hover:border-line hover:bg-surface data-popup-open:border-line data-popup-open:bg-raised ${focusRing}`}
      >
        <span className="max-w-48 truncate">{value}</span>
        <ChevronsUpDown aria-hidden className="size-3.5 text-fg-faint" />
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Positioner sideOffset={6} align="start" className="z-30 outline-hidden">
          <Menu.Popup className={`${menuPopupClass} max-h-80 overflow-y-auto`}>{children}</Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  )
}
