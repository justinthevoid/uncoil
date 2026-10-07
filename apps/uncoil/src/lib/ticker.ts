// One animation clock for the small effect previews: at most 20 frames a second, one requestAnimationFrame
// loop however many previews are on screen, nothing while the window is hidden or motion is reduced.
import { onReducedMotionChange, reducedMotion } from './motion';

type Tick = (t: number) => void;
const subs = new Set<Tick>();
let raf = 0;
let last = 0;
const t0 = performance.now();
const FRAME_MS = 50;
/** With reduced motion every preview shows this one moment. */
const STILL = 6;

function loop(now: number) {
	raf = requestAnimationFrame(loop);
	if (document.hidden || now - last < FRAME_MS) return;
	last = now;
	const t = (now - t0) / 1000;
	for (const f of subs) f(t);
}
function run() {
	if (subs.size && !raf && !reducedMotion()) raf = requestAnimationFrame(loop);
}
function stop() {
	if (raf) cancelAnimationFrame(raf);
	raf = 0;
}

// The preference can change while the app is open: stop on the still frame, or start moving again.
onReducedMotionChange((reduced) => {
	if (!reduced) return run();
	stop();
	for (const f of subs) f(STILL);
});

/** Call `f` with the clock (seconds) on every preview frame; returns the unsubscribe. With reduced motion,
 * `f` runs once at a fixed time and again only if motion is allowed later. */
export function onTick(f: Tick): () => void {
	subs.add(f);
	f(reducedMotion() ? STILL : (performance.now() - t0) / 1000);
	run();
	return () => {
		subs.delete(f);
		if (!subs.size) stop();
	};
}
