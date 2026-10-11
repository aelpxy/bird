import { z } from 'zod'
import { required } from '@/lib/validation'
import { parseDotenv } from '../variables/dotenv'

export const dockerfile = required("Enter the Dockerfile's path.")

// KEY=VALUE lines for Dockerfile ARGs; empty means none
export const buildArgs = z.string().transform((text, context) => {
  const parsed = parseDotenv(text)
  if (!parsed.ok) {
    context.addIssue({ code: 'custom', message: parsed.error })
    return z.NEVER
  }
  return parsed.values
})
