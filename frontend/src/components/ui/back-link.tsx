import { Link, type LinkComponentProps } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'
import type { ReactNode } from 'react'
import { focusRing } from './control-styles'

type BackLinkProps = Omit<LinkComponentProps, 'children' | 'className'> & { children: ReactNode }

export function BackLink({ children, ...link }: BackLinkProps) {
  return (
    <Link
      {...link}
      className={`inline-flex items-center gap-1.5 self-start font-mono text-2xs tracking-[0.08em] text-fg-faint uppercase transition-colors hover:text-fg ${focusRing}`}
    >
      <ArrowLeft aria-hidden className="size-3.5" />
      {children}
    </Link>
  )
}
