<script lang="ts" module>
	export interface Hit {
		device: string;
		shape: string;
		x: number;
		y: number;
	}
</script>

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
		/** LEDs to outline, as "deviceId/shape". */
		marked?: Set<string>;
		/** Pointer picks: the LED nearest the pointer. `phase` is start on press, move while dragging. */
		onpick?: (hit: Hit, phase: 'start' | 'move') => void;
		/** Accessible description of what clicking does. */
		pickLabel?: string;
	}
	let { desk, colors, away = new Set(), marked, onpick, pickLabel }: Props = $props();

	let deskEl: HTMLDivElement | undefined = $state();
	let dragging = false;
	let lastKey = '';
	/** The LED under (or nearest to, within ~1 key) a pointer position. */
	function hitAt(e: PointerEvent): Hit | null {
		if (!deskEl) return null;
		const r = deskEl.getBoundingClientRect();
		const x = bounds.x0 + ((e.clientX - r.left) / r.width) * bounds.w;
		const y = bounds.y0 + ((e.clientY - r.top) / r.height) * bounds.h;
		let best: Hit | null = null;
		let bestD = 1.1;
		for (const d of desk) {
			if (d.kind === 'mousemat') continue;
			for (const s of d.shapes) {
				const dx = Math.max(Math.abs(x - s.x) - s.w / 2, 0);
				const dy = Math.max(Math.abs(y - s.y) - s.h / 2, 0);
				const dist = Math.hypot(dx, dy);
				if (dist < bestD) {
					bestD = dist;
					best = { device: d.id, shape: s.name, x: s.x, y: s.y };
				}
			}
		}
		if (best) return best;
		// Nothing close: the mat, if the pointer is on it.
		const mat = desk.find((d) => d.kind === 'mousemat');
		if (mat && x >= mat.x && x <= mat.x + mat.w && y >= mat.y && y <= mat.y + mat.h) return { device: mat.id, shape: mat.shapes[0]?.name ?? 'Edge', x, y };
		return null;
	}
	function down(e: PointerEvent) {
		if (!onpick) return;
		const h = hitAt(e);
		if (!h) return;
		dragging = true;
		lastKey = `${h.device}/${h.shape}`;
		(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
		onpick(h, 'start');
	}
	function move(e: PointerEvent) {
		if (!onpick || !dragging) return;
		const h = hitAt(e);
		if (!h) return;
		const k = `${h.device}/${h.shape}`;
		if (k === lastKey) return;
		lastKey = k;
		onpick(h, 'move');
	}
	const up = () => (dragging = false);

	// Outlines for marked LEDs (desk units → % of the desk box).
	const outlines = $derived.by(() => {
		if (!marked?.size) return [];
		const out: { key: string; style: string; mat: boolean }[] = [];
		for (const d of desk) {
			for (const s of d.shapes) {
				const key = `${d.id}/${s.name}`;
				if (!marked.has(key)) continue;
				if (d.kind === 'mousemat') out.push({ key, style: box(d), mat: true });
				else {
					const w = s.is_key ? s.w : 0.5;
					const h = s.is_key ? s.h : 0.5;
					out.push({ key, style: box({ x: s.x - w / 2, y: s.y - h / 2, w, h }), mat: false });
				}
			}
		}
		return out;
	});

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
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="desk"
		class:pickable={!!onpick}
		bind:this={deskEl}
		style:aspect-ratio="{bounds.w} / {bounds.h}"
		style:--ar={bounds.w / bounds.h}
		role={onpick ? 'application' : 'img'}
		aria-label={onpick ? (pickLabel ?? 'Your desk') : 'Your desk, lit with the current effect'}
		onpointerdown={down}
		onpointermove={move}
		onpointerup={up}
		onpointercancel={up}
	>
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
		{#each outlines as o (o.key)}
			<span class="mark" class:mat-mark={o.mat} style={o.style}></span>
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
	.desk.pickable {
		cursor: crosshair;
		touch-action: none;
	}
	.mark {
		position: absolute;
		border-radius: 4px;
		box-shadow:
			0 0 0 2px var(--color-select),
			0 0 0 4px var(--color-ground);
		pointer-events: none;
	}
	.mark.mat-mark {
		border-radius: 1.6%/4%;
		box-shadow: inset 0 0 0 2px var(--color-select);
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
