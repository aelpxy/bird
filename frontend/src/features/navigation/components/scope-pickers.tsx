import { Menu } from '@base-ui/react/menu'
import { Link } from '@tanstack/react-router'
import { Check, ChevronsUpDown, Plus } from 'lucide-react'
import { useState, type ReactNode } from 'react'
import { eyebrowClass, focusRing } from '@/components/ui/control-styles'
import { menuItemClass, menuPopupClass, menuSeparatorClass } from '@/components/ui/overlay-styles'
import { preferredEnvironment } from '@/features/projects/api'
import { NewEnvironmentDialog } from '@/features/projects/components/new-environment-dialog'
import { NewProjectDialog } from '@/features/projects/components/new-project-dialog'
import type { CurrentScope } from '../use-current-scope'

const itemClass = `${menuItemClass} justify-between gap-6`

// org, project and environment as three menus; the org one only once there is a choice to make
export function ScopePickers({ current }: { current: CurrentScope }) {
  const { orgs, org, projects, project, scope } = current
  const [creatingProject, setCreatingProject] = useState(false)
  const [creatingEnvironment, setCreatingEnvironment] = useState(false)
  const inOrg = projects.filter((candidate) => candidate.org === org)

  return (
    <div className="flex flex-col gap-3">
      {orgs.length > 1 && (
        <Picker label="Org" value={org}>
          {orgs.map((name) => {
            const first = projects.find((candidate) => candidate.org === name)
            const environment = first && preferredEnvironment(first)
            return (
              <Menu.Item
                key={name}
                className={itemClass}
                render={
                  first && environment ? (
                    <Link to="/projects/$project/$environment" params={{ project: first.name, environment }} />
                  ) : (
                    <Link to="/orgs/$org" params={{ org: name }} />
                  )
                }
              >
                {name}
                {name === org && <Check aria-hidden className="size-3.5" />}
              </Menu.Item>
            )
          })}
        </Picker>
      )}
      <Picker label="Project" value={project && project.org === org ? project.name : undefined} placeholder="No projects yet">
        {inOrg.map((candidate) => {
          const environment = preferredEnvironment(candidate)
          return (
            <Menu.Item
              key={candidate.name}
              className={itemClass}
              render={
                environment ? (
                  <Link to="/projects/$project/$environment" params={{ project: candidate.name, environment }} />
                ) : (
                  <Link to="/projects/$project" params={{ project: candidate.name }} />
                )
              }
            >
              {candidate.name}
              {candidate.name === project?.name && <Check aria-hidden className="size-3.5" />}
            </Menu.Item>
          )
        })}
        {inOrg.length > 0 && <Menu.Separator className={menuSeparatorClass} />}
        <Menu.Item className={menuItemClass} onClick={() => setCreatingProject(true)}>
          <Plus aria-hidden className="size-3.5" />
          New project
        </Menu.Item>
      </Picker>
      {project && project.org === org && (
        <Picker label="Environment" value={scope?.environment} placeholder="No environments">
          {project.environments.map((environment) => (
            <Menu.Item
              key={environment}
              className={itemClass}
              render={<Link to="/projects/$project/$environment" params={{ project: project.name, environment }} />}
            >
              {environment}
              {environment === scope?.environment && <Check aria-hidden className="size-3.5" />}
            </Menu.Item>
          ))}
          {project.environments.length > 0 && <Menu.Separator className={menuSeparatorClass} />}
          <Menu.Item className={menuItemClass} onClick={() => setCreatingEnvironment(true)}>
            <Plus aria-hidden className="size-3.5" />
            New environment
          </Menu.Item>
        </Picker>
      )}
      <NewProjectDialog org={org} open={creatingProject} onOpenChange={setCreatingProject} />
      {project && <NewEnvironmentDialog project={project.name} open={creatingEnvironment} onOpenChange={setCreatingEnvironment} />}
    </div>
  )
}

type PickerProps = { label: string; value?: string; placeholder?: string; children: ReactNode }

function Picker({ label, value, placeholder = 'Choose', children }: PickerProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <span aria-hidden className={eyebrowClass}>
        {label}
      </span>
      <Menu.Root>
        <Menu.Trigger
          aria-label={`${label}: ${value ?? placeholder}`}
          className={`flex h-8 w-full items-center justify-between gap-2 border border-line bg-surface px-2.5 text-sm text-fg transition-colors select-none hover:border-line-strong data-popup-open:border-line-strong data-popup-open:bg-raised ${focusRing}`}
        >
          <span className={`truncate ${value ? '' : 'text-fg-faint'}`}>{value ?? placeholder}</span>
          <ChevronsUpDown aria-hidden className="size-3.5 shrink-0 text-fg-faint" />
        </Menu.Trigger>
        <Menu.Portal>
          <Menu.Positioner sideOffset={4} align="start" className="z-50 outline-hidden">
            <Menu.Popup className={`${menuPopupClass} max-h-80 w-[var(--anchor-width)] overflow-y-auto`}>{children}</Menu.Popup>
          </Menu.Positioner>
        </Menu.Portal>
      </Menu.Root>
    </div>
  )
}
