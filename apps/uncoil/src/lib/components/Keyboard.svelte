<script lang="ts" module>
	/** Keycap legends for the layout's key names. */
	const LEGEND: Record<string, string> = {
		Escape: 'Esc',
		Delete: 'Del',
		Insert: 'Ins',
		'Page Up': 'PgUp',
		'Page Down': 'PgDn',
		'Caps Lock': 'Caps',
		'Left Shift': 'Shift',
		'Right Shift': 'Shift',
		'Left Control': 'Ctrl',
		'Right Control': 'Ctrl',
		'Left Windows': 'Win',
		'Left Alt': 'Alt',
		'Right Alt': 'Alt',
		'Right Fn': 'Fn',
		'Up Arrow': '↑︎',
		'Down Arrow': '↓︎',
		'Left Arrow': '←︎',
		'Right Arrow': '→︎',
		Space: ''
	};
	export const legendFor = (name: string) => LEGEND[name] ?? name;

	/** The swatch book's four gels, each with one job. */
	export type Gel = 'yours' | 'media' | 'light' | 'system';

	export interface Cap {
		/** Small second line under the legend, e.g. what the key does with Fn. */
		sub?: string;
		/** Gel tab across the top of the cap: what kind of change this key carries. */
		gel?: Gel;
		/** Can't be selected (not remappable). */
		fixed?: boolean;
		/** Accessible description. */
		label?: string;
	}
</script>

<script lang="ts">
	// A keyboard drawn as solid keycaps from its real geometry (the device's desk layout).
	// Edit mode: legends, optional second line, selection. Colour mode: each cap lit with its live colour.
	import type { DeskDevice } from '#lib/types.ts';

	interface Props {
		device: DeskDevice;
		/** Per-shape live colours (same order as device.shapes) for colour mode. */
		colors?: string[];
		caps?: Map<string, Cap>;
		selected?: string | null;
		onselect?: (shape: string) => void;
	}
	let { device, colors, caps, selected = null, onselect }: Props = $props();

	const pad = 0.35;
	const W = $derived(device.w + pad * 2);
	const H = $derived(device.h + pad * 2);
	const keys = $derived(device.shapes.map((s, i) => ({ s, i })).filter(({ s }) => s.is_key));
	const leds = $derived(device.shapes.map((s, i) => ({ s, i })).filter(({ s }) => !s.is_key));
	const pos = (x: number, y: number, w: number, h: number) =>
		`left:${((x - w / 2 - device.x + pad) / W) * 100}%;top:${((y - h / 2 - device.y + pad) / H) * 100}%;width:${(w / W) * 100}%;height:${(h / H) * 100}%`;
	const interactive = $derived(!!onselect);
</script>

<div class="board" class:lit={!!colors} style:aspect-ratio="{W} / {H}">
	{#each leds as { s, i } (s.name)}
		<span class="led" style={pos(s.x, s.y, 0.22, 0.22)} style:--c={colors?.[i]}></span>
	{/each}
	{#each keys as { s, i } (s.name)}
		{@const cap = caps?.get(s.name)}
		{@const legend = legendFor(s.name)}
		{#if interactive && cap && !cap.fixed}
			<button
				type="button"
				class="key"
				style:--gel={cap.gel ? `var(--color-gel-${cap.gel})` : undefined}
				class:sel={selected === s.name}
				style={pos(s.x, s.y, s.w - 0.1, s.h - 0.1)}
				aria-pressed={selected === s.name}
				aria-label={cap.label ?? s.name}
				onclick={() => onselect?.(s.name)}
			>
				{#if cap.gel}<span class="tab" aria-hidden="true"></span>{/if}
				<span class="legend">{legend}</span>
				{#if cap.sub}<span class="sub">{cap.sub}</span>{/if}
			</button>
		{:else}
			<span class="key" class:fixed={interactive} style={pos(s.x, s.y, s.w - 0.1, s.h - 0.1)} style:--c={colors?.[i]} aria-hidden="true">
				{#if !colors}<span class="legend">{legend}</span>{/if}
			</span>
		{/if}
	{/each}
</div>

<style>
	.board {
		position: relative;
		width: 100%;
		container-type: inline-size;
		border-radius: 1.4cqw;
		background: var(--case);
		box-shadow: inset 0 0 0 1px var(--color-seam);
	}
	.key {
		position: absolute;
		display: grid;
		align-content: center;
		justify-items: center;
		gap: 0.3cqw;
		padding: 0 0.4cqw;
		border: 0;
		border-radius: 0.75cqw;
		background: var(--cap);
		box-shadow:
			inset 0 -0.3cqw 0 var(--cap-edge),
			0 0 0 1px var(--cap-edge);
		color: var(--color-ink);
		overflow: hidden;
		transition:
			background-color var(--t-mid) var(--ease),
			box-shadow var(--t-mid) var(--ease);
	}
	button.key:hover {
		background: var(--color-surface-2);
	}
	.tab {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: max(3px, 0.45cqw);
		background: var(--gel);
	}
	.legend {
		font-size: clamp(10px, 1.6cqw, 15px);
		font-weight: 600;
		line-height: 1;
		white-space: nowrap;
	}
	.sub {
		max-width: 100%;
		font-size: clamp(8.5px, 1.15cqw, 12px);
		font-weight: 500;
		line-height: 1.1;
		color: var(--color-ink-3);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.key.sel {
		box-shadow:
			inset 0 0 0 2px var(--color-select),
			inset 0 -0.3cqw 0 var(--cap-edge);
	}
	.key.fixed {
		color: var(--color-ink-4);
		opacity: 0.7;
	}
	button.key:focus-visible {
		outline: 2px solid var(--color-select);
		outline-offset: 1px;
		z-index: 1;
	}
	.led {
		position: absolute;
		border-radius: 50%;
		background: var(--color-seam-2);
	}

	/* Too narrow for a second line: the gel tab alone marks the key; the list below names it. */
	@container (max-width: 640px) {
		.sub {
			display: none;
		}
	}

	/* Colour mode: caps take the live LED colour. */
	.lit .key {
		background: var(--c, var(--cap));
		transition: background-color 120ms linear;
	}
	.lit .led {
		background: var(--c, var(--color-seam-2));
		transition: background-color 120ms linear;
	}
</style>
