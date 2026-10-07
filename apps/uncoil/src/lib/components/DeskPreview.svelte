<script lang="ts" module>
	export interface Hit {
		device: string;
		shape: string;
		x: number;
		y: number;
	}
</script>

<script lang="ts">
	// The desk to scale, lit with live colours from the real effect engine, each device drawn from above in
	// its real black finish: the mat with its lit edge, the keyboard's caps and side underglow, the mouse's
	// wheel, logo and strip, and OpenRGB's PC devices as panels. Away devices are dimmed.
	import Keyboard from './Keyboard.svelte';
	import BasiliskArt from './BasiliskArt.svelte';
	import { BODY_BOX, VIEW } from '#lib/art/basilisk.ts';
	import { boardExtent } from '#lib/art/keyboards.ts';
	import type { DeskDevice } from '#lib/types.ts';

	interface Props {
		desk: DeskDevice[];
		/** Per device, per shape (same order as desk). */
		colors: string[][];
		/** Device ids to dim (unplugged / not reported by the engine). */
		away?: Set<string>;
		/** LEDs to outline, as "deviceId/shape". */
		marked?: Set<string>;
		/** Devices to outline as chosen (per-device lighting). */
		chosen?: Set<string>;
		/** Pointer picks: the LED nearest the pointer. `phase` is start on press, move while dragging. */
		onpick?: (hit: Hit, phase: 'start' | 'move') => void;
		/** A click on a device (when nothing is being picked); `add` with Ctrl or Shift held. */
		ondevice?: (id: string, add: boolean) => void;
		/** Accessible description of what clicking does. */
		pickLabel?: string;
	}
	let { desk, colors, away = new Set(), marked, chosen, onpick, ondevice, pickLabel }: Props = $props();

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
		if (!onpick && !ondevice) return;
		const h = hitAt(e);
		if (!h) return;
		if (!onpick) {
			ondevice?.(h.device, e.ctrlKey || e.shiftKey || e.metaKey);
			return;
		}
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
		const x0 = Math.min(...xs) - 0.6;
		const y0 = Math.min(...ys) - 0.6;
		return { x0, y0, w: Math.max(...xs) + 0.6 - x0, h: Math.max(...ys) + 0.6 - y0 };
	});
	const box = (d: { x: number; y: number; w: number; h: number }) => {
		const b = bounds;
		return `left:${((d.x - b.x0) / b.w) * 100}%;top:${((d.y - b.y0) / b.h) * 100}%;width:${(d.w / b.w) * 100}%;height:${(d.h / b.h) * 100}%`;
	};
	/** The Basilisk drawing over its desk box: the body's length is the box's depth, centred on the box. */
	const basiliskBox = (d: DeskDevice) => {
		const u = d.h / BODY_BOX.h; // key units per drawing pixel
		const cx = BODY_BOX.x + BODY_BOX.w / 2;
		const cy = BODY_BOX.y + BODY_BOX.h / 2;
		return box({ x: d.x + d.w / 2 - (cx - VIEW.x) * u, y: d.y + d.h / 2 - (cy - VIEW.y) * u, w: VIEW.w * u, h: VIEW.h * u });
	};
	const isBasilisk = (d: DeskDevice) => d.id.startsWith('razer-basilisk-v3');
	const pc = (d: DeskDevice) => d.id.startsWith('openrgb:');
	// Draw the mat first, then everything sitting on it.
	const order = $derived(desk.map((d, i) => ({ d, i })).sort((a, b) => Number(b.d.kind === 'mousemat') - Number(a.d.kind === 'mousemat')));
	const colorOf = (i: number, d: DeskDevice) => (shape: string) => {
		const k = d.shapes.findIndex((s) => s.name === shape);
		return k < 0 ? undefined : colors[i]?.[k];
	};
	/** The outline drawn round a chosen device. */
	const chosenBox = (d: DeskDevice) => (isBasilisk(d) ? box({ x: d.x - 0.35, y: d.y - 0.25, w: d.w + 0.7, h: d.h + 0.5 }) : box({ x: d.x - 0.15, y: d.y - 0.15, w: d.w + 0.3, h: d.h + 0.3 }));
</script>

{#if desk.length}
	<div class="fit">
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="desk device-finish"
		class:pickable={!!onpick}
		class:choosable={!onpick && !!ondevice}
		bind:this={deskEl}
		style:aspect-ratio="{bounds.w} / {bounds.h}"
		style:--ar={bounds.w / bounds.h}
		style:--u="calc(100cqw / {bounds.w})"
		role={onpick || ondevice ? 'application' : 'img'}
		aria-label={onpick || ondevice ? (pickLabel ?? 'Your desk') : 'Your desk, lit with the current effect'}
		onpointerdown={down}
		onpointermove={move}
		onpointerup={up}
		onpointercancel={up}
	>
		{#each order as { d, i } (d.id)}
			{#if d.kind === 'mousemat'}
				<div class="mat" class:away={away.has(d.id)} style={box(d)} style:--c={colors[i]?.[0] ?? 'var(--led-off)'}>
					<span class="hub"></span>
				</div>
			{:else if d.kind === 'keyboard'}
				{@const e = boardExtent(d)}
				<div class="dev" class:away={away.has(d.id)} style={box({ x: e.x0, y: e.y0, w: e.x1 - e.x0, h: e.y1 - e.y0 })}>
					<Keyboard device={d} colors={colors[i] ?? []} />
				</div>
			{:else if isBasilisk(d)}
				<div class="dev" class:away={away.has(d.id)} style={basiliskBox(d)}>
					<BasiliskArt color={colorOf(i, d)} />
				</div>
			{:else}
				<div class="dev" class:mouse={!pc(d)} class:pc={pc(d)} class:away={away.has(d.id)} style={box(d)}>
					{#each d.shapes as s, k (s.name)}
						<span
							class="mled"
							style:left="{((s.x - d.x) / d.w) * 100}%"
							style:top="{((s.y - d.y) / d.h) * 100}%"
							style:--c={colors[i]?.[k] ?? 'var(--led-off)'}
						></span>
					{/each}
				</div>
			{/if}
		{/each}
		{#each desk as d (d.id)}
			{#if chosen?.has(d.id)}<span class="chosen" class:mat-mark={d.kind === 'mousemat'} style={chosenBox(d)}></span>{/if}
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
	.desk {
		position: relative;
		width: min(100cqw, calc(100cqh * var(--ar)));
		container-type: inline-size;
	}
	.desk.pickable {
		cursor: crosshair;
		touch-action: none;
	}
	.desk.choosable {
		cursor: pointer;
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
		border-radius: calc(var(--u) * 0.5);
		box-shadow: inset 0 0 0 2px var(--color-select);
	}
	.chosen.mat-mark {
		border-radius: calc(var(--u) * 0.5);
	}
	/* two-tone, so it reads on the black desk and on the page round the mat */
	.chosen {
		position: absolute;
		border-radius: calc(var(--u) * 0.45);
		outline: 2px dashed #f4f1ec;
		box-shadow: 0 0 0 2px rgb(15 14 13 / 0.85);
		pointer-events: none;
	}
	.mat,
	.dev {
		position: absolute;
		transition: opacity var(--t-slow) var(--ease);
	}
	.away {
		opacity: 0.3;
	}
	/* The cloth, with its lit edge ring and the cable hub at the back left. */
	.mat {
		border-radius: calc(var(--u) * 0.5);
		background: var(--cloth);
		box-shadow:
			inset 0 0 0 calc(var(--u) * 0.16) var(--c),
			inset 0 0 0 calc(var(--u) * 0.16 + 1px) rgb(0 0 0 / 0.35);
		transition:
			box-shadow 120ms linear,
			opacity var(--t-slow) var(--ease);
	}
	.hub {
		position: absolute;
		top: calc(var(--u) * -0.22);
		left: calc(var(--u) * 1.1);
		width: calc(var(--u) * 1.5);
		height: calc(var(--u) * 0.4);
		border-radius: calc(var(--u) * 0.16);
		background: var(--shell-top);
		box-shadow: 0 0 0 1px var(--case-edge);
	}
	.mouse {
		border-radius: 48% 48% 44% 44% / 30% 30% 22% 22%;
		background: var(--case);
		box-shadow: 0 0 0 1px var(--case-edge);
	}
	/* An OpenRGB device: a plain panel with its LEDs where OpenRGB lays them out. */
	.pc {
		border-radius: calc(var(--u) * 0.18);
		background: var(--case);
		box-shadow: 0 0 0 1px var(--case-edge);
	}
	.mled {
		position: absolute;
		width: calc(var(--u) * 0.32);
		aspect-ratio: 1;
		translate: -50% -50%;
		border-radius: 50%;
		background: var(--c);
		transition: background-color 120ms linear;
	}
	.pc .mled {
		width: calc(var(--u) * 0.24);
		border-radius: 2px;
	}
</style>
