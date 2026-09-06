import { existsSync, readFileSync } from 'node:fs'
import { EventEmitter } from 'node:events'
import { homedir } from 'node:os'
import { dirname, join } from 'node:path'
import { writeFileSync, renameSync, mkdirSync } from 'node:fs'

// cswap exposes usage, never the subscription. The plan does sit on disk though: Claude
// Code's own credential file carries `subscriptionType` and `rateLimitTier` next to the
// tokens. This reads exactly those two strings and nothing else — no token is read into
// a variable, returned, logged or cached — and only for the account that is live right
// now (every other account's copy is encrypted inside the cswap vault). What each account
// showed while it was active is remembered by email, so the labels survive a switch.

export interface PlanInfo {
  label: string
  tier?: string
  subscription?: string
  seenAt: string
}

function credentialsPath(): string {
  const env = process.env.CLAUDE_CONFIG_DIR
  return join(env && env.trim() ? env : join(homedir(), '.claude'), '.credentials.json')
}

const TIER_LABELS: Record<string, string> = {
  default_claude_free: 'Free',
  default_claude_pro: 'Pro',
  default_claude_max_5x: 'Max 5x',
  default_claude_max_20x: 'Max 20x',
  default_claude_team: 'Team',
  default_claude_enterprise: 'Enterprise'
}
const SUBSCRIPTION_LABELS: Record<string, string> = {
  free: 'Free',
  pro: 'Pro',
  max: 'Max',
  team: 'Team',
  enterprise: 'Enterprise'
}

export function labelFor(tier: string | undefined, subscription: string | undefined): string | null {
  if (tier) {
    const known = TIER_LABELS[tier]
    if (known) return known
    // Unknown tier: make something readable rather than nothing — 'default_claude_max_50x'
    // should still say "Max 50x" the day it exists.
    const words = tier.replace(/^default_claude_/, '').split('_').filter(Boolean)
    if (words.length) {
      return words.map((w) => (/^\d+x$/i.test(w) ? w.toLowerCase() : w.charAt(0).toUpperCase() + w.slice(1))).join(' ')
    }
  }
  if (subscription) return SUBSCRIPTION_LABELS[subscription] ?? subscription.charAt(0).toUpperCase() + subscription.slice(1)
  return null
}

export function readActivePlan(): { tier?: string; subscription?: string; label: string } | null {
  const path = credentialsPath()
  if (!existsSync(path)) return null // macOS keeps these in the Keychain
  try {
    const raw = JSON.parse(readFileSync(path, 'utf8')) as { claudeAiOauth?: { subscriptionType?: unknown; rateLimitTier?: unknown } }
    const oauth = raw?.claudeAiOauth
    if (!oauth || typeof oauth !== 'object') return null
    const tier = typeof oauth.rateLimitTier === 'string' ? oauth.rateLimitTier : undefined
    const subscription = typeof oauth.subscriptionType === 'string' ? oauth.subscriptionType : undefined
    const label = labelFor(tier, subscription)
    return label ? { tier, subscription, label } : null
  } catch {
    return null // a locked, partial or unexpected file is simply "unknown"
  }
}

export class PlanStore extends EventEmitter {
  private file: string
  private data: Record<string, PlanInfo> = {}

  constructor(userData: string) {
    super()
    this.file = join(userData, 'plans.json')
    try {
      if (existsSync(this.file)) this.data = JSON.parse(readFileSync(this.file, 'utf8')) as Record<string, PlanInfo>
    } catch {
      this.data = {}
    }
  }

  all(): Record<string, PlanInfo> {
    return { ...this.data }
  }

  // Called after a refresh with the account that is live; a plan is only ever recorded
  // for the account the credential file actually belongs to.
  observe(email: string | null | undefined): void {
    if (!email) return
    const plan = readActivePlan()
    if (!plan) return
    const prev = this.data[email]
    if (prev && prev.label === plan.label && prev.tier === plan.tier) return
    this.data[email] = { ...plan, seenAt: new Date().toISOString() }
    try {
      mkdirSync(dirname(this.file), { recursive: true })
      const tmp = `${this.file}.tmp`
      writeFileSync(tmp, JSON.stringify(this.data, null, 2))
      renameSync(tmp, this.file)
    } catch {
      /* remembering is a convenience, never a failure */
    }
    this.emit('change', this.all())
  }
}
