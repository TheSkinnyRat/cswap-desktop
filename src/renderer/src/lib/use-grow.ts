import { useEffect, useState } from 'react'

/**
 * A reading that arrives by growing into place.
 *
 * A CSS transition needs a *change* of value, and a first render has none — a meter
 * mounted at its figure is simply drawn there, however carefully its transition is
 * written. This starts at zero and moves to the figure on the next frame, which is a
 * change, so the transition already on the element does the rest.
 *
 * Someone who asked for less motion gets the figure immediately: there is no point
 * starting a journey whose transition is switched off, since it would jump from zero.
 */
export function useGrow(target: number): number {
  const [shown, setShown] = useState(() => {
    try {
      return window.matchMedia('(prefers-reduced-motion: reduce)').matches ? target : 0
    } catch {
      return 0
    }
  })
  useEffect(() => {
    const id = requestAnimationFrame(() => setShown(target))
    return () => cancelAnimationFrame(id)
  }, [target])
  return shown
}
