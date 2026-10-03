<script lang="ts">
	// The mouse seen from above, with each remappable button as a region you can click: main buttons,
	// wheel (click, up, down, tilt), the button behind the wheel, side buttons, the clutch, and the two
	// buttons underneath shown as a strip below. A gel tab marks buttons that do something changed.
	import type { Gel } from '#lib/keys.ts';

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
</script>

{#snippet hot(name: string, d: string, tx: number, ty: number, text: string)}
	{#if r(name)}
		<g class={cls(name)} role="button" tabindex="0" aria-label="{r(name)!.label}" aria-pressed={r(name)!.selected} onclick={() => onselect(name)} onkeydown={(e) => key(e, name)} style:--gel={gelColor(name)}>
			<path {d} />
			{#if text}<text x={tx} y={ty} text-anchor="middle" dominant-baseline="central">{text}</text>{/if}
		</g>
	{/if}
{/snippet}

<div class="mouse">
	<svg viewBox="-40 0 320 420" role="group" aria-label="Mouse buttons">
		<!-- body -->
		<path class="body" d="M120 12 C 60 12 34 60 32 130 C 30 210 34 300 52 352 C 66 392 92 408 120 408 C 148 408 174 392 188 352 C 206 300 210 210 208 130 C 206 60 180 12 120 12 Z" />
		{@render hot('LEFT_CLICK', 'M118 16 C 66 18 38 62 36 130 L 36 168 L 118 168 Z', 78, 120, 'L')}
		{@render hot('RIGHT_CLICK', 'M122 16 C 174 18 202 62 204 130 L 204 168 L 122 168 Z', 162, 120, 'R')}
		{@render hot('WHEEL_UP', 'M110 34 L 130 34 L 120 24 Z', 0, 0, '')}
		{@render hot('WHEEL_CLICK', 'M108 46 Q 108 40 114 40 L 126 40 Q 132 40 132 46 L 132 104 Q 132 110 126 110 L 114 110 Q 108 110 108 104 Z', 120, 75, '')}
		{@render hot('WHEEL_DOWN', 'M110 116 L 130 116 L 120 126 Z', 0, 0, '')}
		{@render hot('WHEEL_LEFT', 'M100 66 L 100 84 L 90 75 Z', 0, 0, '')}
		{@render hot('WHEEL_RIGHT', 'M140 66 L 140 84 L 150 75 Z', 0, 0, '')}
		{@render hot('SCROLL_MODE', 'M110 136 Q 110 132 114 132 L 126 132 Q 130 132 130 136 L 130 150 Q 130 154 126 154 L 114 154 Q 110 154 110 150 Z', 120, 143, '')}
		{@render hot('FORWARD', 'M22 176 Q 22 170 28 170 L 36 170 L 36 214 L 28 214 Q 22 214 22 208 Z', 0, 0, '')}
		{@render hot('BACK', 'M22 226 Q 22 220 28 220 L 36 220 L 36 264 L 28 264 Q 22 264 22 258 Z', 0, 0, '')}
		{@render hot('CLUTCH', 'M24 282 Q 24 276 30 276 L 38 276 L 38 306 L 30 306 Q 24 306 24 300 Z', 0, 0, '')}

		<!-- side labels -->
		<g class="callout"><line x1="20" y1="192" x2="-2" y2="192" /><text x="-6" y="192" text-anchor="end" dominant-baseline="central">Forward</text></g>
		<g class="callout"><line x1="20" y1="242" x2="-2" y2="242" /><text x="-6" y="242" text-anchor="end" dominant-baseline="central">Back</text></g>
		<g class="callout"><line x1="22" y1="291" x2="-2" y2="291" /><text x="-6" y="291" text-anchor="end" dominant-baseline="central">Clutch</text></g>
		<g class="callout"><line x1="134" y1="143" x2="222" y2="143" /><text x="226" y="143" dominant-baseline="central">Scroll mode</text></g>
		<g class="callout"><line x1="152" y1="75" x2="222" y2="75" /><text x="226" y="75" dominant-baseline="central">Wheel</text></g>
	</svg>
	<div class="under" role="group" aria-label="Buttons underneath">
		<span class="u-label">Underneath</span>
		{#each ['DPI_BUTTON', 'PROFILE_BUTTON'] as name (name)}
			{#if r(name)}
				<button type="button" class="u" class:sel={r(name)!.selected} aria-pressed={r(name)!.selected} onclick={() => onselect(name)} style:--gel={gelColor(name)}>
					{#if r(name)!.gel}<span class="tab"></span>{/if}
					{name === 'DPI_BUTTON' ? 'DPI' : 'Profile'}
				</button>
			{/if}
		{/each}
	</div>
</div>

<style>
	.mouse {
		display: grid;
		justify-items: center;
		gap: 10px;
	}
	svg {
		width: 100%;
		max-width: 300px;
		height: auto;
		overflow: visible;
	}
	.body {
		fill: var(--case);
		stroke: var(--color-seam-2);
		stroke-width: 1.5;
	}
	.hot path {
		fill: var(--cap);
		stroke: var(--cap-edge);
		stroke-width: 1.5;
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
	.hot.gel path {
		stroke: var(--gel);
		stroke-width: 3;
	}
	.hot.sel path {
		stroke: var(--color-select);
		stroke-width: 3;
	}
	.hot:focus-visible path {
		stroke: var(--color-select);
		stroke-dasharray: 4 3;
		stroke-width: 2;
	}
	.hot text {
		fill: var(--color-ink-2);
		font-size: 22px;
		font-weight: 600;
		pointer-events: none;
	}
	.callout line {
		stroke: var(--color-seam-2);
		stroke-width: 1;
	}
	.callout text {
		fill: var(--color-ink-3);
		font-size: 13px;
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
