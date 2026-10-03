<script lang="ts">
	// The signature: the desk's effect drawn as a stack of white hairline pulses (after Unknown Pleasures).
	// Each line is one horizontal slice of the desk, back to front. Ridges are the wave's colour bands, so
	// angle, band width and speed are visible in the plot itself; a line only rises where LEDs actually
	// sit (the envelope is the real LED density of the placed devices). One slice is decoded into its live
	// colour: the one under the pointer, else the keyboard's home row.
	import { onMount } from 'svelte';
	import type { Config, DeskDevice } from '#lib/types.ts';
	import { frame, phaseAt } from '#lib/effect.ts';

	interface Props {
		desk: DeskDevice[];
		config: Config;
		paused?: boolean;
		/** Effect clock in seconds; the parent owns it so every view stays in step. */
		time: () => number;
	}
	let { desk, config, paused = false, time }: Props = $props();

	const LINES = 30;
	const SAMPLES = 240;

	let host: HTMLDivElement;
	let canvas: HTMLCanvasElement;
	let w = 0;
	let h = 0;
	let pointerY: number | null = null;
	let decodedRow = 0; // eased index of the decoded slice

	// Desk bounds (the mat when there is one) and an LED density field over it.
	const bounds = $derived.by(() => {
		const mat = desk.find((d) => d.kind === 'mousemat');
		if (mat) return { x: mat.x, y: mat.y, w: mat.w, h: mat.h };
		const xs = desk.flatMap((d) => [d.x, d.x + d.w]);
		const ys = desk.flatMap((d) => [d.y, d.y + d.h]);
		return { x: Math.min(...xs), y: Math.min(...ys), w: Math.max(...xs) - Math.min(...xs), h: Math.max(...ys) - Math.min(...ys) };
	});

	const density = $derived.by(() => {
		const b = bounds;
		const grid = new Float32Array(LINES * SAMPLES);
		const leds = desk.filter((d) => d.kind !== 'mousemat').flatMap((d) => d.shapes);
		for (let r = 0; r < LINES; r++) {
			const y = b.y + ((r + 0.5) / LINES) * b.h;
			for (let s = 0; s < SAMPLES; s++) {
				const x = b.x + (s / (SAMPLES - 1)) * b.w;
				let v = 0;
				for (const l of leds) {
					const sx = Math.max(l.w * 0.6, 0.55);
					const sy = Math.max(l.h * 0.6, 0.55);
					const dx = (x - l.x) / sx;
					const dy = (y - l.y) / sy;
					const q = dx * dx + dy * dy;
					if (q < 9) v += Math.exp(-q);
				}
				grid[r * SAMPLES + s] = v;
			}
		}
		let max = 0;
		for (const v of grid) max = Math.max(max, v);
		for (let i = 0; i < grid.length; i++) grid[i] = 1 - Math.exp((-2.2 * grid[i]) / (max || 1));
		return grid;
	});

	// Default decoded slice: the keyboard's home row (A..L), else the middle of the desk.
	const homeRow = $derived.by(() => {
		const kb = desk.find((d) => d.kind === 'keyboard');
		const a = kb?.shapes.find((s) => s.name === 'A');
		const y = a ? a.y : bounds.y + bounds.h / 2;
		return Math.round(((y - bounds.y) / bounds.h) * LINES - 0.5);
	});

	function draw() {
		const ctx = canvas.getContext('2d');
		if (!ctx || !w || !h) return;
		const dpr = devicePixelRatio || 1;
		ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
		ctx.clearRect(0, 0, w, h);

		const b = bounds;
		const padX = 28;
		const top = 40;
		const bottom = 22;
		const plotW = w - padX * 2;
		const spacing = (h - top - bottom) / LINES;
		const amp = spacing * 3.1;
		const t = time();
		const kind = config.effect.kind;
		const phase = phaseAt(config.effect, t);
		const color = frame(config.effect, t, config.saturation, Math.max(config.brightness, 0.25));
		const level = kind === 'off' ? 0 : kind === 'static' ? 0.42 : 1;
		const bright = 0.35 + 0.65 * config.brightness;

		// ease the decoded slice toward the pointer (or home row)
		const target = pointerY == null ? homeRow : Math.max(0, Math.min(LINES - 1, Math.round((pointerY - top) / spacing - 0.5)));
		decodedRow += (target - decodedRow) * 0.22;
		const decoded = Math.round(decodedRow);

		for (let r = 0; r < LINES; r++) {
			const base = top + (r + 1) * spacing;
			const y = b.y + ((r + 0.5) / LINES) * b.h;
			ctx.beginPath();
			for (let s = 0; s < SAMPLES; s++) {
				const x = b.x + (s / (SAMPLES - 1)) * b.w;
				const env = density[r * SAMPLES + s];
				const ridge = kind === 'static' ? 1 : 0.5 + 0.5 * Math.cos(phase(x, y) * Math.PI * 2);
				const py = base - env * level * amp * (0.18 + 0.82 * ridge);
				const px = padX + (s / (SAMPLES - 1)) * plotW;
				if (s) ctx.lineTo(px, py);
				else ctx.moveTo(px, py);
			}
			// occlude the lines behind, like ink over paper
			ctx.lineTo(padX + plotW, base + spacing * 2);
			ctx.lineTo(padX, base + spacing * 2);
			ctx.closePath();
			ctx.fillStyle = '#0b0b0b';
			ctx.fill();

			if (r === decoded && kind !== 'off') {
				const g = ctx.createLinearGradient(padX, 0, padX + plotW, 0);
				for (let k = 0; k <= 16; k++) {
					const x = b.x + (k / 16) * b.w;
					const [cr, cg, cb] = color(x, y);
					g.addColorStop(k / 16, `rgb(${cr},${cg},${cb})`);
				}
				ctx.strokeStyle = g;
				ctx.lineWidth = 2;
			} else {
				const a = kind === 'off' ? 0.22 : bright * 0.92;
				ctx.strokeStyle = `rgba(242,242,242,${a})`;
				ctx.lineWidth = 1;
			}
			// stroke only the top edge (re-trace without the closing segments)
			ctx.save();
			ctx.beginPath();
			for (let s = 0; s < SAMPLES; s++) {
				const x = b.x + (s / (SAMPLES - 1)) * b.w;
				const env = density[r * SAMPLES + s];
				const ridge = kind === 'static' ? 1 : 0.5 + 0.5 * Math.cos(phase(x, y) * Math.PI * 2);
				const py = base - env * level * amp * (0.18 + 0.82 * ridge);
				const px = padX + (s / (SAMPLES - 1)) * plotW;
				if (s) ctx.lineTo(px, py);
				else ctx.moveTo(px, py);
			}
			ctx.stroke();
			ctx.restore();

			if (r === decoded && kind !== 'off') {
				ctx.fillStyle = '#e2372c';
				ctx.fillRect(padX - 18, base - 4, 8, 8);
			}
		}
	}

	onMount(() => {
		const ro = new ResizeObserver(() => {
			const r = host.getBoundingClientRect();
			w = r.width;
			h = r.height;
			const dpr = devicePixelRatio || 1;
			canvas.width = Math.round(w * dpr);
			canvas.height = Math.round(h * dpr);
			draw();
		});
		ro.observe(host);
		let raf = 0;
		let last = 0;
		const loop = (now: number) => {
			raf = requestAnimationFrame(loop);
			if (document.hidden || now - last < 1000 / 40) return;
			last = now;
			draw();
		};
		raf = requestAnimationFrame(loop);
		return () => {
			ro.disconnect();
			cancelAnimationFrame(raf);
		};
	});

	// redraw immediately on any settings change, even while paused
	$effect(() => {
		JSON.stringify(config);
		void paused;
		draw();
	});
</script>

<div
	class="plot"
	bind:this={host}
	role="img"
	aria-label="Pulse plot of the current effect across the desk"
	onpointermove={(e) => (pointerY = e.clientY - host.getBoundingClientRect().top)}
	onpointerleave={() => (pointerY = null)}
>
	<canvas bind:this={canvas}></canvas>
</div>

<style>
	.plot {
		position: relative;
		width: 100%;
		height: 100%;
		min-height: 0;
		cursor: crosshair;
	}
	canvas {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}
</style>
