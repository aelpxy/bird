import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { Box, Database, FolderCode, Plus } from 'lucide-react'
import { z } from 'zod'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { Button } from '@/components/ui/button'
import { ChoiceCard, ChoiceGroup } from '@/components/ui/choice-card'
import { FormDialog } from '@/components/ui/form-dialog'
import type { Scope } from '@/lib/api/paths'
import { notifySuccess } from '@/lib/toast'
import { NAME_HINT, image, name, optionalHostname, optionalName, port } from '@/lib/validation'
import { buildArgs, dockerfile } from '../../builds/schemas'
import { BuildProgress } from '../../builds/components/build-progress'
import { FolderPicker } from '../../builds/components/folder-picker'
import { useSourceDeploy } from '../../builds/use-source-deploy'
import { deployTemplate, templatesQuery } from '../../templates/api'
import { deploy } from '../api'

type Mode = 'image' | 'source' | 'template'

const modes = [
  { value: 'image', label: 'Image', description: 'Any container image', icon: Box },
  { value: 'source', label: 'Source', description: 'A folder with a Dockerfile', icon: FolderCode },
  { value: 'template', label: 'Template', description: 'A ready-made database', icon: Database },
] as const

// only the fields of the chosen way are checked, each against the rule birdd applies
const fieldsFor: Record<Mode, Record<string, z.ZodType>> = {
  image: { name: name(), image, port, domain: optionalHostname },
  source: { name: name(), port, dockerfile, args: buildArgs },
  template: { template: z.string().min(1, 'Pick a template.'), templateName: optionalName },
}

const schema = z
  .object({
    mode: z.enum(['image', 'source', 'template']),
    name: z.string(),
    image: z.string(),
    port: z.number(),
    domain: z.string(),
    dockerfile: z.string(),
    args: z.string(),
    template: z.string(),
    templateName: z.string(),
  })
  .superRefine((values, context) => {
    for (const [key, rule] of Object.entries(fieldsFor[values.mode])) {
      const result = rule.safeParse(values[key as keyof typeof values])
      for (const issue of result.error?.issues ?? []) context.addIssue({ code: 'custom', message: issue.message, path: [key] })
    }
  })

const DEPLOY_NOTE = 'Pulling the image and waiting for it to pass its health check. This can take a minute.'

export function NewServiceDialog({ scope }: { scope: Scope }) {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const templates = useQuery(templatesQuery)
  const source = useSourceDeploy(scope)
  const dialog = useDialogForm({
    schema,
    defaultValues: { mode: 'image' as Mode, name: '', image: '', port: 80, domain: '', dockerfile: 'Dockerfile', args: '', template: '', templateName: '' },
    onSubmit: async (values) => {
      let service: string
      if (values.mode === 'image') {
        const domain = optionalHostname.parse(values.domain)
        service = (await deploy(scope, { name: values.name.trim(), image: values.image.trim(), port: values.port, domains: domain ? [domain] : [] })).service
        notifySuccess(`Deployed ${service}`)
      } else if (values.mode === 'source') {
        const args = buildArgs.parse(values.args)
        service = (await source.run(values.name.trim(), values.port, { dockerfile: values.dockerfile, args })).service
        notifySuccess(`Built and deployed ${service}`)
      } else {
        const result = await deployTemplate(scope, values.template, values.templateName.trim())
        service = result.deployment.service
        notifySuccess(`Deployed ${service}`, result.connection ? `Other services connect with \${{${service}.${result.connection}}}.` : undefined)
      }
      await queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment] })
      await navigate({ to: '/projects/$project/$environment/services/$service', params: { ...scope, service } })
    },
  })
  const { form } = dialog

  return (
    <FormDialog
      dialog={dialog}
      trigger={
        <Button>
          <Plus aria-hidden className="size-3.5" />
          New service
        </Button>
      }
      size="lg"
      title="New service"
      description={`Deploys into ${scope.project} / ${scope.environment}.`}
      submitLabel="Deploy"
      pendingNote={DEPLOY_NOTE}
    >
      <form.AppField name="mode">
        {(modeField) => (
          <>
            <ChoiceGroup legend="Deploy from">
              {modes.map((option) => (
                <ChoiceCard
                  key={option.value}
                  name="mode"
                  label={option.label}
                  description={option.description}
                  icon={<option.icon aria-hidden className="size-4" />}
                  checked={modeField.state.value === option.value}
                  onSelect={() => modeField.handleChange(option.value)}
                />
              ))}
            </ChoiceGroup>

            {modeField.state.value !== 'template' && (
              <div className="grid gap-4 sm:grid-cols-[1fr_7rem]">
                <form.AppField name="name">
                  {(field) => <field.TextField label="Service name" placeholder="web" description={NAME_HINT} autoFocus autoComplete="off" autoCapitalize="none" spellCheck={false} />}
                </form.AppField>
                <form.AppField name="port">{(field) => <field.NumberField label="Port" min={1} max={65535} />}</form.AppField>
              </div>
            )}

            {modeField.state.value === 'image' && (
              <>
                <form.AppField name="image">
                  {(field) => (
                    <field.TextField label="Image" placeholder="ghcr.io/acme/web:1.4" description="Private registries need credentials, added under Registries." mono autoComplete="off" autoCapitalize="none" spellCheck={false} />
                  )}
                </form.AppField>
                <form.AppField name="domain">
                  {(field) => (
                    <field.TextField label="Domain (optional)" placeholder="app.example.com" description="The app must listen on 0.0.0.0 on its port. Add variables and more domains after it is up." autoComplete="off" autoCapitalize="none" spellCheck={false} />
                  )}
                </form.AppField>
              </>
            )}

            {modeField.state.value === 'source' && (
              <>
                <FolderPicker folder={source.folder} onFolder={source.setFolder} />
                <form.AppField name="dockerfile">{(field) => <field.TextField label="Dockerfile" mono autoComplete="off" spellCheck={false} />}</form.AppField>
                <form.AppField name="args">
                  {(field) => <field.TextareaField label="Build arguments (optional)" rows={2} placeholder="NODE_ENV=production" description="KEY=VALUE lines for Dockerfile ARGs. They stay readable in the image, so no secrets." />}
                </form.AppField>
                {source.progress && <BuildProgress progress={source.progress} />}
              </>
            )}

            {modeField.state.value === 'template' && (
              <>
                <form.AppField name="template">
                  {(templateField) => (
                    <ChoiceGroup legend="Template" columns="stack">
                      {(templates.data ?? []).map((option) => (
                        <ChoiceCard
                          key={option.name}
                          name="template"
                          label={option.name}
                          description={option.description}
                          checked={templateField.state.value === option.name}
                          onSelect={() => templateField.handleChange(option.name)}
                        />
                      ))}
                      {templates.isPending && <p className="text-sm text-fg-faint">Loading templates…</p>}
                      {templateField.state.meta.errors.length > 0 && form.state.submissionAttempts > 0 && <p className="text-xs text-bad">Pick a template.</p>}
                    </ChoiceGroup>
                  )}
                </form.AppField>
                <form.AppField name="templateName">
                  {(field) => <field.TextField label="Service name (optional)" description={`Defaults to the template's name. ${NAME_HINT}`} autoComplete="off" autoCapitalize="none" spellCheck={false} />}
                </form.AppField>
              </>
            )}
          </>
        )}
      </form.AppField>
    </FormDialog>
  )
}
