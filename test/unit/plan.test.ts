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
  it('names the plan from the subscription, not from the tier string', () => {
    // the tier for a Pro account is not "default_claude_pro"; reading it as the label
    // is what put "Ai" on Purwa's badge
    expect(labelFor('default_claude_ai', 'pro')).toBe('Pro')
    expect(labelFor('some_internal_name', 'pro')).toBe('Pro')
    expect(labelFor(undefined, 'pro')).toBe('Pro')
    expect(labelFor('anything', 'free')).toBe('Free')
    expect(labelFor('anything', 'team')).toBe('Team')
    expect(labelFor('anything', 'enterprise')).toBe('Enterprise')
  })
  it('reads the multiplier out of the tier, but only for Max', () => {
    expect(labelFor('default_claude_max_5x', 'max')).toBe('Max 5x')
    expect(labelFor('default_claude_max_20x', 'max')).toBe('Max 20x')
    expect(labelFor('default_claude_max_50x', 'max')).toBe('Max 50x') // a tier that does not exist yet
    expect(labelFor('something_else', 'max')).toBe('Max') // unknown shape: still honest
    expect(labelFor('default_claude_ai_5x', 'pro')).toBe('Pro') // a multiplier on a Pro tier is not a Max
  })
  it('falls back to the tier only when it names a plan, and to nothing otherwise', () => {
    expect(labelFor('default_claude_max_20x', undefined)).toBe('Max 20x')
    expect(labelFor('default_claude_pro', undefined)).toBe('Pro')
    expect(labelFor('default_claude_ai', undefined)).toBeNull()
    expect(labelFor(undefined, undefined)).toBeNull()
  })
  it('shows both when the seat runs at a different tier than the subscription names', () => {
    expect(labelFor('default_claude_max_20x', 'team')).toBe('Team · Max 20x')
    expect(labelFor('default_claude_max_5x', 'team')).toBe('Team · Max 5x')
    expect(labelFor('default_claude_pro', 'team')).toBe('Team · Pro')
    expect(labelFor('default_claude_max_20x', 'enterprise')).toBe('Enterprise · Max 20x')
    // nothing to add: the tier names no plan, or names the same one
    expect(labelFor('default_claude_ai', 'team')).toBe('Team')
    expect(labelFor('default_claude_pro', 'pro')).toBe('Pro')
    expect(labelFor('default_claude_team', 'team')).toBe('Team')
  })

  it('titles an unfamiliar subscription rather than dropping it', () => {
    expect(labelFor(undefined, 'startup')).toBe('Startup')
    expect(labelFor(undefined, 'MAX')).toBe('Max')
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
