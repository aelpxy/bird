import { Outlet, createFileRoute, redirect } from '@tanstack/react-router'
import { AppShell } from '@/components/layout/app-shell'
import { sessionQuery } from '@/features/auth/api'

// every page under here needs a signed-in user; others are sent to sign in and brought back
export const Route = createFileRoute('/_app')({
  beforeLoad: async ({ context, location }) => {
    const session = await context.queryClient.query(sessionQuery)
    if (!session) {
      throw redirect({ to: '/login', search: { redirect: location.href }, replace: true })
    }
    return { session }
  },
  component: AppLayout,
})

function AppLayout() {
  const { session } = Route.useRouteContext()
  return (
    <AppShell session={session}>
      <Outlet />
    </AppShell>
  )
}
