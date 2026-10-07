// One animation clock for the small effect previews: at most 20 frames a second, one requestAnimationFrame
// loop however many previews are on screen, nothing while the window is hidden or motion is reduced.
import { reducedMotion } from './motion';

type Tick = (t: number) => void;
const subs = new Set<Tick>();
let raf = 0;
let last = 0;
const t0 = performance.now();
const FRAME_MS = 50;

function loop(now: number) {
	raf = requestAnimationFrame(loop);
	if (document.hidden || now - last < FRAME_MS) return;
	last = now;
	const t = (now - t0) / 1000;
	for (const f of subs) f(t);
}

/** Call `f` with the clock (seconds) on every preview frame; returns the unsubscribe. With reduced motion,
 * `f` runs once at a fixed time and never again. */
export function onTick(f: Tick): () => void {
	if (reducedMotion()) {
		f(6);
		return () => {};
	}
	subs.add(f);
	f((performance.now() - t0) / 1000);
	if (!raf) raf = requestAnimationFrame(loop);
	return () => {
		subs.delete(f);
		if (!subs.size && raf) {
			cancelAnimationFrame(raf);
			raf = 0;
		}
	};
}
