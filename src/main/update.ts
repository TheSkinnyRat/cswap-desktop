import { net } from 'electron'
import type { UpdateInfo } from '@shared/types'

const RELEASES_API = 'https://api.github.com/repos/TheSkinnyRat/cswap-desktop/releases/latest'
const RELEASES_URL = 'https://github.com/TheSkinnyRat/cswap-desktop/releases/latest'

function newer(latest: string, current: string): boolean {
  const a = latest.replace(/^v/, '').split('.').map(Number)
  const b = current.replace(/^v/, '').split('.').map(Number)
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const x = a[i] ?? 0
    const y = b[i] ?? 0
    if (x !== y) return x > y
  }
  return false
}

// A manual "check for updates": read the latest release tag, compare, link to it.
// No auto-download — builds are unsigned, so the user installs by hand.
export async function checkForUpdate(current: string): Promise<UpdateInfo> {
  try {
    const res = await net.fetch(RELEASES_API, { headers: { Accept: 'application/vnd.github+json', 'User-Agent': 'cswap-desktop' } })
    if (!res.ok) return { current, latest: null, url: RELEASES_URL, updateAvailable: false, error: `GitHub answered ${res.status}` }
    const json = (await res.json()) as { tag_name?: string; html_url?: string }
    const latest = json.tag_name?.replace(/^v/, '') ?? null
    return { current, latest, url: json.html_url ?? RELEASES_URL, updateAvailable: !!latest && newer(latest, current) }
  } catch (e) {
    return { current, latest: null, url: RELEASES_URL, updateAvailable: false, error: (e as Error).message }
  }
}
