import type { Transition } from 'motion/react'

// the house spring: critically damped, so nothing overshoots unless a gesture threw it
export const spring: Transition = { type: 'spring', bounce: 0, duration: 0.35 }
