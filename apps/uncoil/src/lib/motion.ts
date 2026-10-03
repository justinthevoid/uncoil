// Svelte's JS transitions don't read prefers-reduced-motion on their own; route every duration through this.
const query = typeof matchMedia !== 'undefined' ? matchMedia('(prefers-reduced-motion: reduce)') : null;

export const reducedMotion = () => query?.matches ?? false;

/** A transition duration in ms, or 0 when the user asked for reduced motion. */
export const ms = (duration: number) => (reducedMotion() ? 0 : duration);
