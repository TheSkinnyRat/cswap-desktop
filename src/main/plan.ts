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

const SUBSCRIPTION_LABELS: Record<string, string> = {
  free: 'Free',
  pro: 'Pro',
  max: 'Max',
  team: 'Team',
  enterprise: 'Enterprise'
}

function titled(v: string): string {
  return v
    .split(/[_\s-]+/)
    .filter(Boolean)
    .map((w) => (/^\d+x$/i.test(w) ? w.toLowerCase() : w.charAt(0).toUpperCase() + w.slice(1)))
    .join(' ')
}

/**
 * The plan as a badge.
 *
 * `subscriptionType` is the field that answers the question — free / pro / max / team.
 * `rateLimitTier` is an internal string, and the multiplier is the only part of it worth
 * reading: taking the whole tier as the label was generalised from a single sample
 * (`default_claude_max_5x`, my own account) and turned a Pro account's badge into "Ai".
 * So the tier is consulted only to tell one Max apart from another.
 */
export function labelFor(tier: string | undefined, subscription: string | undefined): string | null {
  const sub = subscription?.trim().toLowerCase()
  if (sub) {
    const base = SUBSCRIPTION_LABELS[sub] ?? titled(sub)
    if (sub !== 'max') return base
    const multiplier = tier?.match(/(\d+)\s*x/i)
    return multiplier ? `Max ${multiplier[1]}x` : base
  }
  // No subscription field at all: say something only if the tier names a plan we know.
  const known = tier?.match(/(free|pro|max|team|enterprise)/i)?.[1]?.toLowerCase()
  if (!known) return null
  if (known !== 'max') return SUBSCRIPTION_LABELS[known]
  const multiplier = tier?.match(/(\d+)\s*x/i)
  return multiplier ? `Max ${multiplier[1]}x` : 'Max'
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
