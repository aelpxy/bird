import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { ArrowLeft, Plus, X } from 'lucide-react'
import { useState } from 'react'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { Button } from '@/components/ui/button'
import { ChoiceCard, ChoiceGroup } from '@/components/ui/choice-card'
import { eyebrowClass } from '@/components/ui/control-styles'
import { FormDialog } from '@/components/ui/form-dialog'
import { IconButton } from '@/components/ui/icon-button'
import type { Scope } from '@/lib/api/paths'
import { notifySuccess } from '@/lib/toast'
import { NAME_HINT, optionalHostname } from '@/lib/validation'
import { buildArgs } from '@/features/builds/schemas'
import { BuildProgress } from '@/features/builds/components/build-progress'
import { LiveDeployLog } from '@/features/deployments/components/live-deploy-panel'
import { FolderPicker } from '@/features/builds/components/folder-picker'
import { useSourceDeploy } from '@/features/builds/use-source-deploy'
import { domainUrl } from '@/features/domains/api'
import { deployTemplate, templatesQuery } from '@/features/templates/api'
import { deploy } from '../api'
import { nameFromImage } from '../new-service/image-name'
import { defaults, MAX_REPLICAS, schema, variables, type Source, type Values } from '../new-service/schema'
import { DeployStatus } from './new-service/deploy-status'
import { SourcePicker } from './new-service/source-picker'

const titles: Record<Source, string> = { image: 'Deploy an image', source: 'Deploy from a folder', template: 'Add from a template' }

export function NewServiceDialog({ scope }: { scope: Scope }) {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const templates = useQuery(templatesQuery)
  const source = useSourceDeploy(scope)
  const [step, setStep] = useState<'pick' | 'details'>('pick')
  // the name follows the image until someone types their own
  const [nameTouched, setNameTouched] = useState(false)

  async function submit(values: Values) {
    let service: string
    if (values.source === 'template') {
      const result = await deployTemplate(scope, values.template, values.templateName.trim())
      service = result.deployment.service
      notifySuccess(`Added ${service}`, result.connection ? `Other services connect with \${{${service}.${result.connection}}}.` : undefined)
    } else {
      const domain = optionalHostname.parse(values.domain)
      const settings = {
        name: values.name.trim(),
        port: values.port,
        domains: domain ? [domain] : [],
        env: variables.parse(values.env),
        replicas: values.replicas,
        health: values.health,
        volumes: values.volumes.map((entry) => ({ name: entry.name.trim(), path: entry.path.trim() })),
      }
      const deployed =
        values.source === 'image'
          ? await deploy(scope, { ...settings, image: values.image.trim() })
          : await source.buildAndDeploy(settings.name, { dockerfile: values.dockerfile, args: buildArgs.parse(values.args) }, (image) =>
              deploy(scope, { ...settings, image }),
            )
      service = deployed.service
      notifySuccess(`Deployed ${service}`)
    }
    await queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment] })
    await navigate({ to: '/projects/$project/$environment/services/$service', params: { ...scope, service } })
  }

  const dialog = useDialogForm({
    schema,
    defaultValues: defaults,
    onSubmit: submit,
    onOpenChange: (open) => {
      if (!open) {
        setStep('pick')
        setNameTouched(false)
      }
    },
  })
  const { form } = dialog

  return (
    <form.Subscribe selector={(state) => state.values.source}>
      {(chosen) => (
        <FormDialog
          dialog={dialog}
          trigger={
            <Button>
              <Plus aria-hidden className="size-3.5" />
              New service
            </Button>
          }
          size="lg"
          title={step === 'pick' ? 'New service' : titles[chosen]}
          description={`Into ${scope.project} / ${scope.environment}.`}
          submitLabel={chosen === 'template' ? 'Add' : chosen === 'source' ? 'Build and deploy' : 'Deploy'}
          hideSubmit={step === 'pick'}
          footerStart={
            step === 'details' && !dialog.pending ? (
              <Button type="button" variant="ghost" size="sm" className="-ml-2.5" onClick={() => setStep('pick')}>
                <ArrowLeft aria-hidden className="size-3.5" />
                Back
              </Button>
            ) : undefined
          }
        >
          {step === 'pick' ? (
            <SourcePicker
              onPick={(picked) => {
                form.setFieldValue('source', picked)
                setStep('details')
              }}
            />
          ) : (
            renderDetails()
          )}
        </FormDialog>
      )}
    </form.Subscribe>
  )

  // render functions, not components: a component declared in here would remount its fields on every keystroke
  function renderDetails() {
    if (dialog.pending) {
      const { source: chosen, name } = form.state.values
      if (chosen === 'template') return <DeployStatus label="Pulling the image and waiting for it to pass its health check…" />
      return (
        <div className="flex flex-col gap-4">
          {source.progress && <BuildProgress progress={source.progress} />}
          {(chosen === 'image' || source.progress?.phase === 'deploying') && <LiveDeployLog scope={scope} service={name.trim()} />}
        </div>
      )
    }
    return (
      <form.Subscribe selector={(state) => state.values.source}>
        {(chosen) =>
          chosen === 'template' ? (
            <>
              <form.AppField name="template">
                {(field) => (
                  <ChoiceGroup legend="Template" columns="stack">
                    {(templates.data ?? []).map((option) => (
                      <ChoiceCard
                        key={option.name}
                        name="template"
                        label={option.name}
                        description={
                          <>
                            {option.description}
                            <span className="mt-1 block font-mono text-2xs text-fg-faint">{option.image}</span>
                          </>
                        }
                        checked={field.state.value === option.name}
                        onSelect={() => field.handleChange(option.name)}
                      />
                    ))}
                    {field.state.meta.errors.length > 0 && form.state.submissionAttempts > 0 && <p className="text-xs text-bad">Pick a template.</p>}
                  </ChoiceGroup>
                )}
              </form.AppField>
              <form.AppField name="templateName">
                {(field) => <field.TextField label="Service name (optional)" description={`Defaults to the template's name. ${NAME_HINT}`} autoComplete="off" autoCapitalize="none" spellCheck={false} />}
              </form.AppField>
            </>
          ) : (
            <>
              {chosen === 'image' ? (
                <form.AppField
                  name="image"
                  listeners={{ onChange: ({ value }) => !nameTouched && form.setFieldValue('name', nameFromImage(value)) }}
                >
                  {(field) => (
                    <field.TextField label="Image" placeholder="ghcr.io/acme/web:1.4" description="Private registries need credentials, added under Registries." mono autoFocus autoComplete="off" autoCapitalize="none" spellCheck={false} />
                  )}
                </form.AppField>
              ) : (
                <form.Subscribe selector={(state) => state.values.dockerfile}>
                  {(dockerfile) => <FolderPicker folder={source.folder} onFolder={source.setFolder} dockerfile={dockerfile} />}
                </form.Subscribe>
              )}
              <div className="grid gap-4 sm:grid-cols-[1fr_7rem]">
                <form.AppField name="name" listeners={{ onChange: () => setNameTouched(true) }}>
                  {(field) => <field.TextField label="Service name" placeholder="web" description={NAME_HINT} autoComplete="off" autoCapitalize="none" spellCheck={false} />}
                </form.AppField>
                <form.AppField name="port">{(field) => <field.NumberField label="Port" min={1} max={65535} />}</form.AppField>
              </div>
              <form.AppField name="domain">
                {(field) => (
                  <field.TextField
                    label="Domain (optional)"
                    placeholder="app.example.com"
                    description={field.state.value.trim() ? `Served at ${domainUrl(field.state.value.trim().toLowerCase())}` : 'Leave empty to keep it private to this environment.'}
                    autoComplete="off"
                    autoCapitalize="none"
                    spellCheck={false}
                  />
                )}
              </form.AppField>
              {chosen === 'source' && (
                <form.AppField name="dockerfile">{(field) => <field.TextField label="Dockerfile" mono autoComplete="off" spellCheck={false} />}</form.AppField>
              )}
              {renderAdvanced(chosen)}
            </>
          )
        }
      </form.Subscribe>
    )
  }

  function renderAdvanced(chosen: Source) {
    return (
      <details className="group border border-line">
        <summary className={`flex cursor-pointer list-none items-center justify-between px-3 py-2.5 ${eyebrowClass} hover:text-fg`}>
          Advanced
          <span aria-hidden className="transition-transform group-open:rotate-45">+</span>
        </summary>
        <div className="flex flex-col gap-4 border-t border-line p-4">
          <form.AppField name="env">
            {(field) => <field.TextareaField label="Variables" rows={3} placeholder={'DATABASE_URL=${{postgres.DATABASE_URL}}\nLOG_LEVEL=info'} description="KEY=value lines, one per line." />}
          </form.AppField>
          {chosen === 'source' && (
            <form.AppField name="args">
              {(field) => <field.TextareaField label="Build arguments" rows={2} placeholder="NODE_ENV=production" description="Dockerfile ARGs. They stay readable in the image, so no secrets." />}
            </form.AppField>
          )}
          <div className="grid gap-4 sm:grid-cols-2">
            <form.AppField name="replicas">{(field) => <field.NumberField label="Machines" min={1} max={MAX_REPLICAS} />}</form.AppField>
            <form.AppField name="health">
              {(field) => (
                <field.SelectField
                  label="Health check"
                  options={[
                    { value: 'http', label: 'Any HTTP answer' },
                    { value: 'tcp', label: 'Accepts TCP connections' },
                  ]}
                />
              )}
            </form.AppField>
          </div>
          <form.AppField name="volumes" mode="array">
            {(volumes) => (
              <fieldset className="flex flex-col gap-2">
                <legend className={`${eyebrowClass} mb-2`}>Volumes</legend>
                {volumes.state.value.map((_, index) => (
                  <div key={index} className="grid grid-cols-[9rem_1fr_auto] items-start gap-2">
                    <form.AppField name={`volumes[${index}].name`}>{(field) => <field.TextField label="Volume name" hideLabel placeholder="data" mono autoComplete="off" />}</form.AppField>
                    <form.AppField name={`volumes[${index}].path`}>{(field) => <field.TextField label="Mounted at" hideLabel placeholder="/var/lib/app" mono autoComplete="off" />}</form.AppField>
                    <IconButton label="Remove this volume" tone="danger" className="mt-1" onClick={() => volumes.removeValue(index)}>
                      <X aria-hidden className="size-4" />
                    </IconButton>
                  </div>
                ))}
                <div>
                  <Button type="button" variant="ghost" size="sm" className="-ml-2.5" onClick={() => volumes.pushValue({ name: 'data', path: '' })}>
                    <Plus aria-hidden className="size-3.5" />
                    Add a volume
                  </Button>
                </div>
                <p className="text-xs text-fg-faint">Data that outlives deploys. A service with volumes runs one machine.</p>
              </fieldset>
            )}
          </form.AppField>
        </div>
      </details>
    )
  }
}
