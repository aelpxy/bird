import { describe, expect, it } from 'vitest'
import { command, envKey, hostname, image, name, optionalHostname, password, port, registryHost } from './validation'

const message = (result: { success: boolean; error?: { issues: { message: string }[] } }) => result.error?.issues[0]?.message

describe('validation', () => {
  it('accepts names birdd accepts and explains the rule otherwise', () => {
    expect(name().safeParse('web-1').success).toBe(true)
    expect(message(name().safeParse('Web'))).toBe('Lowercase letters, digits and dashes, starting with a letter.')
    expect(message(name('Enter a project name.').safeParse('  '))).toBe('Enter a project name.')
  })

  it('normalizes domains and wants two labels', () => {
    expect(hostname.parse(' App.Example.com ')).toBe('app.example.com')
    expect(hostname.safeParse('localhost').success).toBe(false)
    expect(optionalHostname.parse('')).toBe('')
  })

  it('checks the other values birdd validates', () => {
    expect(registryHost.parse('Registry.example.com:5000')).toBe('registry.example.com:5000')
    expect(registryHost.safeParse('https://ghcr.io').success).toBe(false)
    expect(envKey.safeParse('1BAD').success).toBe(false)
    expect(image.safeParse('nginx:alpine').success).toBe(true)
    expect(image.safeParse('nginx alpine').success).toBe(false)
    expect(password.safeParse('short').success).toBe(false)
  })

  it('reads ports as whole numbers in range', () => {
    expect(port.safeParse(8080).success).toBe(true)
    expect(message(port.safeParse(Number.NaN))).toBe('Enter the port.')
    expect(message(port.safeParse(70_000))).toBe('The port goes from 1 to 65535.')
  })

  it('splits commands like a shell', () => {
    expect(command.parse('psql -c "select 1"')).toEqual(['psql', '-c', 'select 1'])
    expect(message(command.safeParse('echo "open'))).toBe('A double quote is never closed.')
  })
})

describe('mount paths', () => {
  it('accepts absolute paths birdd can mount', async () => {
    const { mountPath } = await import('./validation')
    expect(mountPath.safeParse('/var/lib/postgresql/data').success).toBe(true)
    for (const bad of ['data', '/', '/a/../b', '/with space', '/a:b', '/a,b']) {
      expect(mountPath.safeParse(bad).success, bad).toBe(false)
    }
  })
})
