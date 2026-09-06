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
 * `rateLimitTier` is an internal string, and reading the whole of it as the label was
 * generalised from a single sample (`default_claude_max_5x`, my own account), which
 * badged a Pro account "Ai". So the tier is only ever read for what it can be trusted to
 * say: which plan it names, and the multiplier.
 *
 * A seat can be both — a Team subscription whose seat runs at Max 20x — so when the tier
 * names a different plan than the subscription does, both are shown. No Team account has
 * been seen from here, so that half is inferred from the shape of the other tiers; the
 * badge degrades to the subscription alone rather than guessing.
 */
function tierPlan(tier: string | undefined): string | null {
  if (!tier) return null
  const plan = tier.match(/(free|pro|max|team|enterprise)/i)?.[1]?.toLowerCase()
  if (!plan) return null
  const base = SUBSCRIPTION_LABELS[plan]
  if (plan !== 'max') return base
  const multiplier = tier.match(/(\d+)\s*x/i)
  return multiplier ? `Max ${multiplier[1]}x` : base
}

export function labelFor(tier: string | undefined, subscription: string | undefined): string | null {
  const sub = subscription?.trim().toLowerCase()
  const fromTier = tierPlan(tier)
  if (!sub) return fromTier
  const base = SUBSCRIPTION_LABELS[sub] ?? titled(sub)
  if (sub === 'max') {
    const multiplier = tier?.match(/(\d+)\s*x/i)
    return multiplier ? `Max ${multiplier[1]}x` : base
  }
  // The tier says something the subscription does not — a Team seat running at Max 20x.
  if (fromTier && fromTier !== base && !fromTier.startsWith(base)) return `${base} · ${fromTier}`
  return base
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
