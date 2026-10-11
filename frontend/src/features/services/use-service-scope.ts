import { useParams, useRouteContext } from '@tanstack/react-router'

// the scope and service name of the service page that is open
export function useServiceScope() {
  const { scope } = useRouteContext({ from: '/_app/projects/$project/$environment' })
  const { service } = useParams({ from: '/_app/projects/$project/$environment/services/$service' })
  return { scope, service }
}
