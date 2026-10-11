import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Play, Rocket, RotateCw, Square } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import type { Scope } from '@/lib/api/paths'
import { notifyError, notifySuccess } from '@/lib/toast'
import { deploy, power, type PowerAction, type Service } from '@/features/services/api'

const done: Record<PowerAction, string> = {
  start: 'Started',
  stop: 'Stopped',
  restart: 'Restarted',
}

export function ServiceActions({ scope, service }: { scope: Scope; service: Service }) {
  const queryClient = useQueryClient()
  const mutation = useMutation({
    mutationFn: (action: PowerAction) => power(scope, service.name, action),
    onSuccess: (_summary, action) => notifySuccess(`${done[action]} ${service.name}`),
    onError: (error, action) => notifyError(`Couldn't ${action} ${service.name}`, error),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment] }),
  })
  const pending = mutation.isPending ? mutation.variables : undefined
  const canRun = service.deployment != null
  const redeployment = useMutation({
    mutationFn: () => deploy(scope, { name: service.name, image: service.image, port: service.port }),
    onSuccess: () => notifySuccess(`Redeployed ${service.name}`, 'New machines took over once they were healthy.'),
    onError: (error) => notifyError(`Couldn't redeploy ${service.name}`, error),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment] }),
  })
  // safe to run at once: traffic only moves once the new machines pass their health check
  const redeploy = (
    <Button variant="secondary" size="sm" loading={redeployment.isPending} disabled={mutation.isPending} onClick={() => redeployment.mutate()}>
      {!redeployment.isPending && <Rocket aria-hidden className="size-3.5" />}
      Redeploy
    </Button>
  )

  if (service.state === 'stopped') {
    return (
      <>
        {redeploy}
        <Button
          variant="secondary"
          size="sm"
          loading={pending === 'start'}
          disabled={!canRun}
          onClick={() => mutation.mutate('start')}
        >
          {pending !== 'start' && <Play aria-hidden className="size-4" />}
          Start
        </Button>
      </>
    )
  }
  return (
    <>
      {redeploy}
      <Button
        variant="secondary"
        size="sm"
        loading={pending === 'restart'}
        disabled={!canRun || mutation.isPending}
        onClick={() => mutation.mutate('restart')}
      >
        {pending !== 'restart' && <RotateCw aria-hidden className="size-4" />}
        Restart
      </Button>
      <ConfirmDialog
        trigger={
          <Button variant="secondary" size="sm" disabled={!canRun || mutation.isPending}>
            <Square aria-hidden className="size-3.5" />
            Stop
          </Button>
        }
        title={`Stop ${service.name}?`}
        description="Its machines stop and requests to its domains fail until you start it again. Nothing is deleted."
        confirmLabel="Stop service"
        tone="danger"
        onConfirm={() => mutation.mutateAsync('stop')}
      />
    </>
  )
}
