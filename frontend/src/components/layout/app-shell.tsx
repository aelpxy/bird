import { Dialog } from '@base-ui/react/dialog'
import { Link, useRouterState } from '@tanstack/react-router'
import { Menu, Search } from 'lucide-react'
import { useState, type ReactNode } from 'react'
import { focusRing } from '@/components/ui/control-styles'
import { backdropClass } from '@/components/ui/overlay-styles'
import type { Session } from '@/features/auth/api'
import { CommandPalette } from '@/features/command/components/command-palette'
import { Sidebar } from '@/features/navigation/components/sidebar'
import { Logo } from './logo'

const drawerClass =
  'fixed inset-y-0 left-0 z-50 w-72 max-w-[85vw] border-r border-line-strong bg-canvas transition-transform duration-200 ease-out data-ending-style:-translate-x-full data-starting-style:-translate-x-full'

export function AppShell({ session, children }: { session: Session; children: ReactNode }) {
  const [searching, setSearching] = useState(false)
  const path = useRouterState({ select: (state) => state.location.pathname })
  // remembers where it was opened, so following a link in it closes it
  const [drawerAt, setDrawerAt] = useState<string | null>(null)
  const drawer = drawerAt === path
  const setDrawer = (open: boolean) => setDrawerAt(open ? path : null)

  return (
    <div className="min-h-dvh lg:grid lg:grid-cols-[16rem_minmax(0,1fr)]">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50 focus:bg-fg focus:px-3 focus:py-2 focus:text-sm focus:text-canvas"
      >
        Skip to content
      </a>
      <aside className="sticky top-0 hidden h-dvh border-r border-line lg:block">
        <Sidebar session={session} onSearch={() => setSearching(true)} />
      </aside>

      <header className="material sticky top-0 z-20 flex h-12 items-center justify-between gap-3 border-b border-line px-4 lg:hidden">
        <div className="flex items-center gap-3">
          <button type="button" onClick={() => setDrawer(true)} aria-label="Open navigation" className={`pressable grid size-8 place-items-center text-fg-muted hover:text-fg ${focusRing}`}>
            <Menu aria-hidden className="size-4" />
          </button>
          <Link to="/" aria-label="bird, overview" className={focusRing}>
            <Logo />
          </Link>
        </div>
        <button type="button" onClick={() => setSearching(true)} aria-label="Go to a service, project or page" className={`pressable grid size-8 place-items-center text-fg-muted hover:text-fg ${focusRing}`}>
          <Search aria-hidden className="size-4" />
        </button>
      </header>
      <Dialog.Root open={drawer} onOpenChange={setDrawer}>
        <Dialog.Portal>
          <Dialog.Backdrop className={backdropClass} />
          <Dialog.Popup aria-label="Navigation" className={drawerClass}>
            <Sidebar
              session={session}
              onSearch={() => {
                setDrawer(false)
                setSearching(true)
              }}
            />
          </Dialog.Popup>
        </Dialog.Portal>
      </Dialog.Root>

      <CommandPalette session={session} open={searching} onOpenChange={setSearching} />
      <main id="main" className="mx-auto w-full max-w-6xl min-w-0 px-4 py-8 lg:px-8">
        {children}
      </main>
    </div>
  )
}
