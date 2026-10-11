import { Link } from '@tanstack/react-router'
import type { ReactNode } from 'react'
import type { Session } from '@/features/auth/api'
import { UserMenu } from '@/features/auth/components/user-menu'
import { focusRing } from '@/components/ui/control-styles'
import { Logo } from './logo'

const navLinkClass = `px-2.5 py-1.5 font-mono text-2xs leading-none font-medium tracking-[0.08em] text-fg-faint uppercase transition-colors hover:text-fg data-[status=active]:text-fg ${focusRing}`

export function AppShell({ session, children }: { session: Session; children: ReactNode }) {
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
          <UserMenu session={session} />
        </div>
      </header>
      <main id="main" className="mx-auto w-full max-w-6xl flex-1 px-4 py-8">
        {children}
      </main>
    </div>
  )
}
