import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useAppForm } from '@/components/form/app-form'
import { Eyebrow } from '@/components/ui/eyebrow'
import type { Scope } from '@/lib/api/paths'
import { notifyError, notifySuccess } from '@/lib/toast'
import { proxyQuery, saveProxy, type ProxySettings } from '../api'
import { DEFAULT_MAX_BODY_MB, DEFAULT_TIMEOUT_SECS, proxySchema, toForm, toSettings } from '../schema'

export function ProxyForm({ scope, service, settings }: { scope: Scope; service: string; settings: ProxySettings }) {
  const queryClient = useQueryClient()
  const save = useMutation({
    mutationFn: (next: ProxySettings) => saveProxy(scope, service, next),
    onSuccess: (saved) => {
      queryClient.setQueryData(proxyQuery(scope, service).queryKey, saved)
      notifySuccess('Proxy settings saved', 'They apply from the next request.')
    },
    onError: (error) => notifyError("Couldn't save the proxy settings", error),
  })
  const form = useAppForm({
    defaultValues: toForm(settings),
    validators: { onChange: proxySchema },
    onSubmit: ({ value }) => save.mutateAsync(toSettings(value, settings.maintenance ?? false)).catch(() => undefined),
  })

  return (
    <form.AppForm>
      <form.FormElement className="flex flex-col gap-8">
        <fieldset className="flex flex-col gap-4">
          <Eyebrow as="legend" className="mb-3">
            Rate limiting
          </Eyebrow>
          <form.AppField name="rateLimited">
            {(field) => (
              <field.CheckboxField
                label="Limit how fast each visitor can make requests"
                description="Counted per IP address (IPv6 per /64). Visitors over the limit get 429 Too Many Requests with a Retry-After header."
              />
            )}
          </form.AppField>
          <form.Subscribe selector={(state) => state.values.rateLimited}>
            {(limited) =>
              limited && (
                <div className="grid gap-4 sm:grid-cols-2">
                  <form.AppField name="perSecond">
                    {(field) => <field.NumberField label="Requests per second" min={1} max={10000} description="The sustained rate each visitor gets." />}
                  </form.AppField>
                  <form.AppField name="burst">
                    {(field) => <field.NumberField label="Burst" min={1} max={100000} description="Requests allowed at once, like a page loading its assets." />}
                  </form.AppField>
                </div>
              )
            }
          </form.Subscribe>
        </fieldset>

        <fieldset className="flex flex-col gap-4">
          <Eyebrow as="legend" className="mb-3">
            Cloudflare
          </Eyebrow>
          <form.AppField name="cloudflare">
            {(field) => (
              <field.CheckboxField
                label="The domains are proxied by Cloudflare"
                description="Takes each visitor's address from CF-Connecting-IP on requests that come from Cloudflare's network, for rate limits and X-Forwarded-For. Requests from anywhere else keep their own address."
              />
            )}
          </form.AppField>
        </fieldset>

        <fieldset className="flex flex-col gap-4">
          <Eyebrow as="legend" className="mb-3">
            Limits
          </Eyebrow>
          <div className="grid gap-4 sm:grid-cols-2">
            <form.AppField name="maxBodyMb">
              {(field) => <field.NumberField label="Largest request body (MiB)" min={1} max={10240} description={`Bigger uploads get 413. ${DEFAULT_MAX_BODY_MB} by default.`} />}
            </form.AppField>
            <form.AppField name="timeoutSecs">
              {(field) => <field.NumberField label="Response timeout (seconds)" min={1} max={3600} description={`How long the app has to start answering before a 504. ${DEFAULT_TIMEOUT_SECS} by default.`} />}
            </form.AppField>
          </div>
        </fieldset>

        <fieldset className="flex flex-col gap-4">
          <Eyebrow as="legend" className="mb-3">
            Response headers
          </Eyebrow>
          <form.AppField name="securityHeaders">
            {(field) => (
              <field.CheckboxField
                label="Add security headers"
                description="HSTS over HTTPS, X-Content-Type-Options, X-Frame-Options and Referrer-Policy, each only when the app does not set it already."
              />
            )}
          </form.AppField>
          <form.AppField name="headers">
            {(field) => (
              <field.TextareaField
                label="Custom headers"
                rows={4}
                placeholder={'cache-control: no-store\nx-robots-tag: noindex'}
                description="One per line as name: value. They replace whatever the app sends."
                spellCheck={false}
              />
            )}
          </form.AppField>
        </fieldset>

        <div>
          <form.SubmitButton>Save</form.SubmitButton>
        </div>
      </form.FormElement>
    </form.AppForm>
  )
}
