import { afterEach, describe, expect, it } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { labelFor, readActivePlan, PlanStore } from '../../src/main/plan'

let dir: string | null = null
afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true })
  dir = null
  delete process.env.CLAUDE_CONFIG_DIR
})

function withCredentials(oauth: unknown): string {
  dir = mkdtempSync(join(tmpdir(), 'cswap-plan-'))
  writeFileSync(join(dir, '.credentials.json'), JSON.stringify({ claudeAiOauth: oauth }))
  process.env.CLAUDE_CONFIG_DIR = dir
  return dir
}

describe('labelFor', () => {
  it('names the known tiers', () => {
    expect(labelFor('default_claude_max_5x', 'max')).toBe('Max 5x')
    expect(labelFor('default_claude_max_20x', 'max')).toBe('Max 20x')
    expect(labelFor('default_claude_pro', 'pro')).toBe('Pro')
    expect(labelFor('default_claude_free', 'free')).toBe('Free')
  })
  it('makes an unknown tier readable instead of dropping it', () => {
    expect(labelFor('default_claude_max_50x', 'max')).toBe('Max 50x')
    expect(labelFor('some_new_tier', undefined)).toBe('Some New Tier')
  })
  it('falls back to the subscription, then to nothing', () => {
    expect(labelFor(undefined, 'max')).toBe('Max')
    expect(labelFor(undefined, 'startup')).toBe('Startup')
    expect(labelFor(undefined, undefined)).toBeNull()
  })
})

describe('readActivePlan', () => {
  it('reads the two plan fields and no others', () => {
    withCredentials({ accessToken: 'sk-secret', refreshToken: 'sk-secret', subscriptionType: 'max', rateLimitTier: 'default_claude_max_20x' })
    const plan = readActivePlan()
    expect(plan).toEqual({ tier: 'default_claude_max_20x', subscription: 'max', label: 'Max 20x' })
    expect(JSON.stringify(plan)).not.toContain('secret')
  })
  it('returns null for a missing, unreadable or plan-less file', () => {
    process.env.CLAUDE_CONFIG_DIR = join(tmpdir(), 'cswap-plan-does-not-exist')
    expect(readActivePlan()).toBeNull()
    dir = mkdtempSync(join(tmpdir(), 'cswap-plan-'))
    writeFileSync(join(dir, '.credentials.json'), 'not json at all')
    process.env.CLAUDE_CONFIG_DIR = dir
    expect(readActivePlan()).toBeNull()
    writeFileSync(join(dir, '.credentials.json'), JSON.stringify({ claudeAiOauth: { accessToken: 'x' } }))
    expect(readActivePlan()).toBeNull()
  })
})

describe('PlanStore', () => {
  it('remembers the plan per email and survives a restart', () => {
    const home = withCredentials({ subscriptionType: 'max', rateLimitTier: 'default_claude_max_5x' })
    const store = new PlanStore(home)
    store.observe('me@example.com')
    expect(store.all()['me@example.com']).toMatchObject({ label: 'Max 5x' })
    // a different account is live now
    writeFileSync(join(home, '.credentials.json'), JSON.stringify({ claudeAiOauth: { subscriptionType: 'pro', rateLimitTier: 'default_claude_pro' } }))
    store.observe('other@example.com')
    expect(store.all()['other@example.com'].label).toBe('Pro')
    expect(store.all()['me@example.com'].label).toBe('Max 5x') // the one switched away from keeps its label
    expect(new PlanStore(home).all()['me@example.com'].label).toBe('Max 5x')
  })
  it('records nothing when there is no active account or no plan on disk', () => {
    const home = withCredentials({ subscriptionType: 'max', rateLimitTier: 'default_claude_max_5x' })
    const store = new PlanStore(home)
    store.observe(null)
    store.observe(undefined)
    expect(store.all()).toEqual({})
  })
})
