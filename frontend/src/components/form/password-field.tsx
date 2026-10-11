import { Eye, EyeOff } from 'lucide-react'
import { useState, type ComponentProps } from 'react'
import { IconButton } from '@/components/ui/icon-button'
import { TextField } from './text-field'

type PasswordFieldProps = Omit<ComponentProps<typeof TextField>, 'type' | 'trailing' | 'mono'>

export function PasswordField(props: PasswordFieldProps) {
  const [visible, setVisible] = useState(false)
  const Icon = visible ? EyeOff : Eye
  return (
    <TextField
      {...props}
      type={visible ? 'text' : 'password'}
      trailing={
        <IconButton label={visible ? 'Hide password' : 'Show password'} aria-pressed={visible} onClick={() => setVisible((shown) => !shown)}>
          <Icon aria-hidden className="size-4" />
        </IconButton>
      }
    />
  )
}
