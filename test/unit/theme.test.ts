import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const css = readFileSync(resolve(__dirname, '../../src/renderer/src/styles/app.css'), 'utf8')

describe('stylesheet', () => {
  it('keeps the element base rules inside @layer base', () => {
    // Unlayered CSS wins over @layer utilities whatever the specificity, so a bare
    // `button { color: inherit }` would defeat every text-* utility on every button.
    const layerStart = css.indexOf('@layer base {')
    expect(layerStart).toBeGreaterThan(-1)
    const buttonRule = css.indexOf('button,')
    expect(buttonRule).toBeGreaterThan(layerStart)
    const afterLayer = css.slice(css.indexOf('}', css.lastIndexOf('user-select: text;')))
    expect(afterLayer).not.toMatch(/^\s*(button|body|html)\s*[,{]/m)
  })

  it('gives both themes a white foreground on the accent colour', () => {
    const values = [...css.matchAll(/--accent-fg:\s*([^;]+);/g)].map((m) => m[1].trim())
    expect(values.length).toBe(2)
    for (const v of values) expect(v).toMatch(/#fff|white/)
  })
})
