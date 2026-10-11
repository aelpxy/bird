import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider, createRouter } from '@tanstack/react-router'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { ApiError, isUnauthorized } from './lib/api/client'
import { sessionQuery } from './features/auth/api'
import { routeTree } from './routeTree.gen'
import './index.css'

// a session that expires while the app is open sends the user to sign in, then back here
function onSessionExpired() {
  if (queryClient.getQueryData(sessionQuery.queryKey) === null) return
  queryClient.setQueryData(sessionQuery.queryKey, null)
  const here = router.state.location.href
  void router.navigate({ to: '/login', search: { redirect: here }, replace: true })
}

const queryClient = new QueryClient({
  queryCache: new QueryCache({
    onError: (error) => {
      if (isUnauthorized(error)) onSessionExpired()
    },
  }),
  mutationCache: new MutationCache({
    onError: (error, _variables, _context, mutation) => {
      // a wrong password is not an expired session
      if (isUnauthorized(error) && !mutation.meta?.signIn) onSessionExpired()
    },
  }),
  defaultOptions: {
    queries: {
      // client errors will not change on retry; network blips and 5xx might
      retry: (failures, error) =>
        failures < 2 && !(error instanceof ApiError && error.status < 500),
    },
  },
})

const router = createRouter({
  routeTree,
  context: { queryClient },
  defaultPreload: 'intent',
  // react query decides freshness, so the router never serves its own stale copy
  defaultPreloadStaleTime: 0,
  scrollRestoration: true,
})

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
)
