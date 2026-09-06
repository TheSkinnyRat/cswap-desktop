import { useCallback, useEffect, useState } from 'react'

/**
 * The width of an element, watched.
 *
 * The accounts list changes shape — table or cards — and what decides is how much room
 * the list itself has, not how wide the window is: the sidebar, the scrollbar and the
 * page padding all sit between the two.
 *
 * A callback ref rather than a RefObject, deliberately: the list is not in the tree on
 * the first render (the skeleton is), so an effect that reads `ref.current` at mount
 * finds null, observes nothing, and never runs again — the width stays 0 and the layout
 * simply never changes, with nothing to see in the console.
 */
export function useElementWidth(): [(el: HTMLElement | null) => void, number] {
  const [node, setNode] = useState<HTMLElement | null>(null)
  const [width, setWidth] = useState(0)
  const ref = useCallback((el: HTMLElement | null) => setNode(el), [])
  useEffect(() => {
    if (!node) return
    setWidth(node.getBoundingClientRect().width)
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) setWidth(e.contentRect.width)
    })
    ro.observe(node)
    return () => ro.disconnect()
  }, [node])
  return [ref, width]
}

/** The window's own width, watched — for chrome that reacts to the window, not to a box. */
export function useWindowWidth(): number {
  const [w, setW] = useState(() => (typeof window === 'undefined' ? 0 : window.innerWidth))
  useEffect(() => {
    const on = (): void => setW(window.innerWidth)
    window.addEventListener('resize', on)
    return () => window.removeEventListener('resize', on)
  }, [])
  return w
}
