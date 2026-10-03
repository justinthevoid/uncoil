<script lang="ts">
	// The signature: the desk's lighting drawn as a stack of white hairline pulses (after Unknown
	// Pleasures). Each line is one slice of the maintainer's real desk, back to front; its ridges are the
	// wave's colour bands, computed by the desktop app's own port of the engine (effect.ts), and a line
	// only rises where LEDs actually sit (desk.json is a snapshot of the real layout). One slice is decoded
	// into live colour: the one under the pointer, else the keyboard's home row.
	//
	// Runs only while on screen; with prefers-reduced-motion it draws one still frame (t = 6 s) and only
	// redraws when the pointer picks a different slice.
	import { onMount } from 'svelte';
	import desk from '$uncoil/mock/desk.json';
	import { frame, phaseAt, hex } from '$uncoil/effect';
	import type { DeskDevice, Effect } from '$uncoil/types';

	interface Props {
		/** Number of colour cells in the decoded-slice readout. */
		cells?: number;
	}
	let { cells = 24 }: Props = $props();

	// The default effect, exactly as uncoil ships it (uncoil_core::effect::Effect::default()).
	const effect: Effect = { kind: 'wave', angle_deg: 35, period_s: 14, wavelength: 26, reverse: false };
	const STILL_T = 6;

	const devices = desk as unknown as DeskDevice[];
	const mat = devices.find((d) => d.kind === 'mousemat');
	const bounds = mat
		? { x: mat.x, y: mat.y, w: mat.w, h: mat.h }
		: (() => {
				const xs = devices.flatMap((d) => [d.x, d.x + d.w]);
				const ys = devices.flatMap((d) => [d.y, d.y + d.h]);
				const x = Math.min(...xs);
				const y = Math.min(...ys);
				return { x, y, w: Math.max(...xs) - x, h: Math.max(...ys) - y };
			})();
	// LED points for the density envelope. A wide key (Space, Shift, Enter) is split into one point per
	// key unit along its length, so it reads as a run of bumps like its neighbours, not a flat shelf.
	const leds = devices
		.filter((d) => d.kind !== 'mousemat')
		.flatMap((d) => d.shapes)
		.flatMap((l) => {
			const n = Math.max(1, Math.round(l.w));
			return Array.from({ length: n }, (_, i) => ({ x: l.x - l.w / 2 + (l.w / n) * (i + 0.5), y: l.y }));
		});
	const homeY = devices.find((d) => d.kind === 'keyboard')?.shapes.find((s) => s.name === 'A')?.y ?? bounds.y + bounds.h / 2;

	let host: HTMLDivElement;
	let canvas: HTMLCanvasElement;
	let w = 0;
	let h = 0;
	let lines = 0;
	let samples = 0;
	let density = new Float32Array(0);
	let pointerY: number | null = null;
	let eased = -1;
	let reduce = false;
	let t = STILL_T;

	// Readout state (updated a few times a second, not every frame). Seeded with the still frame so the
	// server-rendered HTML already shows the home row decoded.
	const seedLines = 30;
	const seedRow = Math.round(((homeY - bounds.y) / bounds.h) * seedLines - 0.5);
	const seedY = bounds.y + ((seedRow + 0.5) / seedLines) * bounds.h;
	const seedColor = frame(effect, STILL_T, 1, 1);
	let slice = $state(seedRow + 1);
	let total = $state(seedLines);
	let sliceY = $state(seedY);
	let swatch = $state<string[]>(
		Array.from({ length: 24 }, (_, i) => hex(seedColor(bounds.x + ((i + 0.5) / 24) * bounds.w, seedY)))
	);

	// LED density envelope: each LED point is a small bump, so every key reads as texture in the line it
	// sits under. Normalised
	// by a high percentile (not the max), so the dense mouse strip doesn't flatten the keyboard.
	function buildDensity() {
		const grid = new Float32Array(lines * samples);
		const SIGMA = 0.4;
		for (let r = 0; r < lines; r++) {
			const y = bounds.y + ((r + 0.5) / lines) * bounds.h;
			for (let s = 0; s < samples; s++) {
				const x = bounds.x + (s / (samples - 1)) * bounds.w;
				let v = 0;
				for (const l of leds) {
					const dx = (x - l.x) / SIGMA;
					const dy = (y - l.y) / SIGMA;
					const q = dx * dx + dy * dy;
					if (q < 12) v += Math.exp(-q);
				}
				grid[r * samples + s] = v;
			}
		}
		const lit = Array.from(grid).filter((v) => v > 0.02).sort((a, b) => a - b);
		const p = lit.length ? lit[Math.floor(lit.length * 0.9)] : 1;
		for (let i = 0; i < grid.length; i++) grid[i] = Math.pow(Math.min(1, grid[i] / p), 0.85);
		density = grid;
	}

	function geometry() {
		const padX = Math.round(Math.min(40, Math.max(22, w * 0.035)));
		const spacing0 = h / (lines + 4.2);
		const top = spacing0 * 3.4;
		const bottom = spacing0 * 0.8;
		const spacing = (h - top - bottom) / lines;
		return { padX, top, spacing, plotW: w - padX * 2, amp: spacing * 3.1 };
	}

	function rowFromPointer(): number {
		const { top, spacing } = geometry();
		if (pointerY == null) return Math.round(((homeY - bounds.y) / bounds.h) * lines - 0.5);
		return Math.max(0, Math.min(lines - 1, Math.round((pointerY - top) / spacing - 1)));
	}

	function draw(updateReadout: boolean) {
		const ctx = canvas?.getContext('2d');
		if (!ctx || !w || !h || !lines) return;
		const dpr = devicePixelRatio || 1;
		ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
		ctx.clearRect(0, 0, w, h);

		const { padX, top, spacing, plotW, amp } = geometry();
		const phase = phaseAt(effect, t);
		const color = frame(effect, t, 1, 1);

		const target = rowFromPointer();
		eased = eased < 0 || reduce ? target : eased + (target - eased) * 0.22;
		const decoded = Math.round(eased);

		const trace = (r: number, base: number, y: number) => {
			for (let s = 0; s < samples; s++) {
				const x = bounds.x + (s / (samples - 1)) * bounds.w;
				const ridge = 0.5 + 0.5 * Math.cos(phase(x, y) * Math.PI * 2);
				const py = base - density[r * samples + s] * amp * (0.18 + 0.82 * ridge);
				const px = padX + (s / (samples - 1)) * plotW;
				if (s) ctx.lineTo(px, py);
				else ctx.moveTo(px, py);
			}
		};

		for (let r = 0; r < lines; r++) {
			const base = top + (r + 1) * spacing;
			const y = bounds.y + ((r + 0.5) / lines) * bounds.h;

			// occlude the lines behind, like ink over paper
			ctx.beginPath();
			trace(r, base, y);
			ctx.lineTo(padX + plotW, base + spacing * 2);
			ctx.lineTo(padX, base + spacing * 2);
			ctx.closePath();
			ctx.fillStyle = '#0b0b0b';
			ctx.fill();

			if (r === decoded) {
				const g = ctx.createLinearGradient(padX, 0, padX + plotW, 0);
				for (let k = 0; k <= 32; k++) {
					const [cr, cg, cb] = color(bounds.x + (k / 32) * bounds.w, y);
					g.addColorStop(k / 32, `rgb(${cr},${cg},${cb})`);
				}
				ctx.strokeStyle = g;
				ctx.lineWidth = 2;
			} else {
				ctx.strokeStyle = 'rgba(242,242,242,0.92)';
				ctx.lineWidth = 1;
			}
			ctx.beginPath();
			trace(r, base, y);
			ctx.stroke();

			if (r === decoded) {
				ctx.fillStyle = '#e2372c';
				ctx.fillRect(padX - 18, base - 4, 8, 8);
			}
		}

		if (updateReadout) {
			const y = bounds.y + ((decoded + 0.5) / lines) * bounds.h;
			slice = decoded + 1;
			total = lines;
			sliceY = y;
			swatch = Array.from({ length: cells }, (_, i) => hex(color(bounds.x + ((i + 0.5) / cells) * bounds.w, y)));
		}
	}

	function resize() {
		const r = host.getBoundingClientRect();
		w = r.width;
		h = r.height;
		const dpr = devicePixelRatio || 1;
		canvas.width = Math.round(w * dpr);
		canvas.height = Math.round(h * dpr);
		const nextLines = Math.max(16, Math.min(34, Math.round(h / 17)));
		const nextSamples = Math.max(120, Math.min(320, Math.round(w / 2.5)));
		if (nextLines !== lines || nextSamples !== samples) {
			lines = nextLines;
			samples = nextSamples;
			buildDensity();
			eased = -1;
		}
		draw(true);
	}

	onMount(() => {
		const mq = matchMedia('(prefers-reduced-motion: reduce)');
		reduce = mq.matches;
		let visible = false;
		let raf = 0;
		let last = 0;
		let frames = 0;

		const loop = (now: number) => {
			raf = 0;
			if (!visible || reduce || document.hidden) return;
			raf = requestAnimationFrame(loop);
			if (now - last < 1000 / 40) return;
			const dt = last ? Math.min(0.1, (now - last) / 1000) : 0;
			last = now;
			t += dt;
			draw(frames++ % 5 === 0);
		};
		const start = () => {
			if (!raf && visible && !reduce && !document.hidden) {
				last = 0;
				raf = requestAnimationFrame(loop);
			}
		};

		const ro = new ResizeObserver(resize);
		ro.observe(host);
		const io = new IntersectionObserver(([e]) => {
			visible = e.isIntersecting;
			start();
		});
		io.observe(host);
		const onVis = () => start();
		document.addEventListener('visibilitychange', onVis);
		const onMq = () => {
			reduce = mq.matches;
			if (reduce) t = STILL_T;
			draw(true);
			start();
		};
		mq.addEventListener('change', onMq);

		return () => {
			ro.disconnect();
			io.disconnect();
			document.removeEventListener('visibilitychange', onVis);
			mq.removeEventListener('change', onMq);
			if (raf) cancelAnimationFrame(raf);
		};
	});

	function point(e: PointerEvent) {
		pointerY = e.clientY - host.getBoundingClientRect().top;
		if (reduce) draw(true);
	}
	function leave() {
		pointerY = null;
		if (reduce) draw(true);
	}
</script>

<figure class="pulse">
	<div
		class="plot"
		bind:this={host}
		role="img"
		aria-label="Pulse plot: uncoil's default rainbow wave across the maintainer's desk, drawn as stacked white lines that rise where the keyboard and mouse LEDs sit. One line is shown in its live colours."
		onpointermove={point}
		onpointerdown={point}
		onpointerleave={leave}
	>
		<canvas bind:this={canvas}></canvas>
	</div>
	<figcaption class="readout">
		<span class="caps-sm label">Decoded slice <span class="num">{String(slice).padStart(2, '0')}/{total}</span></span>
		<span class="cells" aria-hidden="true">
			{#each swatch as c, i (i)}
				<span style:background={c}></span>
			{/each}
		</span>
		<span class="caps-sm label right num">y {sliceY.toFixed(1)} u</span>
	</figcaption>
</figure>

<style>
	.pulse {
		margin: 0;
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}
	.plot {
		position: relative;
		flex: 1;
		min-height: 0;
		cursor: crosshair;
	}
	canvas {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}
	.readout {
		display: grid;
		grid-template-columns: auto 1fr auto;
		align-items: center;
		gap: 16px;
		padding-top: 14px;
		border-top: 1px solid var(--color-seam);
		color: var(--color-ink-3);
	}
	.cells {
		display: grid;
		grid-auto-flow: column;
		grid-auto-columns: 1fr;
		gap: 3px;
		height: 8px;
	}
	.cells > span {
		background: var(--color-seam-2);
	}
	.label {
		white-space: nowrap;
	}
	.num {
		color: var(--color-ink);
		font-variant-numeric: tabular-nums;
	}
	@media (max-width: 420px) {
		.right {
			display: none;
		}
		.readout {
			grid-template-columns: auto 1fr;
		}
	}
</style>
