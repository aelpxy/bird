import { describe, expect, it } from 'vitest'
import { nameFromImage } from './image-name'

describe('nameFromImage', () => {
  it('takes the repository name without registry, tag or digest', () => {
    expect(nameFromImage('ghcr.io/acme/web:1.4')).toBe('web')
    expect(nameFromImage('nginx:alpine')).toBe('nginx')
    expect(nameFromImage('docker.io/traefik/whoami@sha256:abc')).toBe('whoami')
    expect(nameFromImage('registry.example.com:5000/team/api')).toBe('api')
  })

  it('turns what birdd does not allow into dashes', () => {
    expect(nameFromImage('acme/My_App.Server:1')).toBe('my-app-server')
    expect(nameFromImage('2fa-proxy')).toBe('fa-proxy')
    expect(nameFromImage('')).toBe('')
  })
})
