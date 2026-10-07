<script lang="ts">
	// A mouse mat from above, in its desk box: the surface (cloth, or a hard mat's fine texture), the cable hub at
	// the back left, and its edge light. One LED lights the whole edge; several follow the device file's order
	// round the lit edges (down the left, along the front, up the right on Razer's edge-lit mats).
	import type { DeskDevice } from '#lib/types.ts';

	interface Props {
		device: DeskDevice;
		/** Colour per shape (same order as device.shapes). */
		colors: string[];
	}
	let { device, colors }: Props = $props();
	const uid = $props.id();
	const off = 'var(--led-off)';
	const HARD = ['razer-firefly-v2'];
	const hard = $derived(HARD.includes(device.id));
	const W = $derived(device.w);
	const H = $derived(device.h);
	const R = $derived(Math.min(0.6, H * 0.06));
	const E = 0.18; // the lit band's width, in key units

	/** The lit run for several LEDs: down the left, along the front, up the right, split evenly in order. */
	const parts = $derived.by(() => {
		const n = device.shapes.length;
		if (n <= 1) return [];
		const i = E / 2;
		const pts: [number, number][] = [[i, i + 0.4], [i, H - i], [W - i, H - i], [W - i, i + 0.4]];
		const seg = pts.slice(1).map((p, k) => Math.hypot(p[0] - pts[k][0], p[1] - pts[k][1]));
		const total = seg.reduce((a, b) => a + b, 0);
		const at = (d: number): [number, number] => {
			for (let k = 0; k < seg.length; k++) {
				if (d <= seg[k] || k === seg.length - 1) {
					const t = Math.min(d / seg[k], 1);
					return [pts[k][0] + (pts[k + 1][0] - pts[k][0]) * t, pts[k][1] + (pts[k + 1][1] - pts[k][1]) * t];
				}
				d -= seg[k];
			}
			return pts[pts.length - 1];
		};
		return Array.from({ length: n }, (_, k) => {
			const a = (total * k) / n + 0.06, b = (total * (k + 1)) / n - 0.06;
			const steps = 6;
			const p = Array.from({ length: steps + 1 }, (_, s) => at(a + ((b - a) * s) / steps));
			return 'M' + p.map(([x, y]) => `${x.toFixed(2)} ${y.toFixed(2)}`).join(' L');
		});
	});
</script>

<svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" aria-hidden="true">
	<defs>
		<pattern id="{uid}-hard" width="0.12" height="0.12" patternUnits="userSpaceOnUse">
			<circle cx="0.06" cy="0.06" r="0.012" class="grain" />
		</pattern>
	</defs>
	<rect class="hub" x="1.1" y="-0.24" width="1.5" height="0.5" rx="0.16" />
	<rect class="surface" x="0" y="0" width={W} height={H} rx={R} />
	{#if hard}<rect x="0.3" y="0.3" width={W - 0.6} height={H - 0.6} rx={R} fill="url(#{uid}-hard)" />{/if}
	{#if parts.length}
		{#each parts as d, k (k)}<path class="band" {d} style:stroke={colors[k] ?? off} />{/each}
	{:else}
		<rect class="ring" x={E / 2} y={E / 2} width={W - E} height={H - E} rx={R} style:stroke={colors[0] ?? off} />
	{/if}
</svg>

<style>
	svg {
		display: block;
		width: 100%;
		height: 100%;
		overflow: visible;
	}
	.surface {
		fill: var(--cloth);
		stroke: rgb(0 0 0 / 0.35);
		stroke-width: 1px;
		vector-effect: non-scaling-stroke;
	}
	.hub {
		fill: var(--shell-top);
		stroke: var(--case-edge);
		stroke-width: 1px;
		vector-effect: non-scaling-stroke;
	}
	.grain {
		fill: var(--seam-line);
	}
	.ring,
	.band {
		fill: none;
		stroke-width: 0.18px;
		stroke-linecap: round;
		transition: stroke 120ms linear;
	}
</style>
