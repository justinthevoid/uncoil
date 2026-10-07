// The live preview loop shared by Lighting and Studio: one effect clock, live colours from the real
// engine (or the mock), and simulated inputs so reactive, ripple and audio effects can be previewed.
import { getDesk, previewFrame } from './api';
import { kindsNeeding } from './effects';
import { reducedMotion } from './motion';
import { deskSources } from './state.svelte';
import type { Config, DeskDevice, Effect, PreviewPress } from './types';

const usesKind = (e: Effect, kinds: string[]): boolean => (e.kind === 'studio' ? e.layers.some((l) => l.enabled && kinds.includes(l.effect.kind)) : kinds.includes(e.kind));
const KEYS = kindsNeeding('keys');
const AUDIO = kindsNeeding('audio');

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
	/** Bumped by anything that changes the frame; a paused preview draws only when it moved past `drawn`. */
	let changes = 1;
	let drawn = 0;
	const time = () => {
		const now = performance.now();
		if (!state.paused) clock += (now - lastNow) / 1000;
		lastNow = now;
		return clock;
	};

	/** A press at a desk position (from a click on the preview). */
	function press(x: number, y: number) {
		presses = [...presses, { x, y, t: clock }].slice(-24);
		changes++;
	}

	function simulate(t: number, cfg: Config) {
		presses = presses.filter((p) => t - p.t < 5);
		if (!state.simulateTyping || state.paused || !usesKind(cfg.effect, KEYS) || t < nextSim) return;
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
		// The config as last seen: its JSON is the change key, and the parsed copy is reused while it holds, so a
		// tick costs one stringify rather than a deep snapshot.
		let json = '';
		let cfg: Config | null = null;
		const current = (): Config => {
			const j = JSON.stringify(getConfig());
			if (j !== json || !cfg) {
				json = j;
				cfg = JSON.parse(j) as Config;
				changes++;
			}
			return cfg;
		};
		// The desk follows what is connected (an experimental device with a layout joins it when it connects, and
		// the devices OpenRGB reports join it as the PC column) and where things are placed. The old desk stays up
		// until the new one arrives; a failed fetch (the engine not up yet) is tried again a second later.
		let src = deskSources();
		let deskKey = '';
		let retryAt = 0;
		const tick = async () => {
			if (busy || (document.hidden && state.colors.length)) return;
			busy = true;
			try {
				const c = current();
				const s = deskSources();
				const want = `${s.key}|${JSON.stringify(c.desk ?? {})}`;
				if (want !== deskKey && performance.now() >= retryAt) {
					try {
						const d = await getDesk(c, s.connected, s.external);
						if (!alive) return;
						state.desk = d;
						src = s;
						deskKey = want;
						changes++;
					} catch {
						retryAt = performance.now() + 1000;
					}
				}
				if (!deskKey) return;
				const t = time();
				// Paused, a frame only changes with the config, the desk or a press.
				if (state.paused && drawn === changes) return;
				const v = changes;
				simulate(t, c);
				const f = await previewFrame(c, t, presses, usesKind(c.effect, AUDIO) ? audioAt(t) : 0, src.connected, src.external);
				if (!alive) return;
				state.colors = f;
				drawn = v;
			} catch {
				// keep the last frame; the next tick tries again
			} finally {
				busy = false;
			}
		};
		tick();
		const id = setInterval(tick, 40);
		return () => {
			alive = false;
			clearInterval(id);
		};
	}

	return {
		state,
		start,
		press,
		uses: (kinds: string[]) => usesKind(getConfig().effect, kinds),
		/** Does the effect react to key presses or audio (from the effect catalogue)? */
		needs: (input: 'keys' | 'audio') => usesKind(getConfig().effect, input === 'keys' ? KEYS : AUDIO)
	};
}
