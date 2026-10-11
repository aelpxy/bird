import { Link } from '@tanstack/react-router'
import { Search } from 'lucide-react'
import { useState, type ReactNode } from 'react'
import type { Session } from '@/features/auth/api'
import { UserMenu } from '@/features/auth/components/user-menu'
import { CommandPalette } from '@/features/command/components/command-palette'
import { focusRing } from '@/components/ui/control-styles'
import { Logo } from './logo'

const navLinkClass = `px-2.5 py-1.5 font-mono text-2xs leading-none font-medium tracking-[0.08em] text-fg-faint uppercase transition-colors hover:text-fg data-[status=active]:text-fg ${focusRing}`

export function AppShell({ session, children }: { session: Session; children: ReactNode }) {
  const [searching, setSearching] = useState(false)
  return (
    <div className="flex min-h-dvh flex-col">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50 focus:bg-fg focus:px-3 focus:py-2 focus:text-sm focus:text-canvas"
      >
        Skip to content
      </a>
      <header className="material sticky top-0 z-20 border-b border-line">
        <div className="mx-auto flex h-12 max-w-6xl items-center justify-between gap-4 px-4">
          <div className="flex items-center gap-4">
            <Link to="/" aria-label="bird, projects" className={focusRing}>
              <Logo />
            </Link>
            <nav aria-label="Main" className="hidden sm:block">
              <ul className="flex items-center">
                <li>
                  <Link to="/" activeOptions={{ exact: true }} className={navLinkClass}>
                    Projects
                  </Link>
                </li>
                <li>
                  <Link to="/orgs" className={navLinkClass}>
                    Orgs
                  </Link>
                </li>
                {session.role === 'admin' && (
                  <>
                    <li>
                      <Link to="/registries" className={navLinkClass}>
                        Registries
                      </Link>
                    </li>
                    <li>
                      <Link to="/admin/users" className={navLinkClass}>
                        Users
                      </Link>
                    </li>
                  </>
                )}
              </ul>
            </nav>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setSearching(true)}
              aria-label="Go to a service, project or page"
              aria-keyshortcuts="Meta+K Control+K"
              className={`pressable flex h-7 items-center gap-2 border border-line px-2 text-fg-faint transition-colors hover:border-line-strong hover:text-fg ${focusRing}`}
            >
              <Search aria-hidden className="size-3.5" />
              <kbd className="hidden font-mono text-2xs sm:inline">⌘K</kbd>
            </button>
            <UserMenu session={session} />
          </div>
        </div>
      </header>
      <CommandPalette session={session} open={searching} onOpenChange={setSearching} />
      <main id="main" className="mx-auto w-full max-w-6xl flex-1 px-4 py-8">
        {children}
      </main>
    </div>
  )
}
