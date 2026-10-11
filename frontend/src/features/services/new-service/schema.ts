import { z } from 'zod'
import { image, name, optionalHostname, optionalName, port, volume, wholeNumber } from '@/lib/validation'
import { buildArgs, dockerfile } from '@/features/builds/schemas'
import { parseDotenv } from '@/features/variables/dotenv'

export type Source = 'image' | 'source' | 'template'

export const MAX_REPLICAS = 32

export const defaults = {
  source: 'image' as Source,
  name: '',
  image: '',
  port: 80,
  domain: '',
  dockerfile: 'Dockerfile',
  args: '',
  template: '',
  templateName: '',
  env: '',
  replicas: 1,
  health: 'http' as 'http' | 'tcp',
  volumes: [] as { name: string; path: string }[],
}

// KEY=value lines for the service's variables; empty means none
export const variables = z.string().transform((text, context) => {
  const parsed = parseDotenv(text)
  if (!parsed.ok) {
    context.addIssue({ code: 'custom', message: parsed.error })
    return z.NEVER
  }
  return parsed.values
})

const advanced = { env: variables, replicas: wholeNumber(1, MAX_REPLICAS, 'the number of machines'), volumes: z.array(volume) }

// only the fields of the chosen source are checked, each against the rule birdd applies
const fieldsFor: Record<Source, Record<string, z.ZodType>> = {
  image: { name: name(), image, port, domain: optionalHostname, ...advanced },
  source: { name: name(), port, domain: optionalHostname, dockerfile, args: buildArgs, ...advanced },
  template: { template: z.string().min(1, 'Pick a template.'), templateName: optionalName },
}

export const schema = z
  .object({
    source: z.enum(['image', 'source', 'template']),
    name: z.string(),
    image: z.string(),
    port: z.number(),
    domain: z.string(),
    dockerfile: z.string(),
    args: z.string(),
    template: z.string(),
    templateName: z.string(),
    env: z.string(),
    replicas: z.number(),
    health: z.enum(['http', 'tcp']),
    volumes: z.array(z.object({ name: z.string(), path: z.string() })),
  })
  .superRefine((values, context) => {
    for (const [key, rule] of Object.entries(fieldsFor[values.source])) {
      const result = rule.safeParse(values[key as keyof typeof values])
      for (const issue of result.error?.issues ?? []) context.addIssue({ code: 'custom', message: issue.message, path: [key, ...issue.path] })
    }
    // only one machine may write a volume, which birdd enforces too
    if (values.source !== 'template' && values.volumes.length > 0 && values.replicas > 1) {
      context.addIssue({ code: 'custom', message: 'A service with volumes runs one machine.', path: ['replicas'] })
    }
  })

export type Values = z.output<typeof schema>
