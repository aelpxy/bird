import { z } from 'zod'
import { splitCommand } from './split-command'

// birdd's own rules, checked here first so mistakes show next to the field instead of after a round trip

export const NAME_HINT = 'Lowercase letters, digits and dashes, starting with a letter.'
const NAME = /^[a-z][a-z0-9-]{0,62}$/
const HOSTNAME = /^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.?$/
const REGISTRY_HOST = /^(localhost|([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)(:[0-9]{1,5})?$/
const ENV_KEY = /^[A-Za-z_][A-Za-z0-9_]{0,254}$/
const IMAGE = /^[A-Za-z0-9][A-Za-z0-9._\-/:@]{0,511}$/

export const MIN_PASSWORD_CHARS = 12
export const MAX_PASSWORD_CHARS = 256

export const name = (missing = 'Enter a name.') => z.string().trim().min(1, missing).regex(NAME, NAME_HINT)

export const optionalName = z
  .string()
  .trim()
  .refine((value) => value === '' || NAME.test(value), NAME_HINT)

export const hostname = z
  .string()
  .trim()
  .toLowerCase()
  .min(1, 'Enter a domain.')
  .refine((value) => HOSTNAME.test(value), 'Enter a full domain, like app.example.com.')

export const optionalHostname = z
  .string()
  .trim()
  .toLowerCase()
  .refine((value) => value === '' || HOSTNAME.test(value), 'Enter a full domain, like app.example.com.')

export const registryHost = z
  .string()
  .trim()
  .toLowerCase()
  .min(1, "Enter the registry's host.")
  .refine((value) => REGISTRY_HOST.test(value), 'Just the host, like ghcr.io or registry.example.com:5000.')

export const envKey = z
  .string()
  .trim()
  .min(1, 'Enter a name.')
  .regex(ENV_KEY, 'Letters, digits and underscores, not starting with a digit.')

export const image = z
  .string()
  .trim()
  .min(1, 'Enter an image.')
  .regex(IMAGE, 'Letters, digits and . _ - / : @, like ghcr.io/acme/web:1.4.')

export const password = z
  .string()
  .min(MIN_PASSWORD_CHARS, `Use at least ${MIN_PASSWORD_CHARS} characters.`)
  .max(MAX_PASSWORD_CHARS, `Use at most ${MAX_PASSWORD_CHARS} characters.`)

export const required = (message: string) => z.string().min(1, message)

// a whole number in a range, as birdd accepts it
export const wholeNumber = (min: number, max: number, what: string) =>
  z
    .number({ error: `Enter ${what}.` })
    .int(`Enter a whole number for ${what}.`)
    .min(min, `${what[0].toUpperCase()}${what.slice(1)} goes from ${min} to ${max}.`)
    .max(max, `${what[0].toUpperCase()}${what.slice(1)} goes from ${min} to ${max}.`)

export const port = wholeNumber(1, 65_535, 'the port')

// a command typed like in a shell, split into the arguments birdd runs
export const command = z.string().transform((value, context) => {
  const split = splitCommand(value)
  if (!split.ok) {
    context.addIssue({ code: 'custom', message: split.error })
    return z.NEVER
  }
  return split.args
})

// where a volume appears inside the container
export const mountPath = z
  .string()
  .trim()
  .min(1, 'Enter where the volume is mounted.')
  .refine((value) => value.startsWith('/') && value.length > 1, 'An absolute path, like /var/lib/app.')
  .refine((value) => value.length <= 255, 'Use at most 255 characters.')
  .refine((value) => !value.split('/').includes('..'), 'The path cannot contain "..".')
  .refine((value) => /^[!-~]+$/.test(value) && !/[:,]/.test(value), 'No spaces, colons or commas.')

export const volume = z.object({ name: name('Name the volume.'), path: mountPath })
