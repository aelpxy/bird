import { useMutation, useQueryClient } from '@tanstack/react-query'
import { z } from 'zod'
import { useAppForm } from '@/components/form/app-form'
import { focusRing, labelClass } from '@/components/ui/control-styles'
import { ErrorAlert } from '@/components/ui/error-alert'
import type { Schemas } from '@/lib/api/client'
import type { Scope } from '@/lib/api/paths'
import { notifySuccess } from '@/lib/toast'
import { command, image, port, wholeNumber } from '@/lib/validation'
import { deploy, type Service } from '../api'

type HealthKind = 'http' | 'tcp' | 'path'
type CommandMode = 'keep' | 'set' | 'clear'

const commandModes = z.enum(['keep', 'set', 'clear'])

const schema = z
  .object({
    image,
    port,
    commandMode: commandModes,
    command: z.string(),
    releaseMode: commandModes,
    release: z.string(),
    health: z.enum(['http', 'tcp', 'path']),
    healthPath: z.string(),
    healthTimeout: wholeNumber(1, 3600, 'the timeout'),
    memory: wholeNumber(32, 262_144, 'the memory limit'),
    cpus: z.number({ error: 'Enter the CPU limit.' }).min(0.1, 'CPU goes from 0.1 to 64 cores.').max(64, 'CPU goes from 0.1 to 64 cores.'),
    allowImageChange: z.boolean(),
  })
  // only the parts being changed need a value
  .superRefine((values, context) => {
    const checks: [boolean, string, z.ZodType, unknown][] = [
      [values.commandMode === 'set', 'command', command, values.command],
      [values.releaseMode === 'set', 'release', command, values.release],
      [values.health === 'path', 'healthPath', z.string().regex(/^\/\S*$/, 'Start the path with /.'), values.healthPath],
    ]
    for (const [applies, path, rule, value] of checks) {
      if (!applies) continue
      for (const issue of rule.safeParse(value).error?.issues ?? []) context.addIssue({ code: 'custom', message: issue.message, path: [path] })
    }
  })

function quote(args: string[] | null | undefined): string {
  return (args ?? []).map((arg) => (/[\s'"\\]/.test(arg) || arg === '' ? JSON.stringify(arg) : arg)).join(' ')
}

function healthKind(health: string): HealthKind {
  return health === 'http' || health === 'tcp' ? health : 'path'
}

function request(service: Service, values: z.output<typeof schema>): Schemas['DeployRequest'] {
  const body: Schemas['DeployRequest'] = {
    name: service.name,
    image: values.image,
    port: values.port,
    health: values.health === 'path' ? values.healthPath.trim() : values.health,
    health_timeout: values.healthTimeout,
    memory: values.memory,
    cpus: Math.round(values.cpus * 1000),
    allow_image_change: values.allowImageChange,
  }
  if (values.commandMode === 'set') body.command = command.parse(values.command)
  if (values.commandMode === 'clear') body.default_command = true
  if (values.releaseMode === 'set') body.release = command.parse(values.release)
  if (values.releaseMode === 'clear') body.no_release = true
  return body
}

export function ServiceConfigForm({ scope, service }: { scope: Scope; service: Service }) {
  const queryClient = useQueryClient()
  const mutation = useMutation({
    mutationFn: (values: z.output<typeof schema>) => deploy(scope, request(service, values)),
    onSuccess: () => {
      notifySuccess(`Redeployed ${service.name}`, 'Traffic moved once the new machines were healthy.')
      return queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment, service.name] })
    },
  })
  const form = useAppForm({
    defaultValues: {
      image: service.image,
      port: service.port,
      commandMode: 'keep' as CommandMode,
      command: quote(service.command),
      releaseMode: 'keep' as CommandMode,
      release: quote(service.release),
      health: healthKind(service.health),
      healthPath: healthKind(service.health) === 'path' ? service.health : '/healthz',
      healthTimeout: service.health_timeout,
      memory: service.memory,
      cpus: service.cpus / 1000,
      allowImageChange: false,
    },
    validators: { onChange: schema },
    onSubmit: ({ value }) => mutation.mutateAsync(schema.parse(value)).catch(() => undefined),
  })

  return (
    <form.AppForm>
      <form.FormElement className="flex flex-col gap-5">
        {mutation.isError && <ErrorAlert error={mutation.error} />}
        <div className="grid gap-4 sm:grid-cols-[1fr_7rem]">
          <form.AppField name="image">{(field) => <field.TextField label="Image" mono autoComplete="off" spellCheck={false} />}</form.AppField>
          <form.AppField name="port">{(field) => <field.NumberField label="Port" min={1} max={65535} />}</form.AppField>
        </div>

        {(
          [
            { mode: 'commandMode', value: 'command', label: 'Command', current: service.command, empty: "The image's own command", placeholder: 'bin/server --workers 4', hint: undefined },
            {
              mode: 'releaseMode',
              value: 'release',
              label: 'Release command',
              current: service.release,
              empty: 'None',
              placeholder: 'bin/migrate',
              hint: 'Runs once per deploy, before new machines start, like database migrations. If it fails, the deploy stops.',
            },
          ] as const
        ).map((setting) => (
          <form.AppField key={setting.mode} name={setting.mode}>
            {(modeField) => (
              <div className="flex flex-col gap-2">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <span className={labelClass}>{setting.label}</span>
                  <div className="flex gap-1">
                    {modeField.state.value !== 'set' && <ModeButton onClick={() => modeField.handleChange('set')}>{setting.current ? 'Change' : 'Set'}</ModeButton>}
                    {setting.current && modeField.state.value !== 'clear' && <ModeButton onClick={() => modeField.handleChange('clear')}>Remove</ModeButton>}
                    {modeField.state.value !== 'keep' && <ModeButton onClick={() => modeField.handleChange('keep')}>Undo</ModeButton>}
                  </div>
                </div>
                {modeField.state.value === 'set' ? (
                  <form.AppField name={setting.value}>
                    {(field) => (
                      <field.TextField
                        label={`New ${setting.label.toLowerCase()}`}
                        hideLabel
                        placeholder={setting.placeholder}
                        description="Quote arguments with spaces. Nothing is expanded by a shell."
                        mono
                        autoFocus
                        autoComplete="off"
                        spellCheck={false}
                      />
                    )}
                  </form.AppField>
                ) : (
                  <p className={`font-mono text-ui ${modeField.state.value === 'clear' ? 'text-fg-faint line-through' : 'text-fg'}`}>
                    {setting.current ? quote(setting.current) : <span className="font-sans text-fg-faint">{setting.empty}</span>}
                  </p>
                )}
                {modeField.state.value === 'clear' && (
                  <p className="text-xs text-warn">Removed on save; {setting.mode === 'commandMode' ? "the image's own command runs" : 'deploys run without one'}.</p>
                )}
                {setting.hint && modeField.state.value !== 'clear' && <p className="text-xs text-fg-faint">{setting.hint}</p>}
              </div>
            )}
          </form.AppField>
        ))}

        <div className="grid gap-4 sm:grid-cols-[1fr_10rem]">
          <div className="flex flex-col gap-2">
            <form.AppField name="health">
              {(field) => (
                <field.SelectField
                  label="Health check"
                  options={[
                    { value: 'http', label: 'Any HTTP answer on /' },
                    { value: 'path', label: 'A path that answers 2xx' },
                    { value: 'tcp', label: 'Accepts TCP connections' },
                  ]}
                  description="New machines must pass it before they get traffic. Use TCP for databases."
                />
              )}
            </form.AppField>
            <form.Subscribe selector={(state) => state.values.health}>
              {(health) =>
                health === 'path' && <form.AppField name="healthPath">{(field) => <field.TextField label="Path" mono autoComplete="off" spellCheck={false} />}</form.AppField>
              }
            </form.Subscribe>
          </div>
          <form.AppField name="healthTimeout">{(field) => <field.NumberField label="Timeout (seconds)" min={1} max={3600} />}</form.AppField>
        </div>

        <div className="grid gap-4 sm:grid-cols-2">
          <form.AppField name="memory">{(field) => <field.NumberField label="Memory per machine (MB)" min={32} max={262144} />}</form.AppField>
          <form.AppField name="cpus">{(field) => <field.NumberField label="CPU per machine (cores)" inputMode="decimal" step={0.1} min={0.1} max={64} />}</form.AppField>
        </div>

        {service.volumes.length > 0 && (
          <form.AppField name="allowImageChange">
            {(field) => (
              <field.CheckboxField
                label="Allow a different image to use the volumes"
                description="birdd refuses an image other than the one that wrote the data, so a major upgrade cannot corrupt it by accident."
              />
            )}
          </form.AppField>
        )}

        <form.Subscribe selector={(state) => state.isSubmitting}>
          {(submitting) =>
            submitting && (
              <p role="status" className="text-sm text-fg-muted">
                Deploying: pulling the image, running the release command and waiting for health checks.
              </p>
            )
          }
        </form.Subscribe>
        <div>
          <form.SubmitButton>Save and redeploy</form.SubmitButton>
        </div>
      </form.FormElement>
    </form.AppForm>
  )
}

function ModeButton({ onClick, children }: { onClick: () => void; children: string }) {
  return (
    <button type="button" onClick={onClick} className={`px-1.5 py-0.5 font-mono text-2xs tracking-[0.08em] text-fg-faint uppercase hover:bg-raised hover:text-fg ${focusRing}`}>
      {children}
    </button>
  )
}
