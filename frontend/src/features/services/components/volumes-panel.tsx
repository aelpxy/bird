import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { HardDrive, Plus } from 'lucide-react'
import { z } from 'zod'
import { useAppForm } from '@/components/form/app-form'
import { focusRing } from '@/components/ui/control-styles'
import { ErrorAlert } from '@/components/ui/error-alert'
import { Eyebrow } from '@/components/ui/eyebrow'
import { Panel } from '@/components/ui/panel'
import type { Scope } from '@/lib/api/paths'
import { interval } from '@/lib/format'
import { notifySuccess } from '@/lib/toast'
import { volume } from '@/lib/validation'
import { deploy, type Service } from '../api'

const schema = volume.extend({ allowImageChange: z.boolean() })

export function VolumesPanel({ scope, service }: { scope: Scope; service: Service }) {
  const queryClient = useQueryClient()
  const add = useMutation({
    mutationFn: (values: z.output<typeof schema>) =>
      deploy(scope, {
        name: service.name,
        image: service.image,
        port: service.port,
        volumes: [{ name: values.name, path: values.path }],
        allow_image_change: values.allowImageChange,
      }),
    onSuccess: (_result, values) => {
      notifySuccess(`Added ${values.name}`, `Mounted at ${values.path}; ${service.name} was redeployed with it.`)
      form.reset()
      return queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment, service.name] })
    },
  })
  const form = useAppForm({
    defaultValues: { name: service.volumes.length === 0 ? 'data' : '', path: '', allowImageChange: false },
    validators: { onChange: schema },
    onSubmit: ({ value }) => add.mutateAsync(schema.parse(value)).catch(() => undefined),
  })
  const tooMany = service.replicas > 1
  const backups = service.backup_schedule

  return (
    <div className="flex flex-col gap-6">
      {service.volumes.length > 0 ? (
        <ul className="divide-y divide-line border border-line bg-surface">
          {service.volumes.map((entry) => (
            <li key={entry.name} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
              <span className="grid size-9 shrink-0 place-items-center border border-line-strong bg-sunken">
                <HardDrive aria-hidden className="size-4 text-fg-muted" />
              </span>
              <div className="min-w-0 flex-1">
                <p className="font-mono text-ui font-medium text-fg">{entry.name}</p>
                <p className="mt-0.5 font-mono text-2xs text-fg-faint">mounted at {entry.path}</p>
              </div>
              <Link
                to="/projects/$project/$environment/services/$service/backups"
                params={{ ...scope, service: service.name }}
                className={`text-xs underline decoration-line-strong underline-offset-4 hover:text-fg ${backups ? 'text-fg-muted' : 'text-warn'} ${focusRing}`}
              >
                {backups ? `Backed up every ${interval(backups.every)}` : 'Not backed up'}
              </Link>
            </li>
          ))}
        </ul>
      ) : (
        <p className="border border-dashed border-line-strong px-4 py-5 text-sm text-fg-muted">
          No volumes: everything {service.name} writes is lost when its machines are replaced, on every deploy.
        </p>
      )}

      <Panel className="flex flex-col gap-4">
        <Eyebrow>Add a volume</Eyebrow>
        {tooMany ? (
          <p className="text-sm text-fg-muted">
            Only one machine may write a volume, and {service.name} runs {service.replicas}.{' '}
            <Link to="/projects/$project/$environment/services/$service/settings/scaling" params={{ ...scope, service: service.name }} className="text-fg underline underline-offset-4">
              Scale it to one
            </Link>{' '}
            first.
          </p>
        ) : (
          <form.AppForm>
            <form.FormElement className="flex flex-col gap-4">
              {add.isError && <ErrorAlert error={add.error} />}
              <div className="grid gap-4 sm:grid-cols-[11rem_1fr]">
                <form.AppField name="name">{(field) => <field.TextField label="Name" placeholder="data" mono autoComplete="off" autoCapitalize="none" spellCheck={false} />}</form.AppField>
                <form.AppField name="path">
                  {(field) => <field.TextField label="Mounted at" placeholder="/var/lib/app" description="Where the app reads and writes its data." mono autoComplete="off" spellCheck={false} />}
                </form.AppField>
              </div>
              {service.volumes.length > 0 && (
                <form.AppField name="allowImageChange">
                  {(field) => (
                    <field.CheckboxField
                      label="The image changed since the data was written"
                      description="birdd refuses another image on existing volumes unless you allow it, so a major upgrade cannot corrupt data by accident."
                    />
                  )}
                </form.AppField>
              )}
              <div className="flex flex-wrap items-center gap-3">
                <form.SubmitButton variant="secondary">
                  <Plus aria-hidden className="size-3.5" />
                  Add and redeploy
                </form.SubmitButton>
                <p className="text-xs text-fg-faint">A volume keeps its path for good. To move data, add a new one.</p>
              </div>
            </form.FormElement>
          </form.AppForm>
        )}
      </Panel>
    </div>
  )
}
