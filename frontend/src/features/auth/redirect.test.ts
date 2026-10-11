import { describe, expect, it } from 'vitest'
import { safeRedirect } from './redirect'

describe('safeRedirect', () => {
  it('keeps paths on this site', () => {
    expect(safeRedirect('/projects/shop/production?tab=1#x')).toBe('/projects/shop/production?tab=1#x')
  })

  it('refuses anything that could leave the site or loop back to sign in', () => {
    for (const target of ['https://evil.example', '//evil.example', '/\\evil.example', 'projects', '/login?redirect=/', undefined, 42]) {
      expect(safeRedirect(target)).toBe('/')
    }
  })
})
