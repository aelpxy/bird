import { useMutation, useQueryClient } from '@tanstack/react-query'
import { z } from 'zod'
import { useAppForm } from '@/components/form/app-form'
import type { Scope } from '@/lib/api/paths'
import { plural } from '@/lib/format'
import { notifyError, notifySuccess } from '@/lib/toast'
import { wholeNumber } from '@/lib/validation'
import { scale, type Service } from '../api'

const MAX_REPLICAS = 32
const schema = z.object({ replicas: wholeNumber(1, MAX_REPLICAS, 'the number of machines') })

export function ScalePanel({ scope, service }: { scope: Scope; service: Service }) {
  const queryClient = useQueryClient()
  const mutation = useMutation({
    mutationFn: (replicas: number) => scale(scope, service.name, replicas),
    onSuccess: (_result, replicas) => notifySuccess(`Scaling ${service.name} to ${plural(replicas, 'machine')}`),
    onError: (error) => notifyError(`Couldn't scale ${service.name}`, error),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment] }),
  })
  const form = useAppForm({
    defaultValues: { replicas: service.replicas },
    validators: { onChange: schema },
    onSubmit: ({ value }) => mutation.mutateAsync(value.replicas).catch(() => undefined),
  })
  const pinned = service.volumes.length > 0

  return (
    <form.AppForm>
      <form.FormElement className="flex flex-wrap items-start gap-2">
        <div className="w-36">
          <form.AppField name="replicas">{(field) => <field.NumberField label="Machines" min={1} max={MAX_REPLICAS} disabled={pinned} />}</form.AppField>
        </div>
        <div className="mt-[1.6rem]">
          <form.SubmitButton variant="secondary" disabled={pinned}>
            Save
          </form.SubmitButton>
        </div>
        <p className="basis-full text-xs text-fg-faint">
          {pinned ? 'A service with volumes runs exactly one machine, since only one may write its data.' : `Requests are spread across machines. Up to ${MAX_REPLICAS}.`}
        </p>
      </form.FormElement>
    </form.AppForm>
  )
}
