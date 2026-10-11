import type { QueryClient } from '@tanstack/react-query'
import { HeadContent, Link, Outlet, createRootRouteWithContext } from '@tanstack/react-router'
import { MotionConfig } from 'motion/react'
import { buttonClass } from '@/components/ui/button-styles'
import { Toaster } from '@/components/ui/toaster'

export const Route = createRootRouteWithContext<{ queryClient: QueryClient }>()({
  head: () => ({ meta: [{ title: 'bird' }] }),
  component: Root,
  notFoundComponent: NotFound,
})

function Root() {
  return (
    // animations follow the system's reduce-motion setting
    <MotionConfig reducedMotion="user">
      <Toaster>
        <HeadContent />
        <Outlet />
      </Toaster>
    </MotionConfig>
  )
}

function NotFound() {
  return (
    <main className="bg-grid grid min-h-dvh place-items-center px-4 text-center">
      <div className="border border-line-strong bg-surface px-10 py-8">
        <p className="font-mono text-2xs tracking-[0.08em] text-fg-faint uppercase">Error 404</p>
        <h1 className="mt-2 text-xl font-semibold tracking-tight text-fg">This page doesn&apos;t exist</h1>
        <Link to="/" className={`${buttonClass('secondary', 'sm')} mt-6`}>
          Go to projects
        </Link>
      </div>
    </main>
  )
}
