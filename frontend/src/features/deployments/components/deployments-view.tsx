import { useState } from 'react'
import type { Scope } from '@/lib/api/paths'
import type { Deployment } from '../api'
import { preferredDeployment } from '../status'
import { DeploymentDetail } from './deployment-detail'
import { DeploymentRow } from './deployment-row'

type DeploymentsViewProps = { scope: Scope; service: string; deployments: Deployment[] }

// the history on the left, the chosen deploy and its log on the right
export function DeploymentsView({ scope, service, deployments }: DeploymentsViewProps) {
  const [chosen, setChosen] = useState<string>()
  const selected = deployments.find((deployment) => deployment.id === chosen) ?? preferredDeployment(deployments)
  return (
    <div className="grid items-start gap-4 lg:grid-cols-[minmax(17rem,22rem)_1fr]">
      <ol aria-label="Deployments" className="divide-y divide-line border border-line bg-surface lg:sticky lg:top-20 lg:max-h-[calc(100vh-7rem)] lg:overflow-y-auto">
        {deployments.map((deployment) => (
          <li key={deployment.id}>
            <DeploymentRow deployment={deployment} selected={deployment.id === selected?.id} onSelect={() => setChosen(deployment.id)} />
          </li>
        ))}
      </ol>
      <section aria-label="Selected deployment" className="border border-line bg-surface p-4">
        {selected && <DeploymentDetail key={selected.id + selected.status} scope={scope} service={service} deployment={selected} />}
      </section>
    </div>
  )
}
