import { useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Plus, Variable } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { EmptyState } from '@/components/ui/empty-state'
import { ErrorState } from '@/components/ui/error-state'
import { Skeleton } from '@/components/ui/skeleton'
import { notifySuccess } from '@/lib/toast'
import { updateVariables, variablesQuery } from '@/features/variables/api'
import { ImportDialog } from '@/features/variables/components/import-dialog'
import { VariableDialog, type VariableInput } from '@/features/variables/components/variable-dialog'
import { VariablesTable } from '@/features/variables/components/variables-table'
import { useServiceScope } from '@/features/services/use-service-scope'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/variables')({
  component: Variables,
})

function Variables() {
  const { scope, service } = useServiceScope()
  const queryClient = useQueryClient()
  const variables = useQuery(variablesQuery(scope, service))

  async function add(input: VariableInput) {
    await updateVariables(scope, service, { set: { [input.key]: input.value }, deploy: input.apply })
    await queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment, service] })
    notifySuccess(`Added ${input.key}`, input.apply ? `${service} was redeployed with it.` : 'It applies on the next deploy.')
  }

  async function importAll(values: Record<string, string>, apply: boolean) {
    await updateVariables(scope, service, { set: values, deploy: apply })
    await queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment, service] })
    const count = Object.keys(values).length
    notifySuccess(`Saved ${count} ${count === 1 ? 'variable' : 'variables'}`, apply ? `${service} was redeployed with them.` : 'They apply on the next deploy.')
  }

  const addButton = (
    <VariableDialog
      trigger={
        <Button size="sm">
          <Plus aria-hidden className="size-4" />
          New variable
        </Button>
      }
      onSubmit={add}
    />
  )

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-sm text-fg-muted">
          Values stay hidden until you reveal them. Each deploy keeps a copy of the values it ran with.
        </p>
        <div className="flex gap-2">
          <ImportDialog onImport={importAll} />
          {variables.data && variables.data.length > 0 && addButton}
        </div>
      </div>
      {variables.isPending ? (
        <Skeleton className="h-32 w-full rounded-xl" />
      ) : variables.isError ? (
        <ErrorState error={variables.error} onRetry={() => void variables.refetch()} />
      ) : variables.data.length === 0 ? (
        <EmptyState icon={Variable} title="No variables" description="Add configuration and secrets your app reads from its environment." action={addButton} />
      ) : (
        <VariablesTable scope={scope} service={service} keys={variables.data} />
      )}
    </div>
  )
}
