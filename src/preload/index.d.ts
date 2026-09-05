import type { CswapApi } from '../shared/types'

declare global {
  interface Window {
    cswap: CswapApi
    platform: NodeJS.Platform
  }
}
export {}
