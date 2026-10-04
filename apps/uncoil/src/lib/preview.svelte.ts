// The live preview loop shared by Lighting and Studio: one effect clock, live colours from the real
// engine (or the mock), and simulated inputs so reactive, ripple and audio effects can be previewed.
import { getDesk, previewFrame } from './api';
import { reducedMotion } from './motion';
import { connectedIds } from './state.svelte';
import type { Config, DeskDevice, Effect, PreviewPress } from './types';

const usesKind = (e: Effect, kinds: string[]): boolean => (e.kind === 'studio' ? e.layers.some((l) => l.enabled && kinds.includes(l.effect.kind)) : kinds.includes(e.kind));

export function createPreview(getConfig: () => Config) {
	const state = $state({
		desk: [] as DeskDevice[],
		colors: [] as string[][],
		paused: reducedMotion(),
		/** Simulate typing on random keys for reactive / ripple previews. */
		simulateTyping: true
	});
	let clock = 6;
	let lastNow = performance.now();
	let presses: PreviewPress[] = [];
	let nextSim = 0;
	const time = () => {
		const now = performance.now();
		if (!state.paused) clock += (now - lastNow) / 1000;
		lastNow = now;
		return clock;
	};

	/** A press at a desk position (from a click on the preview). */
	function press(x: number, y: number) {
		presses = [...presses, { x, y, t: clock }].slice(-24);
	}

	function simulate(t: number, cfg: Config) {
		presses = presses.filter((p) => t - p.t < 5);
		if (!state.simulateTyping || state.paused || !usesKind(cfg.effect, ['reactive', 'ripple']) || t < nextSim) return;
		const kb = state.desk.find((d) => d.kind === 'keyboard');
		const keys = kb?.shapes.filter((s) => s.is_key) ?? [];
		if (keys.length) {
			const k = keys[Math.floor(Math.random() * keys.length)];
			presses.push({ x: k.x, y: k.y, t });
		}
		nextSim = t + 0.18 + Math.random() * 0.5;
	}
	const audioAt = (t: number) => Math.max(0, Math.min(1, 0.45 + 0.3 * Math.sin(t * 2.1) + 0.2 * Math.sin(t * 7.3 + 1) + 0.1 * Math.sin(t * 13.7)));

	function start() {
		let alive = true;
		let busy = false;
		const cfg = () => $state.snapshot(getConfig()) as Config;
		// The desk follows what is connected: an experimental device with a layout joins it when it connects.
		let deskKey = '';
		const loadDesk = async () => {
			const ids = connectedIds();
			deskKey = ids.join(',');
			const d = await getDesk(cfg(), ids);
			if (!alive) return;
			state.desk = d;
			state.colors = await previewFrame(cfg(), time(), [], 0, ids);
		};
		loadDesk();
		const id = setInterval(async () => {
			if (!state.desk.length || busy || (document.hidden && state.colors.length)) return;
			busy = true;
			try {
				if (connectedIds().join(',') !== deskKey) await loadDesk();
				const c = cfg();
				const t = time();
				simulate(t, c);
				const f = await previewFrame(c, t, presses, usesKind(c.effect, ['audio_meter']) ? audioAt(t) : 0, deskKey ? deskKey.split(',') : []);
				if (alive) state.colors = f;
			} finally {
				busy = false;
			}
		}, 40);
		return () => {
			alive = false;
			clearInterval(id);
		};
	}

	return { state, start, press, uses: (kinds: string[]) => usesKind(getConfig().effect, kinds) };
}
