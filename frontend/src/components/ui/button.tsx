import { Button as BaseButton } from '@base-ui/react/button'
import type { ComponentProps } from 'react'
import { buttonClass, sizes, type Variant } from './button-styles'
import { Spinner } from './spinner'

type ButtonProps = Omit<ComponentProps<typeof BaseButton>, 'className'> & {
  className?: string
  variant?: Variant
  size?: keyof typeof sizes
  // keeps the button focusable and announces the wait, instead of losing focus mid-submit
  loading?: boolean
}

export function Button({
  variant = 'primary',
  size = 'md',
  loading = false,
  disabled,
  className = '',
  children,
  ...props
}: ButtonProps) {
  return (
    <BaseButton
      {...props}
      disabled={disabled || loading}
      focusableWhenDisabled
      aria-busy={loading || undefined}
      className={`${buttonClass(variant, size)} data-disabled:cursor-not-allowed data-disabled:opacity-50 ${className}`}
    >
      {loading && <Spinner />}
      {children}
    </BaseButton>
  )
}
