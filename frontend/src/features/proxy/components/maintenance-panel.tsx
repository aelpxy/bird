import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Construction } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Panel } from '@/components/ui/panel'
import type { Scope } from '@/lib/api/paths'
import { notifyError, notifySuccess } from '@/lib/toast'
import { proxyQuery, saveProxy, type ProxySettings } from '../api'

// a switch rather than a form field: it takes effect at once, like stopping the service
export function MaintenancePanel({ scope, service, settings }: { scope: Scope; service: string; settings: ProxySettings }) {
  const queryClient = useQueryClient()
  const on = settings.maintenance ?? false
  const toggle = useMutation({
    mutationFn: () => saveProxy(scope, service, { ...settings, maintenance: !on }),
    onSuccess: (saved) => {
      queryClient.setQueryData(proxyQuery(scope, service).queryKey, saved)
      notifySuccess(saved.maintenance ? `${service} is in maintenance` : `${service} is serving again`)
    },
    onError: (error) => notifyError(`Couldn't change maintenance for ${service}`, error),
  })
  return (
    <Panel tone={on ? 'notice' : 'default'} className="flex flex-wrap items-center gap-4">
      <Construction aria-hidden className="size-4 shrink-0 text-fg-faint" />
      <div className="min-w-0 flex-1">
        <p className="flex items-center gap-2 text-sm font-medium text-fg">
          Maintenance mode
          {on && (
            <Badge tone="warn" dot>
              On
            </Badge>
          )}
        </p>
        <p className="mt-0.5 text-xs text-fg-muted">
          {on ? 'Visitors get a 503 page. The machines keep running, so turning it off is instant.' : 'Answer every visitor with a 503 page while you work, without stopping the machines.'}
        </p>
      </div>
      <Button variant={on ? 'primary' : 'secondary'} size="sm" disabled={toggle.isPending} onClick={() => toggle.mutate()}>
        {on ? 'Turn off' : 'Turn on'}
      </Button>
    </Panel>
  )
}
