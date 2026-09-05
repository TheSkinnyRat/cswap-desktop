import type { CswapApi } from '@shared/types'
import { createMockApi } from './mock'

// In Electron the preload exposes window.cswap. In a plain browser (vite dev
// for UI work, screenshots) fall back to an in-memory mock with the same shape.
export const api: CswapApi = typeof window !== 'undefined' && window.cswap ? window.cswap : createMockApi()
export const isElectron = typeof window !== 'undefined' && !!window.cswap
export const platform: NodeJS.Platform = (typeof window !== 'undefined' && window.platform) || 'linux'
