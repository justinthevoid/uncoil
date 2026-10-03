<script lang="ts">
	// Live desk preview. Geometry comes from get_desk once (and again only if placements change);
	// colours come from preview_frame, which runs the daemon's own effect code, about 30 times a second.
	import { onMount } from 'svelte';
	import { getDesk, previewFrame } from '#lib/api.ts';
	import type { Config, DeskDevice } from '#lib/types.ts';

	let { config }: { config: Config } = $props();

	const reducedMotion =
		typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

	let playing = $state(!reducedMotion);
	let error = $state<string | null>(null);

	let wrap: HTMLDivElement;
	let canvas: HTMLCanvasElement;

	let desk: DeskDevice[] = [];
	let deskKey = '';
	let colors: string[][] = [];
	let bounds = { x: 0, y: 0, w: 1, h: 1 };
	let t = 0;

	const FRAME_MS = 1000 / 30;
	const PAD = 1.1; // key units of breathing room around the desk

	const SURFACE = '#141518';
	const BODY = '#1e2024';
	const BODY_EDGE = '#2c2f34';
	const KEYCAP = '#24272b';

	function rgba(hex: string, a: number) {
		const n = parseInt(hex.slice(1), 16);
		return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
	}

	function computeBounds(devs: DeskDevice[]) {
		let x0 = Infinity;
		let y0 = Infinity;
		let x1 = -Infinity;
		let y1 = -Infinity;
		for (const d of devs) {
			x0 = Math.min(x0, d.x);
			y0 = Math.min(y0, d.y);
			x1 = Math.max(x1, d.x + d.w);
			y1 = Math.max(y1, d.y + d.h);
			for (const s of d.shapes) {
				x0 = Math.min(x0, s.x - s.w / 2);
				y0 = Math.min(y0, s.y - s.h / 2);
				x1 = Math.max(x1, s.x + s.w / 2);
				y1 = Math.max(y1, s.y + s.h / 2);
			}
		}
		return isFinite(x0) ? { x: x0, y: y0, w: x1 - x0, h: y1 - y0 } : { x: 0, y: 0, w: 1, h: 1 };
	}

	function glow(ctx: CanvasRenderingContext2D, x: number, y: number, r: number, hex: string, a: number) {
		const g = ctx.createRadialGradient(x, y, 0, x, y, r);
		g.addColorStop(0, rgba(hex, a));
		g.addColorStop(1, rgba(hex, 0));
		ctx.fillStyle = g;
		ctx.fillRect(x - r, y - r, r * 2, r * 2);
	}

	function body(ctx: CanvasRenderingContext2D, d: DeskDevice, radius: number, px: number) {
		ctx.beginPath();
		ctx.roundRect(d.x, d.y, d.w, d.h, radius);
		ctx.fillStyle = BODY;
		ctx.fill();
		ctx.lineWidth = px;
		ctx.strokeStyle = BODY_EDGE;
		ctx.stroke();
	}

	function draw() {
		const ctx = canvas?.getContext('2d');
		if (!ctx) return;
		const W = canvas.width;
		const H = canvas.height;
		ctx.setTransform(1, 0, 0, 1, 0, 0);
		ctx.globalCompositeOperation = 'source-over';
		ctx.clearRect(0, 0, W, H);
		if (!desk.length) return;

		const s = Math.min(W / (bounds.w + PAD * 2), H / (bounds.h + PAD * 2));
		const ox = (W - bounds.w * s) / 2 - bounds.x * s;
		const oy = (H - bounds.h * s) / 2 - bounds.y * s;
		ctx.setTransform(s, 0, 0, s, ox, oy);
		const px = 1 / s; // one device pixel, in key units

		// Mats sit underneath everything else.
		const isMat = (i: number) => Number(desk[i].kind === 'mousemat');
		const order = desk.map((_, i) => i).sort((a, b) => isMat(b) - isMat(a));

		for (const i of order) {
			const d = desk[i];
			const c = colors[i] ?? [];
			const col = (j: number) => c[j] ?? '#000000';

			if (d.kind === 'mousemat') {
				// One LED drives the whole edge strip, so the outline is the LED.
				const edge = col(0);
				const r = 0.55;
				ctx.beginPath();
				ctx.roundRect(d.x, d.y, d.w, d.h, r);
				ctx.fillStyle = SURFACE;
				ctx.fill();
				ctx.save();
				ctx.shadowColor = edge;
				ctx.shadowBlur = 0.9 * s;
				ctx.strokeStyle = edge;
				ctx.lineWidth = 0.13;
				ctx.stroke();
				ctx.restore();
				ctx.beginPath();
				ctx.roundRect(d.x + 0.2, d.y + 0.2, d.w - 0.4, d.h - 0.4, r - 0.15);
				ctx.strokeStyle = rgba(edge, 0.12);
				ctx.lineWidth = 0.3;
				ctx.stroke();
				continue;
			}

			// Light spilling from LEDs (underglow, mouse strip) onto the surface below.
			ctx.globalCompositeOperation = 'lighter';
			d.shapes.forEach((sh, j) => {
				if (!sh.is_key) glow(ctx, sh.x, sh.y, d.kind === 'keyboard' ? 1.5 : 0.85, col(j), 0.45);
			});
			ctx.globalCompositeOperation = 'source-over';

			body(ctx, d, d.kind === 'mouse' ? Math.min(d.w, d.h) / 2 : 0.35, px);

			const gap = 0.07;
			d.shapes.forEach((sh, j) => {
				if (!sh.is_key) return;
				ctx.beginPath();
				ctx.roundRect(sh.x - sh.w / 2 + gap, sh.y - sh.h / 2 + gap, sh.w - gap * 2, sh.h - gap * 2, 0.13);
				ctx.fillStyle = KEYCAP;
				ctx.fill();
				// Additive over the unlit keycap, so an LED that is off reads as a dark key, not a hole.
				ctx.globalCompositeOperation = 'lighter';
				ctx.fillStyle = col(j);
				ctx.fill();
				ctx.globalCompositeOperation = 'source-over';
			});

			// The LEDs themselves.
			d.shapes.forEach((sh, j) => {
				if (sh.is_key) return;
				ctx.globalCompositeOperation = 'lighter';
				glow(ctx, sh.x, sh.y, 0.45, col(j), 0.7);
				ctx.globalCompositeOperation = 'source-over';
				ctx.beginPath();
				ctx.arc(sh.x, sh.y, 0.11, 0, Math.PI * 2);
				ctx.fillStyle = col(j);
				ctx.fill();
			});
		}
	}

	function resize() {
		const dpr = window.devicePixelRatio || 1;
		const w = Math.max(1, Math.round(wrap.clientWidth * dpr));
		const h = Math.max(1, Math.round(wrap.clientHeight * dpr));
		if (canvas.width !== w || canvas.height !== h) {
			canvas.width = w;
			canvas.height = h;
			draw();
		}
	}

	onMount(() => {
		const ro = new ResizeObserver(resize);
		ro.observe(wrap);
		resize();

		let raf = 0;
		let lastTick = performance.now();
		let lastFrame = 0;
		let lastKey = '';
		let pending = false;

		const tick = (now: number) => {
			raf = requestAnimationFrame(tick);
			if (playing) t += (now - lastTick) / 1000;
			lastTick = now;
			if (pending || now - lastFrame < FRAME_MS) return;

			const snap = $state.snapshot(config) as Config;
			const key = JSON.stringify(snap) + t;
			if (key === lastKey) return; // paused and nothing changed
			lastKey = key;
			lastFrame = now;
			pending = true;

			const dk = JSON.stringify(snap.desk);
			const geometry =
				dk === deskKey
					? Promise.resolve()
					: getDesk(snap).then((d) => {
							desk = d;
							deskKey = dk;
							bounds = computeBounds(d);
						});
			geometry
				.then(() => previewFrame(snap, t))
				.then((c) => {
					colors = c;
					error = null;
					draw();
				})
				.catch((e) => {
					error = String(e);
				})
				.finally(() => (pending = false));
		};
		raf = requestAnimationFrame(tick);

		return () => {
			cancelAnimationFrame(raf);
			ro.disconnect();
		};
	});
</script>

<figure class="preview">
	<div
		class="well"
		bind:this={wrap}
		role="img"
		aria-label="Live preview of the lighting across your keyboard, mouse and mouse mat"
	>
		<canvas bind:this={canvas}></canvas>
		{#if error}
			<p class="error">The preview couldn't be computed: {error}</p>
		{/if}
	</div>
	<figcaption>
		<span>Computed by the same effect code that drives your devices.</span>
		<button type="button" class="play" onclick={() => (playing = !playing)}>
			{#if playing}
				<svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true">
					<rect x="2" y="1.5" width="2.6" height="9" rx="0.6" />
					<rect x="7.4" y="1.5" width="2.6" height="9" rx="0.6" />
				</svg>
				Pause preview
			{:else}
				<svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true">
					<path d="M3 1.6v8.8a.6.6 0 0 0 .9.5l7-4.4a.6.6 0 0 0 0-1L3.9 1.1a.6.6 0 0 0-.9.5z" />
				</svg>
				Play preview
			{/if}
		</button>
	</figcaption>
</figure>

<style>
	.preview {
		display: grid;
		grid-template-rows: 1fr auto;
		gap: 10px;
		min-height: 0;
		margin: 0;
	}
	.well {
		position: relative;
		min-height: 0;
		border-radius: 14px;
		background: radial-gradient(ellipse at 50% 40%, #131416 0%, var(--color-well) 70%);
		box-shadow:
			inset 0 1px 0 rgb(0 0 0 / 0.6),
			inset 0 0 0 1px #0a0b0c,
			0 1px 0 rgb(255 255 255 / 0.03);
		overflow: hidden;
	}
	canvas {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}
	.error {
		position: absolute;
		inset: auto 16px 14px 16px;
		color: var(--color-faint);
		font-size: 12px;
	}
	figcaption {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		color: var(--color-faint);
		font-size: 12px;
		padding: 0 4px;
	}
	.play {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		padding: 4px 10px;
		border: 1px solid var(--color-line);
		border-radius: 6px;
		background: transparent;
		color: var(--color-dim);
		font: inherit;
		font-size: 12px;
	}
	.play:hover {
		color: var(--color-text);
		border-color: var(--color-ink-3);
	}
	.play svg {
		fill: currentColor;
	}
</style>
