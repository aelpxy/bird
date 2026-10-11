import type { ElementType, ReactNode } from 'react'
import { eyebrowClass } from './control-styles'

export function Eyebrow({ as: Tag = 'p', children, className = '' }: { as?: ElementType; children: ReactNode; className?: string }) {
  return <Tag className={`${eyebrowClass} ${className}`}>{children}</Tag>
}
