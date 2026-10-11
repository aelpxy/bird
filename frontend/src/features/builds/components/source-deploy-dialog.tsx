import { useQueryClient } from '@tanstack/react-query'
import { FolderCode } from 'lucide-react'
import { z } from 'zod'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { Button } from '@/components/ui/button'
import { FormDialog } from '@/components/ui/form-dialog'
import type { Scope } from '@/lib/api/paths'
import { notifySuccess } from '@/lib/toast'
import type { Service } from '../../services/api'
import { buildArgs, dockerfile } from '../schemas'
import { useSourceDeploy } from '../use-source-deploy'
import { LiveDeployLog } from '@/features/deployments/components/live-deploy-log'
import { BuildProgress } from './build-progress'
import { FolderPicker } from './folder-picker'

const schema = z.object({ dockerfile, args: buildArgs })

// builds an existing service's new image from a local folder and deploys it
export function SourceDeployDialog({ scope, service }: { scope: Scope; service: Service }) {
  const queryClient = useQueryClient()
  const source = useSourceDeploy(scope)
  const dialog = useDialogForm({
    schema,
    defaultValues: { dockerfile: 'Dockerfile', args: '' },
    onSubmit: async (values) => {
      await source.run(service.name, service.port, values)
      notifySuccess(`Built and deployed ${service.name}`)
      await queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment, service.name] })
    },
  })
  return (
    <FormDialog
      dialog={dialog}
      trigger={
        <Button variant="secondary" size="sm">
          <FolderCode aria-hidden className="size-3.5" />
          Deploy from source
        </Button>
      }
      size="lg"
      title={`Deploy ${service.name} from source`}
      description={`Builds on the server and deploys on port ${service.port}, keeping its other settings.`}
      submitLabel="Build and deploy"
    >
      <FolderPicker folder={source.folder} onFolder={source.setFolder} />
      <dialog.form.AppField name="dockerfile">{(field) => <field.TextField label="Dockerfile" mono autoComplete="off" spellCheck={false} />}</dialog.form.AppField>
      <dialog.form.AppField name="args">
        {(field) => (
          <field.TextareaField
            label="Build arguments (optional)"
            rows={2}
            placeholder="NODE_ENV=production"
            description="KEY=VALUE lines for Dockerfile ARGs. They stay readable in the image, so no secrets."
          />
        )}
      </dialog.form.AppField>
      {source.progress && <BuildProgress progress={source.progress} />}
      {source.progress?.phase === 'deploying' && <LiveDeployLog scope={scope} service={service.name} />}
    </FormDialog>
  )
}
