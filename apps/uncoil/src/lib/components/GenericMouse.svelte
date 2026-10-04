<script lang="ts">
	// A plain mouse seen from above, for mice without their own drawing: left and right buttons, the wheel
	// (click, up, down, and tilt when the keymap has it) and two side buttons. Only buttons the keymap
	// lists are drawn as clickable; the list next to the drawing carries everything else.
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
		return `hot${x?.selected ? ' sel' : ''}${x?.gel ? ' gel' : ''}`;
	};
	const gelColor = (name: string) => (r(name)?.gel ? `var(--color-gel-${r(name)!.gel})` : undefined);
	function key(e: KeyboardEvent, name: string) {
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			onselect(name);
		}
	}
	const tilt = $derived(!!r('WHEEL_LEFT') || !!r('WHEEL_RIGHT'));
</script>

{#snippet hot(name: string, d: string, tx: number, ty: number, text: string)}
	{#if r(name)}
		<g class={cls(name)} role="button" tabindex="0" aria-label={r(name)!.label} aria-pressed={r(name)!.selected} onclick={() => onselect(name)} onkeydown={(e) => key(e, name)} style:--gel={gelColor(name)}>
			<path {d} />
			{#if text}<text x={tx} y={ty} text-anchor="middle" dominant-baseline="central">{text}</text>{/if}
		</g>
	{/if}
{/snippet}

<div class="mouse">
	<svg viewBox="-40 0 320 420" role="group" aria-label="Mouse buttons">
		<path class="body" d="M120 14 C 66 14 40 58 38 132 C 36 214 40 300 58 350 C 72 390 96 406 120 406 C 144 406 168 390 182 350 C 200 300 204 214 202 132 C 200 58 174 14 120 14 Z" />
		{@render hot('LEFT_CLICK', 'M117 18 C 70 20 44 62 42 132 L 42 176 L 117 176 Z', 80, 124, 'L')}
		{@render hot('RIGHT_CLICK', 'M123 18 C 170 20 196 62 198 132 L 198 176 L 123 176 Z', 160, 124, 'R')}
		{@render hot('WHEEL_UP', 'M110 38 L 130 38 L 120 28 Z', 0, 0, '')}
		{@render hot('WHEEL_CLICK', 'M109 50 Q 109 44 115 44 L 125 44 Q 131 44 131 50 L 131 106 Q 131 112 125 112 L 115 112 Q 109 112 109 106 Z', 120, 78, '')}
		{@render hot('WHEEL_DOWN', 'M110 118 L 130 118 L 120 128 Z', 0, 0, '')}
		{@render hot('WHEEL_LEFT', 'M101 69 L 101 87 L 91 78 Z', 0, 0, '')}
		{@render hot('WHEEL_RIGHT', 'M139 69 L 139 87 L 149 78 Z', 0, 0, '')}
		{@render hot('FORWARD', 'M28 188 Q 28 182 34 182 L 41 182 L 41 228 L 34 228 Q 28 228 28 222 Z', 0, 0, '')}
		{@render hot('BACK', 'M28 240 Q 28 234 34 234 L 41 234 L 41 280 L 34 280 Q 28 280 28 274 Z', 0, 0, '')}

		{#if r('FORWARD')}<g class="callout"><line x1="26" y1="205" x2="-2" y2="205" /><text x="-6" y="205" text-anchor="end" dominant-baseline="central">Forward</text></g>{/if}
		{#if r('BACK')}<g class="callout"><line x1="26" y1="257" x2="-2" y2="257" /><text x="-6" y="257" text-anchor="end" dominant-baseline="central">Back</text></g>{/if}
		{#if r('WHEEL_CLICK')}<g class="callout"><line x1={tilt ? 152 : 134} y1="78" x2="222" y2="78" /><text x="226" y="78" dominant-baseline="central">Wheel</text></g>{/if}
	</svg>
</div>

<style>
	.mouse {
		display: grid;
		justify-items: center;
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
</style>
