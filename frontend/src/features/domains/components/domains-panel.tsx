import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ArrowUpRight, Globe, Plus, X } from 'lucide-react'
import { z } from 'zod'
import { useAppForm } from '@/components/form/app-form'
import { Code } from '@/components/ui/code'
import { focusRing } from '@/components/ui/control-styles'
import { IconButton } from '@/components/ui/icon-button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import type { Scope } from '@/lib/api/paths'
import { notifyError, notifySuccess } from '@/lib/toast'
import { hostname } from '@/lib/validation'
import type { Service } from '@/features/services/api'
import { addDomain, domainUrl, domainsQuery, removeDomain, setRedirect } from '@/features/domains/api'
import { SelectField } from '@/components/ui/select-field'
import { HttpsBadge } from './https-badge'

const addSchema = z.object({ hostname })

export function DomainsPanel({ scope, service }: { scope: Scope; service: Service }) {
  const queryClient = useQueryClient()
  const statuses = useQuery(domainsQuery(scope, service.name))
  const refresh = () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment] }),
      queryClient.invalidateQueries({ queryKey: domainsQuery(scope, service.name).queryKey }),
    ])
  const add = useMutation({
    mutationFn: (hostname: string) => addDomain(scope, service.name, hostname),
    onSuccess: (_domains, hostname) => {
      notifySuccess(`Added ${hostname}`, 'Point its DNS at this server; HTTPS follows once it resolves.')
      form.reset()
      return refresh()
    },
    onError: (error, hostname) => notifyError(`Couldn't add ${hostname}`, error),
  })
  const redirect = useMutation({
    mutationFn: ({ hostname, target }: { hostname: string; target: string | null }) => setRedirect(scope, service.name, hostname, target),
    onSuccess: (_result, { hostname, target }) => notifySuccess(target ? `${hostname} now redirects to ${target}` : `${hostname} serves ${service.name} again`),
    onError: (error, { hostname }) => notifyError(`Couldn't change ${hostname}`, error),
    onSettled: refresh,
  })
  const form = useAppForm({
    defaultValues: { hostname: '' },
    validators: { onSubmit: addSchema },
    onSubmit: ({ value }) => add.mutateAsync(addSchema.parse(value).hostname).catch(() => undefined),
  })

  return (
    <div className="flex flex-col gap-4">
      {service.domains.length > 0 ? (
        <ul className="divide-y divide-line border border-line bg-surface">
          {service.domains.map((domain) => {
            const status = statuses.data?.find((candidate) => candidate.hostname === domain)
            const https = status?.https
            // a redirect lands on a domain that serves the app, so targets that redirect are left out
            const targets = (statuses.data ?? []).filter((candidate) => candidate.hostname !== domain && !candidate.redirect_to)
            return (
              <li key={domain} className="flex flex-col gap-1.5 px-4 py-2.5">
                <div className="flex items-center justify-between gap-3">
                  <a
                    href={domainUrl(domain)}
                    target="_blank"
                    rel="noreferrer"
                    className={`inline-flex min-w-0 items-center gap-2 font-mono text-ui text-fg hover:underline underline-offset-4 ${focusRing}`}
                  >
                    <Globe aria-hidden className="size-4 shrink-0 text-fg-faint" />
                    <span className="truncate">{domain}</span>
                    <ArrowUpRight aria-hidden className="size-3.5 shrink-0 text-fg-faint" />
                  </a>
                  <span className="ml-auto">{https && <HttpsBadge https={https} />}</span>
                  {status && (targets.length > 0 || status.redirect_to) && (
                    <div className="w-56">
                      <SelectField
                        label={`What ${domain} does`}
                        hideLabel
                        value={status.redirect_to ?? ''}
                        disabled={redirect.isPending}
                        options={[{ value: '', label: `Serves ${service.name}` }, ...targets.map((target) => ({ value: target.hostname, label: `Redirects to ${target.hostname}` }))]}
                        onChange={(target) => redirect.mutate({ hostname: domain, target: target || null })}
                      />
                    </div>
                  )}
                  <ConfirmDialog
                    trigger={
                      <IconButton label={`Remove ${domain}`} tone="danger">
                        <X aria-hidden className="size-4" />
                      </IconButton>
                    }
                    title={`Remove ${domain}?`}
                    description={`Requests to ${domain} stop reaching ${service.name} right away.`}
                    confirmLabel="Remove domain"
                    tone="danger"
                    onConfirm={async () => {
                      await removeDomain(scope, service.name, domain)
                      notifySuccess(`Removed ${domain}`)
                      await refresh()
                    }}
                  />
                </div>
                {https?.state === 'failing' && <p className="pl-6 font-mono text-2xs break-words text-bad">{https.error}</p>}
              </li>
            )
          })}
        </ul>
      ) : (
        <p className="text-sm text-fg-muted">
          No domains: {service.name} is only reachable from other services in {scope.environment}, as{' '}
          <Code>{service.name}</Code>.
        </p>
      )}
      <form.AppForm>
        <form.FormElement className="flex flex-wrap items-start gap-2">
          <div className="min-w-60 flex-1">
            <form.AppField name="hostname">
              {(field) => <field.TextField label="Add a domain" placeholder="app.example.com" autoComplete="off" autoCapitalize="none" spellCheck={false} />}
            </form.AppField>
          </div>
          <div className="mt-[1.6rem]">
            <form.SubmitButton>
              <Plus aria-hidden className="size-3.5" />
              Add
            </form.SubmitButton>
          </div>
        </form.FormElement>
      </form.AppForm>
    </div>
  )
}
