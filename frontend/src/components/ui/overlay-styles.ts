// dialogs, menus and toasts: hard edges and a hard shadow, appearing quickly

export const backdropClass =
  'fixed inset-0 z-40 bg-black/70 transition-opacity duration-150 data-ending-style:opacity-0 data-starting-style:opacity-0'

const popupBase =
  'fixed top-1/2 left-1/2 z-50 flex max-h-[calc(100dvh-2rem)] max-w-[calc(100vw-2rem)] -translate-x-1/2 -translate-y-1/2 flex-col overflow-y-auto border border-line-strong bg-surface transition-[scale,opacity] duration-150 ease-out data-ending-style:scale-[0.98] data-ending-style:opacity-0 data-starting-style:scale-[0.98] data-starting-style:opacity-0'

export const dialogWidths = { md: 'w-[30rem]', lg: 'w-[38rem]', xl: 'w-[52rem]' } as const

export function popupClass(width: keyof typeof dialogWidths = 'md'): string {
  return `${popupBase} ${dialogWidths[width]}`
}

export const menuPopupClass =
  'min-w-52 origin-[var(--transform-origin)] border border-line-strong bg-surface p-1 outline-hidden transition-[scale,opacity] duration-100 ease-out data-ending-style:scale-[0.98] data-ending-style:opacity-0 data-starting-style:scale-[0.98] data-starting-style:opacity-0'

export const menuItemClass =
  'flex cursor-default items-center gap-2 px-2.5 py-1.5 text-sm text-fg-muted outline-hidden select-none data-highlighted:bg-raised data-highlighted:text-fg'

export const menuSeparatorClass = 'mx-1 my-1 h-px bg-line'

// near the top, so the results grow downwards as they are typed for
export const palettePopupClass =
  'fixed top-[15dvh] left-1/2 z-50 flex max-h-[70dvh] w-[38rem] max-w-[calc(100vw-2rem)] -translate-x-1/2 flex-col overflow-hidden border border-line-strong bg-surface transition-[scale,opacity] duration-150 ease-out data-ending-style:scale-[0.98] data-ending-style:opacity-0 data-starting-style:scale-[0.98] data-starting-style:opacity-0'
