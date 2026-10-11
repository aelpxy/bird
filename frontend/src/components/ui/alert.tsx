import { CircleAlert } from 'lucide-react'
import { motion } from 'motion/react'
import type { ReactNode } from 'react'
import { spring } from '@/lib/motion'

// announced by screen readers as soon as it appears
export function Alert({ children }: { children: ReactNode }) {
  return (
    <motion.div
      role="alert"
      initial={{ opacity: 0, y: -4 }}
      animate={{ opacity: 1, y: 0 }}
      transition={spring}
      className="flex items-start gap-2.5 border border-bad/40 border-l-2 border-l-bad bg-bad/8 px-3 py-2.5 text-sm text-fg"
    >
      <CircleAlert aria-hidden className="mt-0.5 size-4 shrink-0 text-bad" />
      <div className="min-w-0 flex-1">{children}</div>
    </motion.div>
  )
}
