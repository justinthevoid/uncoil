<script lang="ts">
	// Docks from above, in their desk box. The Mouse Dock Pro: a round charging puck with its lit ring and the
	// charging pad in the middle. The Base Station V2 Chroma: a weighted round base with its LED ring, the
	// upright at the back and the headset arm reaching forward, and its two USB ports in the back. Each LED (the
	// device file's ring, "Base 1" … at the front, clockwise) lights its arc of the ring.
	import type { DeskDevice } from '#lib/types.ts';

	interface Props {
		device: DeskDevice;
		/** Colour per shape (same order as device.shapes). */
		colors: string[];
	}
	let { device, colors }: Props = $props();
	const off = 'var(--led-off)';
	const stand = $derived(device.id.includes('base-station'));
	const W = $derived(device.w);
	const H = $derived(device.h);
	const cx = $derived(W / 2);
	const cy = $derived(H / 2);
	const r = $derived(Math.min(W, H) / 2 - 0.08);

	/** One arc per LED, centred on each LED's angle from the device file. */
	const arcs = $derived.by(() => {
		const n = device.shapes.length;
		if (!n) return [];
		const ring = r - 0.14;
		return device.shapes.map((s) => {
			const a = Math.atan2(s.y - (device.y + H / 2), s.x - (device.x + W / 2));
			const half = Math.PI / n - 0.06;
			const p = (t: number) => `${(cx + ring * Math.cos(t)).toFixed(3)} ${(cy + ring * Math.sin(t)).toFixed(3)}`;
			return `M${p(a - half)} A${ring} ${ring} 0 0 1 ${p(a + half)}`;
		});
	});
</script>

<svg viewBox="0 0 {W} {H}" aria-hidden="true">
	<circle class="body" {cx} {cy} {r} />
	{#each arcs as d, k (k)}<path class="ring" {d} style:stroke={colors[k] ?? off} />{/each}
	<circle class="top" {cx} {cy} r={r - 0.32} />
	{#if stand}
		<!-- the upright at the back, the arm reaching forward to hold the headset, USB ports behind -->
		<rect class="port" x={cx - 0.62} y={cy - r - 0.06} width="0.5" height="0.2" rx="0.04" />
		<rect class="port" x={cx + 0.12} y={cy - r - 0.06} width="0.5" height="0.2" rx="0.04" />
		<rect class="post" x={cx - 0.42} y={cy - r + 0.5} width="0.84" height="1.1" rx="0.3" />
		<rect class="arm" x={cx - 0.3} y={cy - r + 0.9} width="0.6" height={r * 1.35} rx="0.3" />
	{:else}
		<!-- the charging pad and its contacts -->
		<circle class="pad" {cx} {cy} r={r * 0.46} />
		<circle class="dot" cx={cx - 0.12} {cy} r="0.05" />
		<circle class="dot" cx={cx + 0.12} {cy} r="0.05" />
		<rect class="port" x={cx - 0.2} y={cy - r - 0.04} width="0.4" height="0.16" rx="0.04" />
	{/if}
</svg>

<style>
	svg {
		display: block;
		width: 100%;
		height: 100%;
		overflow: visible;
	}
	.body {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1px;
		vector-effect: non-scaling-stroke;
	}
	.top,
	.post,
	.arm {
		fill: var(--shell-top);
		stroke: var(--seam-line);
		stroke-width: 1px;
		vector-effect: non-scaling-stroke;
	}
	.pad {
		fill: var(--plate);
		stroke: var(--seam-line);
		stroke-width: 1px;
		vector-effect: non-scaling-stroke;
	}
	.dot,
	.port {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1px;
		vector-effect: non-scaling-stroke;
	}
	.ring {
		fill: none;
		stroke-width: 0.16px;
		stroke-linecap: round;
		transition: stroke 120ms linear;
	}
</style>
