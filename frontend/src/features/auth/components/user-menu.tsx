import { Menu } from '@base-ui/react/menu'
import { useQueryClient } from '@tanstack/react-query'
import { Link, useRouter } from '@tanstack/react-router'
import { BookOpen, Building2, ChevronDown, LogOut, PackageOpen, Settings, Users } from 'lucide-react'
import { Avatar } from '@/components/ui/avatar'
import { focusRing } from '@/components/ui/control-styles'
import { Eyebrow } from '@/components/ui/eyebrow'
import { menuItemClass, menuPopupClass, menuSeparatorClass } from '@/components/ui/overlay-styles'
import { sessionQuery, signOut, type Session } from '@/features/auth/api'

export function UserMenu({ session }: { session: Session }) {
  const queryClient = useQueryClient()
  const router = useRouter()
  const isAdmin = session.role === 'admin'

  async function handleSignOut() {
    try {
      await signOut()
    } finally {
      // nothing from this account may linger for whoever uses the browser next
      queryClient.clear()
      queryClient.setQueryData(sessionQuery.queryKey, null)
      router.history.replace('/login')
    }
  }

  return (
    <Menu.Root>
      <Menu.Trigger
        className={`flex h-8 items-center gap-2 border border-transparent pr-2 pl-0.5 text-ui text-fg-muted transition-colors select-none hover:border-line hover:text-fg data-popup-open:border-line data-popup-open:bg-raised data-popup-open:text-fg ${focusRing}`}
      >
        <Avatar name={session.name} size="sm" />
        <span className="max-w-32 truncate">{session.name}</span>
        <ChevronDown aria-hidden className="size-3.5 text-fg-faint" />
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Positioner sideOffset={6} align="end" className="z-30 outline-hidden">
          <Menu.Popup className={menuPopupClass}>
            <div className="px-2.5 py-2">
              <p className="truncate text-sm font-medium text-fg">{session.name}</p>
              <Eyebrow className="mt-0.5">{session.role}</Eyebrow>
            </div>
            <Menu.Separator className={menuSeparatorClass} />
            <div className="sm:hidden">
              <Menu.Item render={<Link to="/orgs" />} className={menuItemClass}>
                <Building2 aria-hidden className="size-4" />
                Orgs
              </Menu.Item>
              {isAdmin && (
                <>
                  <Menu.Item render={<Link to="/registries" />} className={menuItemClass}>
                    <PackageOpen aria-hidden className="size-4" />
                    Registries
                  </Menu.Item>
                  <Menu.Item render={<Link to="/admin/users" />} className={menuItemClass}>
                    <Users aria-hidden className="size-4" />
                    Users
                  </Menu.Item>
                </>
              )}
              <Menu.Separator className={menuSeparatorClass} />
            </div>
            <Menu.Item render={<Link to="/account" />} className={menuItemClass}>
              <Settings aria-hidden className="size-4" />
              Account settings
            </Menu.Item>
            <Menu.Item render={<a href="/docs" target="_blank" rel="noreferrer" />} className={menuItemClass}>
              <BookOpen aria-hidden className="size-4" />
              API reference
              <span className="sr-only">(opens in a new tab)</span>
            </Menu.Item>
            <Menu.Separator className={menuSeparatorClass} />
            <Menu.Item onClick={() => void handleSignOut()} className={menuItemClass}>
              <LogOut aria-hidden className="size-4" />
              Sign out
            </Menu.Item>
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  )
}
