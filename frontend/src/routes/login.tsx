import { createFileRoute, redirect } from '@tanstack/react-router'
import { sessionQuery } from '@/features/auth/api'
import { SignInFlow } from '@/features/auth/components/sign-in-flow'
import { safeRedirect } from '@/features/auth/redirect'

type LoginSearch = { redirect?: string }

export const Route = createFileRoute('/login')({
  validateSearch: (search: Record<string, unknown>): LoginSearch => ({
    redirect: typeof search.redirect === 'string' ? search.redirect : undefined,
  }),
  beforeLoad: async ({ context, search }) => {
    const session = await context.queryClient.query(sessionQuery)
    if (session) throw redirect({ href: safeRedirect(search.redirect), replace: true })
  },
  head: () => ({ meta: [{ title: 'Sign in · bird' }] }),
  component: LoginPage,
})

function LoginPage() {
  const { redirect: target } = Route.useSearch()
  return <SignInFlow redirectTo={safeRedirect(target)} />
}
