<script lang="ts">
	// A traced mouse seen from above (lib/art/mice.ts), with each remappable button it has a region for as a
	// region you can click: main buttons, wheel (click, up, down, tilt), buttons behind the wheel, side
	// buttons and clutch, with callouts naming the small ones; the buttons underneath are chips. Buttons
	// without a region are still in the list beside it. A gel fill marks buttons that do something changed.
	import type { Gel } from '#lib/keys.ts';
	import { SPIRAL, type MouseArt } from '#lib/art/mice.ts';

	interface Region {
		/** Keymap button name (e.g. LEFT_CLICK). */
		name: string;
		label: string;
		gel: Gel | null;
		selected: boolean;
	}
	interface Props {
		art: MouseArt;
		regions: Map<string, Region>;
		onselect: (name: string) => void;
	}
	let { art, regions, onselect }: Props = $props();
	const uid = $props.id();

	const r = (name: string) => regions.get(name);
	const cls = (name: string) => {
		const x = r(name);
		return x ? `hot${x.selected ? ' sel' : ''}${x.gel ? ' gel' : ''}` : 'hot off';
	};
	const gelColor = (name: string) => (r(name)?.gel ? `var(--color-gel-${r(name)!.gel})` : undefined);
	function key(e: KeyboardEvent, name: string) {
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			onselect(name);
		}
	}
	const LABELS: Record<string, string> = { WHEEL_CLICK: 'Wheel', SCROLL_MODE: 'Scroll mode', DPI_BUTTON: 'DPI', FORWARD: 'Forward', BACK: 'Back', CLUTCH: 'Clutch' };
	// a margin each side for the callouts, and type sized to the photo
	const U = $derived(art.body_box.h / 496);
	const PAD = $derived(104 * U);
	const L = $derived(art.view.x - PAD + 4 * U);
	const R = $derived(art.view.x + art.view.w + PAD - 4 * U);
	const callouts = $derived(
		Object.entries(art.callouts)
			.filter(([name]) => r(name) && LABELS[name])
			.map(([name, from]) => ({ name, label: LABELS[name], from, side: from[0] > art.body_box.x + art.body_box.w / 2 ? 'r' : 'l' }))
	);
	const SMALL = ['WHEEL_CLICK', 'WHEEL_UP', 'WHEEL_DOWN', 'WHEEL_LEFT', 'WHEEL_RIGHT', 'SCROLL_MODE', 'DPI_BUTTON', 'FORWARD', 'BACK', 'CLUTCH'];
	const under = $derived(['DPI_BUTTON', 'PROFILE_BUTTON'].filter((n) => r(n) && !art.region[n]));
	const S = $derived(1.7 * U);
</script>

{#snippet hot(name: string, clipped = false)}
	{#if r(name) && art.region[name]}
		<g
			class={cls(name)}
			role="button"
			tabindex="0"
			aria-label={r(name)!.label}
			aria-pressed={r(name)!.selected}
			onclick={() => onselect(name)}
			onkeydown={(e) => key(e, name)}
			style:--gel={gelColor(name)}
			clip-path={clipped ? `url(#${uid}-body)` : undefined}
		>
			<path d={art.region[name]} />
		</g>
	{/if}
{/snippet}

<div class="mouse">
	<svg viewBox="{art.view.x - PAD} {art.view.y} {art.view.w + PAD * 2} {art.view.h}" role="group" aria-label="Mouse buttons" style:--u={U}>
		<defs>
			<clipPath id="{uid}-body"><path d={art.body} /></clipPath>
			<pattern id="{uid}-grip" width={4 * U} height={4 * U} patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
				<circle cx={2 * U} cy={2 * U} r={0.8 * U} class="dot" />
			</pattern>
		</defs>
		<path class="body" d={art.body} />
		{@render hot('LEFT_CLICK', true)}
		{@render hot('RIGHT_CLICK', true)}
		<g clip-path="url(#{uid}-body)" class="lines">
			{#each art.grips ?? [] as g (g)}<path d={g} fill="url(#{uid}-grip)" stroke="none" />{/each}
			{#each art.seams as d (d)}<path {d} />{/each}
		</g>
		<path class="well" d={art.well} />
		<path class="logo" d={SPIRAL} transform="translate({art.logo[0] - 12 * S} {art.logo[1] - 12 * S}) scale({S})" />
		{#each SMALL as name (name)}
			{@render hot(name)}
			{#if name === 'WHEEL_CLICK'}<path class="tread" d={art.tread} />{/if}
		{/each}

		{#each callouts as c (c.name)}
			<g class="callout">
				<line x1={c.from[0]} y1={c.from[1]} x2={c.side === 'r' ? R - 92 * U : L + 66 * U} y2={c.from[1]} />
				<text x={c.side === 'r' ? R - 88 * U : L} y={c.from[1]} dominant-baseline="central">{c.label}</text>
			</g>
		{/each}
	</svg>
	{#if under.length}
		<div class="under" role="group" aria-label="Buttons underneath">
			<span class="u-label">Underneath</span>
			{#each under as name (name)}
				<button type="button" class="u" class:sel={r(name)!.selected} aria-pressed={r(name)!.selected} onclick={() => onselect(name)} style:--gel={gelColor(name)}>
					{#if r(name)!.gel}<span class="tab"></span>{/if}
					{name === 'DPI_BUTTON' ? 'DPI' : 'Profile'}
				</button>
			{/each}
		</div>
	{/if}
</div>

<style>
	.mouse {
		display: grid;
		justify-items: center;
		gap: 12px;
	}
	svg {
		width: 100%;
		max-width: 440px;
		max-height: 66vh;
		height: auto;
		overflow: visible;
	}
	path,
	line {
		vector-effect: non-scaling-stroke;
	}
	.body {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1.4;
	}
	.lines path {
		fill: none;
		stroke: var(--seam-line);
		stroke-width: 1.2;
		stroke-linecap: round;
		pointer-events: none;
	}
	.dot {
		fill: var(--seam-line);
	}
	.well {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1.2;
	}
	.tread {
		fill: none;
		stroke: var(--seam-line);
		stroke-width: 1.4;
		pointer-events: none;
	}
	.logo {
		fill: none;
		stroke: var(--color-ink-3);
		stroke-width: 1.7;
		stroke-linecap: round;
	}
	.hot path {
		fill: var(--shell-top);
		stroke: var(--seam-line);
		stroke-width: 1.2;
		transition:
			fill var(--t-mid) var(--ease),
			stroke var(--t-mid) var(--ease);
	}
	.hot {
		cursor: default;
		outline: none;
	}
	.hot:hover path {
		fill: var(--color-surface-2);
	}
	/* clipped regions lose half their stroke at the body's edge, so a change and the selection also fill */
	.hot.gel path {
		fill: color-mix(in srgb, var(--gel) 18%, var(--shell-top));
		stroke: var(--gel);
		stroke-width: 3;
	}
	.hot.sel path {
		fill: var(--color-surface-3);
		stroke: var(--color-select);
		stroke-width: 3;
	}
	.hot:focus-visible path {
		stroke: var(--color-select);
		stroke-dasharray: 4 3;
		stroke-width: 2;
	}
	.callout line {
		stroke: var(--color-seam-2);
		stroke-width: 1;
	}
	.callout text {
		fill: var(--color-ink-3);
		font-size: calc(15px * var(--u));
	}
	.under {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.u-label {
		color: var(--color-ink-3);
		font-size: 12px;
	}
	.u {
		position: relative;
		height: 30px;
		padding: 0 14px;
		border: 1px solid var(--cap-edge);
		border-radius: var(--radius);
		background: var(--cap);
		font-weight: 600;
		overflow: hidden;
	}
	.u:hover {
		background: var(--color-surface-2);
	}
	.u.sel {
		box-shadow: inset 0 0 0 2px var(--color-select);
	}
	.tab {
		position: absolute;
		inset: 0 0 auto 0;
		height: 3px;
		background: var(--gel);
	}
</style>
