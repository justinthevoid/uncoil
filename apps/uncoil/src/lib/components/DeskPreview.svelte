<script lang="ts">
	// The desk to scale, lit with live colours from the real effect engine: the mat's edge glows, the
	// keyboard's caps and underglow light up, the mouse's LEDs shine. Away devices are dimmed.
	import Keyboard from './Keyboard.svelte';
	import type { DeskDevice } from '#lib/types.ts';

	interface Props {
		desk: DeskDevice[];
		/** Per device, per shape (same order as desk). */
		colors: string[][];
		/** Device ids to dim (unplugged / not reported by the engine). */
		away?: Set<string>;
	}
	let { desk, colors, away = new Set() }: Props = $props();

	const bounds = $derived.by(() => {
		const xs = desk.flatMap((d) => [d.x, d.x + d.w]);
		const ys = desk.flatMap((d) => [d.y, d.y + d.h]);
		const x0 = Math.min(...xs) - 0.4;
		const y0 = Math.min(...ys) - 0.4;
		return { x0, y0, w: Math.max(...xs) + 0.4 - x0, h: Math.max(...ys) + 0.4 - y0 };
	});
	const box = (d: { x: number; y: number; w: number; h: number }) => {
		const b = bounds;
		return `left:${((d.x - b.x0) / b.w) * 100}%;top:${((d.y - b.y0) / b.h) * 100}%;width:${(d.w / b.w) * 100}%;height:${(d.h / b.h) * 100}%`;
	};
	// Draw the mat first, then everything sitting on it.
	const order = $derived(desk.map((d, i) => ({ d, i })).sort((a, b) => Number(b.d.kind === 'mousemat') - Number(a.d.kind === 'mousemat')));
</script>

{#if desk.length}
	<div class="fit">
	<div class="desk" style:aspect-ratio="{bounds.w} / {bounds.h}" style:--ar={bounds.w / bounds.h} role="img" aria-label="Your desk, lit with the current effect">
		{#each order as { d, i } (d.id)}
			{#if d.kind === 'mousemat'}
				<div class="mat" class:away={away.has(d.id)} style={box(d)} style:--c={colors[i]?.[0] ?? 'transparent'}></div>
			{:else if d.kind === 'keyboard'}
				<div class="dev" class:away={away.has(d.id)} style={box({ x: d.x - 0.35, y: d.y - 0.35, w: d.w + 0.7, h: d.h + 0.7 })}>
					<Keyboard device={d} colors={colors[i] ?? []} />
				</div>
			{:else}
				<div class="dev mouse" class:away={away.has(d.id)} style={box(d)}>
					{#each d.shapes as s, k (s.name)}
						<span
							class="mled"
							style:left="{((s.x - d.x) / d.w) * 100}%"
							style:top="{((s.y - d.y) / d.h) * 100}%"
							style:--c={colors[i]?.[k] ?? 'var(--color-seam-2)'}
						></span>
					{/each}
				</div>
			{/if}
		{/each}
	</div>
	</div>
{/if}

<style>
	/* Fill the parent's box (both directions) while keeping the desk's proportions. */
	.fit {
		width: 100%;
		height: 100%;
		min-height: 0;
		container-type: size;
		display: grid;
		place-items: center;
	}
	.desk {
		position: relative;
		width: min(100cqw, calc(100cqh * var(--ar)));
	}
	.mat,
	.dev {
		position: absolute;
		transition: opacity var(--t-slow) var(--ease);
	}
	.away {
		opacity: 0.3;
	}
	.mat {
		border-radius: 1.6%/4%;
		background: var(--case);
		box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--c) 65%, var(--case));
		transition:
			box-shadow 120ms linear,
			opacity var(--t-slow) var(--ease);
	}
	.mouse {
		border-radius: 48% 48% 44% 44% / 30% 30% 22% 22%;
		background: var(--cap);
		box-shadow: inset 0 0 0 1px var(--cap-edge), inset 0 -4px 0 var(--cap-edge);
	}
	.mled {
		position: absolute;
		width: 9%;
		aspect-ratio: 1;
		translate: -50% -50%;
		border-radius: 50%;
		background: var(--c);
		transition: background-color 120ms linear;
	}
</style>
