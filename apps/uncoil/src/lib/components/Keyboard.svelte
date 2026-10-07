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
	// A keyboard seen from above: the keys from its real geometry (the device's desk layout) in the frames their
	// groups sit in, each keycap as a top face on its skirt; the case, its front lip, screen and side dial as
	// measured from its product photo (lib/art/keyboards.ts); and the underglow as light bars where its LEDs run.
	// Edit mode: legends, optional second line, selection. Colour mode: each cap and LED in its live colour.
	import { boardExtent, isStripLed, KEYBOARD_ART, keyFrames } from '#lib/art/keyboards.ts';
	import type { DeskDevice, Shape } from '#lib/types.ts';

	interface Props {
		device: DeskDevice;
		/** Per-shape live colours (same order as device.shapes) for colour mode. */
		colors?: string[];
		caps?: Map<string, Cap>;
		selected?: string | null;
		onselect?: (shape: string) => void;
	}
	let { device, colors, caps, selected = null, onselect }: Props = $props();

	const ext = $derived(boardExtent(device));
	const W = $derived(ext.x1 - ext.x0);
	const H = $derived(ext.y1 - ext.y0);
	const keys = $derived(device.shapes.map((s, i) => ({ s, i })).filter(({ s }) => s.is_key && !isStripLed(s.name)));
	/** A box centred on (x, y), as a style in % of the board. */
	const pos = (x: number, y: number, w: number, h: number) =>
		`left:${((x - w / 2 - ext.x0) / W) * 100}%;top:${((y - h / 2 - ext.y0) / H) * 100}%;width:${(w / W) * 100}%;height:${(h / H) * 100}%`;
	/** A box by its corner, relative to the first key's corner (the art's units). */
	const at = (x: number, y: number, w: number, h: number) => pos(ext.origin[0] + x + w / 2, ext.origin[1] + y + h / 2, w, h);
	const interactive = $derived(!!onselect);

	const art = $derived(KEYBOARD_ART[device.id]);
	const frames = $derived(keyFrames(keys.map(({ s }) => s), art?.frames));
	/** The case: the art's, else the device box. */
	const shell = $derived.by(() => {
		if (!art) return pos(device.x + device.w / 2, device.y + device.h / 2, device.w, device.h);
		const [l, t, r, b] = art.case;
		return at(l, t, r - l, b - t);
	});

	type Led = { s: Shape; i: number };
	/** LEDs that run in a line along one side become one light bar; anything else stays a point. */
	const lit = $derived.by(() => {
		const leds: Led[] = device.shapes.map((s, i) => ({ s, i })).filter(({ s }) => !s.is_key || isStripLed(s.name));
		const bars: { vertical: boolean; leds: Led[]; style: string }[] = [];
		const used = new Set<number>();
		for (const vertical of [true, false]) {
			const groups = new Map<number, Led[]>();
			for (const l of leds) {
				if (used.has(l.i)) continue;
				const k = Math.round((vertical ? l.s.x : l.s.y) * 10);
				groups.set(k, [...(groups.get(k) ?? []), l]);
			}
			for (const g of groups.values()) {
				if (g.length < 3) continue;
				g.sort((a, b) => (vertical ? a.s.y - b.s.y : a.s.x - b.s.x));
				const along = g.map((l) => (vertical ? l.s.y : l.s.x));
				const step = (along[along.length - 1] - along[0]) / (g.length - 1);
				const from = along[0] - step / 2;
				const len = along[along.length - 1] - along[0] + step;
				const across = vertical ? g[0].s.x : g[0].s.y;
				const t = 0.22;
				const style = vertical ? pos(across, from + len / 2, t, len) : pos(from + len / 2, across, len, t);
				bars.push({ vertical, leds: g, style });
				g.forEach((l) => used.add(l.i));
			}
		}
		return { bars, points: leds.filter((l) => !used.has(l.i)) };
	});
</script>

<div class="board" class:lit={!!colors} style:aspect-ratio="{W} / {H}">
	{#each lit.bars as b, n (n)}
		<span class="bar" class:vertical={b.vertical} style={b.style} aria-hidden="true">
			{#each b.leds as { i } (i)}<span style:--c={colors?.[i]}></span>{/each}
		</span>
	{/each}
	{#if art?.rest}
		{@const [l, t, r, b] = art.rest}
		<span class="rest" style={at(l, t + 0.08, r - l, b - t - 0.08)} aria-hidden="true"></span>
	{/if}
	{#if art?.sideDial}
		{@const [t, b] = art.sideDial}
		<span class="side-dial" style={at(art.case[2] - 0.3, t, 0.48, b - t)} aria-hidden="true"></span>
	{/if}
	{#each art?.sideButtons ?? [] as [t, b] (t)}
		<span class="side-button" style={at(art!.case[2] - 0.05, t, 0.12, b - t)} aria-hidden="true"></span>
	{/each}
	<span class="case" style={shell} aria-hidden="true"></span>
	{#if art?.lip !== undefined}
		{@const [l, , r, b] = art.case}
		<span class="lip" style={at(l, art.lip, r - l, b - art.lip)} aria-hidden="true"><span class="mark">uncoil</span></span>
	{/if}
	{#each frames as rects, n (n)}
		{#each rects as [x, y, w, h], k (k)}
			<span class="frame" style={pos(x + w / 2, y + h / 2, w, h)} aria-hidden="true"></span>
		{/each}
	{/each}
	{#if art?.oled}
		{@const [x, y, w, h] = art.oled}
		<span class="oled" style={at(x, y, w, h)} aria-hidden="true"></span>
	{/if}
	{#each lit.points as { s, i } (s.name)}
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
				style={pos(s.x, s.y, s.w - 0.14, s.h - 0.14)}
				aria-pressed={selected === s.name}
				aria-label={cap.label ?? s.name}
				onclick={() => onselect?.(s.name)}
			>
				{#if cap.gel}<span class="tab" aria-hidden="true"></span>{/if}
				<span class="legend">{legend}</span>
				{#if cap.sub}<span class="sub">{cap.sub}</span>{/if}
			</button>
		{:else}
			<span class="key" class:fixed={interactive} style={pos(s.x, s.y, s.w - 0.14, s.h - 0.14)} style:--c={colors?.[i]} aria-hidden="true">
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
	}
	.case {
		position: absolute;
		border-radius: 1.4cqw;
		background: var(--case);
		box-shadow: 0 0 0 1px var(--case-edge);
	}
	/* the front lip, a seam across the case, with uncoil's wordmark where a vendor's would be */
	.lip {
		position: absolute;
		display: grid;
		place-items: center;
		border-top: 1px solid var(--seam-line);
		border-radius: 0 0 1.4cqw 1.4cqw;
	}
	.mark {
		color: var(--seam-line);
		font-family: var(--font-display);
		font-size: clamp(8px, 1.15cqw, 14px);
		font-weight: 600;
		letter-spacing: 0.12em;
	}
	/* a wrist rest that is part of the device: its knit as a fine diagonal weave */
	.rest {
		position: absolute;
		border-radius: 1.1cqw;
		background:
			repeating-linear-gradient(45deg, var(--seam-line) 0 1px, transparent 1px 4px),
			var(--shell-top);
		box-shadow: 0 0 0 1px var(--case-edge);
	}
	/* the recessed frames the key groups sit in */
	.frame {
		position: absolute;
		background: var(--plate);
	}
	.side-dial {
		position: absolute;
		border-radius: 0.4cqw;
		background: repeating-linear-gradient(180deg, var(--cap-skirt) 0 2px, var(--cap) 2px 4px);
		box-shadow: 0 0 0 1px var(--case-edge);
	}
	.side-button {
		position: absolute;
		border-radius: 0.3cqw;
		background: var(--cap-skirt);
		box-shadow: 0 0 0 1px var(--case-edge);
	}
	.oled {
		position: absolute;
		border-radius: 0.5cqw;
		background: #1a1816;
		box-shadow: 0 0 0 1px var(--case-edge);
	}
	.key {
		position: absolute;
		display: grid;
		align-content: center;
		justify-items: center;
		gap: 0.3cqw;
		padding: 0 0.5cqw 0.45cqw;
		border: 0;
		border-radius: 0.75cqw;
		background: var(--cap-skirt);
		box-shadow: 0 0 0 1px var(--cap-edge);
		color: var(--color-ink);
		overflow: hidden;
		transition: background-color var(--t-mid) var(--ease);
	}
	/* the keycap's top face, set back from its front edge like a real cap seen from above */
	.key::before {
		content: '';
		position: absolute;
		inset: 0.2cqw 0.36cqw 0.62cqw;
		border-radius: 0.55cqw;
		background: var(--cap);
		transition: background-color var(--t-mid) var(--ease);
	}
	.legend,
	.sub {
		position: relative;
	}
	button.key:hover::before {
		background: var(--color-surface-2);
	}
	.tab {
		position: absolute;
		z-index: 1;
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
		outline: 2px solid var(--color-select);
		outline-offset: -1px;
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
	.bar {
		position: absolute;
		display: flex;
		gap: 1px;
		border-radius: 999px;
		overflow: hidden;
	}
	.bar.vertical {
		flex-direction: column;
	}
	.bar > span {
		flex: 1;
		background: var(--c, var(--led-off));
	}
	.led {
		position: absolute;
		border-radius: 50%;
		background: var(--led-off);
	}

	/* Too narrow for a second line: the gel tab alone marks the key; the list below names it. */
	@container (max-width: 640px) {
		.sub {
			display: none;
		}
	}

	/* Colour mode: each cap lit with its LED colour, its skirt a shade darker. */
	.lit .key {
		background: color-mix(in oklab, var(--c, var(--cap)) 58%, #000);
		transition: background-color 120ms linear;
	}
	.lit .key::before {
		background: var(--c, var(--cap));
		transition: background-color 120ms linear;
	}
	.lit .bar > span,
	.lit .led {
		background: var(--c, var(--led-off));
		transition: background-color 120ms linear;
	}
</style>
