<script lang="ts">
	// The Basilisk V3 Pro seen from above (silhouette traced from its product photo, lib/art/basilisk.ts), with
	// each remappable button as a region you can click: main buttons, wheel (click, up, down, tilt), the two
	// buttons behind the wheel, the side buttons and the clutch, and the profile button underneath as a chip.
	// A gel fill marks buttons that do something changed.
	import type { Gel } from '#lib/keys.ts';
	import { BODY, GRIPS, LOGO, REGION, SEAMS, TREAD, VIEW, WELL } from '#lib/art/basilisk.ts';
	import { SPIRAL } from './BasiliskArt.svelte';

	interface Region {
		/** Keymap button name (e.g. LEFT_CLICK). */
		name: string;
		label: string;
		gel: Gel | null;
		selected: boolean;
	}
	interface Props {
		regions: Map<string, Region>;
		onselect: (name: string) => void;
	}
	let { regions, onselect }: Props = $props();
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
	type Name = keyof typeof REGION;
	const PAD = 104;
	const L = VIEW.x - PAD + 4;
	const R = VIEW.x + VIEW.w + PAD - 4;
	const CALLOUTS: { name: Name; label: string; from: [number, number]; side: 'l' | 'r' }[] = [
		{ name: 'WHEEL_CLICK', label: 'Wheel', from: [262, 238], side: 'r' },
		{ name: 'SCROLL_MODE', label: 'Scroll mode', from: [255, 330], side: 'r' },
		{ name: 'DPI_BUTTON', label: 'DPI', from: [255, 362], side: 'r' },
		{ name: 'FORWARD', label: 'Forward', from: [108, 242], side: 'l' },
		{ name: 'BACK', label: 'Back', from: [106, 284], side: 'l' },
		{ name: 'CLUTCH', label: 'Clutch', from: [84, 342], side: 'l' }
	];
	const S = 1.7;
</script>

{#snippet hot(name: Name, clipped = false)}
	{#if r(name)}
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
			<path d={REGION[name]} />
		</g>
	{/if}
{/snippet}

<div class="mouse">
	<svg viewBox="{VIEW.x - PAD} {VIEW.y} {VIEW.w + PAD * 2} {VIEW.h}" role="group" aria-label="Mouse buttons">
		<defs>
			<clipPath id="{uid}-body"><path d={BODY} /></clipPath>
			<pattern id="{uid}-grip" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
				<circle cx="2" cy="2" r="0.8" class="dot" />
			</pattern>
		</defs>
		<path class="body" d={BODY} />
		{@render hot('LEFT_CLICK', true)}
		{@render hot('RIGHT_CLICK', true)}
		<g clip-path="url(#{uid}-body)" class="lines">
			{#each GRIPS as g (g)}<path d={g} fill="url(#{uid}-grip)" stroke="none" />{/each}
			{#each SEAMS as d (d)}<path {d} />{/each}
		</g>
		<path class="well" d={WELL} />
		<path class="logo" d={SPIRAL} transform="translate({LOGO[0] - 12 * S} {LOGO[1] - 12 * S}) scale({S})" />
		<text class="lr" x="176" y="262" text-anchor="middle" dominant-baseline="central">L</text>
		<text class="lr" x="306" y="236" text-anchor="middle" dominant-baseline="central">R</text>
		{@render hot('FORWARD')}
		{@render hot('BACK')}
		{@render hot('CLUTCH')}
		{@render hot('WHEEL_CLICK')}
		<path class="tread" d={TREAD} />
		{@render hot('WHEEL_UP')}
		{@render hot('WHEEL_DOWN')}
		{@render hot('WHEEL_LEFT')}
		{@render hot('WHEEL_RIGHT')}
		{@render hot('SCROLL_MODE')}
		{@render hot('DPI_BUTTON')}

		{#each CALLOUTS as c (c.name)}
			{#if r(c.name)}
				<g class="callout">
					<line x1={c.from[0]} y1={c.from[1]} x2={c.side === 'r' ? R - 92 : L + 66} y2={c.from[1]} />
					<text x={c.side === 'r' ? R - 88 : L} y={c.from[1]} dominant-baseline="central">{c.label}</text>
				</g>
			{/if}
		{/each}
	</svg>
	{#if r('PROFILE_BUTTON')}
		<div class="under" role="group" aria-label="Button underneath">
			<span class="u-label">Underneath</span>
			<button type="button" class="u" class:sel={r('PROFILE_BUTTON')!.selected} aria-pressed={r('PROFILE_BUTTON')!.selected} onclick={() => onselect('PROFILE_BUTTON')} style:--gel={gelColor('PROFILE_BUTTON')}>
				{#if r('PROFILE_BUTTON')!.gel}<span class="tab"></span>{/if}
				Profile
			</button>
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
	.lr {
		fill: var(--color-ink-3);
		font-size: 22px;
		font-weight: 600;
		pointer-events: none;
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
		font-size: 15px;
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
