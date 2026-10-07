// Svelte's JS transitions don't read prefers-reduced-motion on their own; route every duration through this.
const query = typeof matchMedia !== 'undefined' ? matchMedia('(prefers-reduced-motion: reduce)') : null;

export const reducedMotion = () => query?.matches ?? false;

/** A transition duration in ms, or 0 when the user asked for reduced motion. */
export const ms = (duration: number) => (reducedMotion() ? 0 : duration);

/** Call `f` whenever the reduced-motion preference changes; returns the unsubscribe. */
export function onReducedMotionChange(f: (reduced: boolean) => void): () => void {
	const on = (e: MediaQueryListEvent) => f(e.matches);
	query?.addEventListener('change', on);
	return () => query?.removeEventListener('change', on);
}
