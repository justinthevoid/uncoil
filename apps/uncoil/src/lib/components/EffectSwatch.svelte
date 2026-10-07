<script lang="ts">
	// An effect card's swatch: the effect itself, running on a small keyboard of 15 × 5 keys laid over a
	// keyboard's real extent, rendered by the same effect code as the desk preview (lib/effect.ts). Reactive
	// and ripple get simulated presses, the audio meter a made-up level. Still with reduced motion.
	import { onMount } from 'svelte';
	import { frameWith, type Inputs } from '#lib/effect.ts';
	import { onTick } from '#lib/ticker.ts';
	import type { LayerEffect, PreviewPress } from '#lib/types.ts';

	let { effect: fx }: { effect: LayerEffect } = $props();

	const COLS = 15;
	const ROWS = 5;
	const KB = { w: 16.25, h: 6.25 };
	const cell = (c: number, r: number): [number, number] => [((c + 0.5) / COLS) * KB.w, ((r + 0.5) / ROWS) * KB.h];

	let canvas: HTMLCanvasElement | undefined = $state();
	let presses: PreviewPress[] = [];
	let nextPress = 0;

	function draw(t: number) {
		const cv = canvas;
		if (!cv) return;
		const dpr = Math.min(devicePixelRatio || 1, 2);
		const w = cv.clientWidth;
		const h = cv.clientHeight;
		if (!w || !h) return;
		if (cv.width !== Math.round(w * dpr) || cv.height !== Math.round(h * dpr)) {
			cv.width = Math.round(w * dpr);
			cv.height = Math.round(h * dpr);
		}
		const g = cv.getContext('2d');
		if (!g) return;
		if (fx.kind === 'reactive' || fx.kind === 'ripple') {
			presses = presses.filter((p) => t - p.t < 3);
			if (t >= nextPress) {
				const [x, y] = cell(Math.floor(Math.random() * COLS), Math.floor(Math.random() * ROWS));
				presses.push({ x, y, t });
				nextPress = t + 0.35 + Math.random() * 0.45;
			}
		}
		const inputs: Inputs = {
			presses,
			audio: Math.max(0, Math.min(1, 0.5 + 0.3 * Math.sin(t * 2.3) + 0.15 * Math.sin(t * 7.1))),
			bounds: { minX: 0, minY: 0, maxX: KB.w, maxY: KB.h },
			keyboardCenter: [KB.w / 2, KB.h / 2]
		};
		const at = frameWith($state.snapshot(fx) as LayerEffect, t, 1, 1, inputs);
		g.setTransform(dpr, 0, 0, dpr, 0, 0);
		g.fillStyle = '#141312';
		g.fillRect(0, 0, w, h);
		const gap = 1.5;
		const kw = (w - gap * (COLS + 1)) / COLS;
		const kh = (h - gap * (ROWS + 1)) / ROWS;
		for (let r = 0; r < ROWS; r++) {
			for (let c = 0; c < COLS; c++) {
				const [x, y] = cell(c, r);
				const [cr, cg, cb] = at('', '', x, y);
				g.fillStyle = `rgb(${cr} ${cg} ${cb})`;
				g.beginPath();
				g.roundRect(gap + c * (kw + gap), gap + r * (kh + gap), kw, kh, 1.5);
				g.fill();
			}
		}
	}

	onMount(() => onTick(draw));
	// Settings changed while paused or with reduced motion: show them anyway.
	$effect(() => {
		$state.snapshot(fx);
		draw(6);
	});
</script>

<canvas bind:this={canvas} aria-hidden="true"></canvas>

<style>
	canvas {
		display: block;
		width: 100%;
		height: 100%;
		border-radius: var(--radius-sm);
	}
</style>
