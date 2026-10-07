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
	// wheel, logo and strip, and the PC's OpenRGB devices together as one PC, its parts where they sit inside.
	// Away devices are dimmed. It is a canvas: the wheel zooms, dragging empty space pans, and with Arrange on
	// (when `onmove` is given) dragging a device moves it, so the effects cross the desk the way it really is.
	import { tick, untrack } from 'svelte';
	import { Maximize, Minus, Move, Plus, RotateCcw } from '@lucide/svelte';
	import Keyboard from './Keyboard.svelte';
	import MouseArt from './MouseArt.svelte';
	import MatArt from './MatArt.svelte';
	import DockArt from './DockArt.svelte';
	import PcInterior from './PcInterior.svelte';
	import { mouseArt } from '#lib/art/mice.ts';
	import { boardExtent } from '#lib/art/keyboards.ts';
	import { isPc, snap } from '#lib/desk.ts';
	import { pcParts } from '#lib/pc.ts';
	import { app } from '#lib/state.svelte.ts';
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
		/** Arrange: devices dragged (or nudged with the arrow keys) by (dx, dy) key units. */
		onmove?: (ids: string[], dx: number, dy: number) => void;
		/** Arrange: put every device back where uncoil places it. */
		onreset?: () => void;
	}
	let { desk, colors, away = new Set(), marked, chosen, onpick, ondevice, pickLabel, onmove, onreset }: Props = $props();

	let fitEl: HTMLDivElement | undefined = $state();
	let deskEl: HTMLDivElement | undefined = $state();

	// ---- the PC: OpenRGB's devices drawn as one tower, right of where the daemon stacks them ------------
	const pcDevs = $derived(desk.filter((d) => isPc(d.id)));
	const kbDepth = $derived.by(() => {
		const kb = desk.find((d) => d.kind === 'keyboard');
		if (!kb) return 7;
		const e = boardExtent(kb);
		return e.y1 - e.y0;
	});
	/** The tower's desk box: as tall as the keyboard (6 to 9 keys), its right edge and top where the column is. */
	const pcBox = $derived.by(() => {
		if (!pcDevs.length) return null;
		const right = Math.max(...pcDevs.map((d) => d.x + d.w));
		const top = Math.min(...pcDevs.map((d) => d.y));
		const h = Math.min(9, Math.max(6, kbDepth));
		const w = (h * 580) / 620;
		return { x: right - w, y: top, w, h };
	});
	const parts = $derived(pcParts(app.status?.openrgb?.devices ?? []));
	function pcColor(device: string, i: number) {
		const k = desk.findIndex((d) => d.id === device);
		return k < 0 ? undefined : colors[k]?.[i];
	}

	// ---- what sits where, in desk units ---------------------------------------------------------------
	const dock = (d: DeskDevice) => /mouse-dock|base-station/.test(d.id);
	const traced = (d: DeskDevice) => d.kind === 'mouse' && !!mouseArt(d.id);
	/** The box a device is drawn in. */
	function frame(d: DeskDevice) {
		if (d.kind === 'keyboard') {
			const e = boardExtent(d);
			return { x: e.x0, y: e.y0, w: e.x1 - e.x0, h: e.y1 - e.y0 };
		}
		if (traced(d)) {
			const a = mouseArt(d.id)!;
			const u = d.h / a.body_box.h; // key units per drawing pixel
			const cx = a.body_box.x + a.body_box.w / 2;
			const cy = a.body_box.y + a.body_box.h / 2;
			return { x: d.x + d.w / 2 - (cx - a.view.x) * u, y: d.y + d.h / 2 - (cy - a.view.y) * u, w: a.view.w * u, h: a.view.h * u };
		}
		return { x: d.x, y: d.y, w: d.w, h: d.h };
	}
	/** What can be moved: each device, and the PC as one. Mats first, so what sits on them is on top. */
	const items = $derived.by(() => {
		const out: { key: string; label: string; ids: string[]; box: { x: number; y: number; w: number; h: number } }[] = [];
		for (const d of desk.filter((d) => !isPc(d.id)).sort((a, b) => Number(b.kind === 'mousemat') - Number(a.kind === 'mousemat')))
			out.push({ key: d.id, label: d.name, ids: [d.id], box: frame(d) });
		if (pcBox) out.push({ key: 'pc', label: 'PC', ids: pcDevs.map((d) => d.id), box: pcBox });
		return out;
	});

	const bounds = $derived.by(() => {
		const boxes = items.map((i) => i.box);
		if (!boxes.length) return { x0: 0, y0: 0, w: 1, h: 1 };
		const x0 = Math.min(...boxes.map((b) => b.x)) - 0.6;
		const y0 = Math.min(...boxes.map((b) => b.y)) - 0.6;
		return { x0, y0, w: Math.max(...boxes.map((b) => b.x + b.w)) + 0.6 - x0, h: Math.max(...boxes.map((b) => b.y + b.h)) + 0.6 - y0 };
	});
	const box = (d: { x: number; y: number; w: number; h: number }) => {
		const b = bounds;
		return `left:${((d.x - b.x0) / b.w) * 100}%;top:${((d.y - b.y0) / b.h) * 100}%;width:${(d.w / b.w) * 100}%;height:${(d.h / b.h) * 100}%`;
	};

	// ---- zoom and pan: a transform over the fitted desk ------------------------------------------------
	let zoom = $state(1);
	let pan = $state({ x: 0, y: 0 });
	const fitted = $derived(zoom === 1 && pan.x === 0 && pan.y === 0);
	function zoomAt(cx: number, cy: number, z: number) {
		if (!deskEl) return;
		z = Math.min(6, Math.max(0.5, z));
		const r = deskEl.getBoundingClientRect();
		const k = z / zoom;
		pan = { x: pan.x + (cx - r.left) * (1 - k), y: pan.y + (cy - r.top) * (1 - k) };
		zoom = z;
	}
	function zoomBy(f: number) {
		if (!fitEl) return;
		const r = fitEl.getBoundingClientRect();
		zoomAt(r.left + r.width / 2, r.top + r.height / 2, zoom * f);
	}
	function fit() {
		zoom = 1;
		pan = { x: 0, y: 0 };
	}
	$effect(() => {
		const el = fitEl;
		if (!el) return;
		const wheel = (e: WheelEvent) => {
			e.preventDefault();
			zoomAt(e.clientX, e.clientY, zoom * Math.exp(-e.deltaY * (e.deltaMode === 1 ? 0.05 : 0.0015)));
		};
		el.addEventListener('wheel', wheel, { passive: false });
		return () => el.removeEventListener('wheel', wheel);
	});
	// When the desk's extent changes (a device moved), a fitted view fits the new desk; a zoomed or panned one
	// keeps what is on screen where it was.
	let before: { b: typeof bounds; r: DOMRect } | null = null;
	let shown = untrack(() => bounds);
	$effect.pre(() => {
		const b = bounds;
		untrack(() => {
			if (deskEl && !fitted) before = { b: shown, r: deskEl.getBoundingClientRect() };
			shown = b;
		});
		tick().then(() => {
			const was = before;
			before = null;
			if (!was || !deskEl) return;
			const r = deskEl.getBoundingClientRect();
			const k = (was.r.width / was.b.w) / (r.width / b.w);
			if (!Number.isFinite(k) || k <= 0) return;
			const unit = was.r.width / was.b.w;
			zoom = zoom * k;
			// with the origin at the top left, scaling leaves the left edge put
			pan = { x: pan.x + was.r.left + (b.x0 - was.b.x0) * unit - r.left, y: pan.y + was.r.top + (b.y0 - was.b.y0) * unit - r.top };
		});
	});

	// ---- pointer: pick, choose or pan; with Arrange, move -------------------------------------------
	let arranging = $state(false);
	let picking = false;
	let lastKey = '';
	let panFrom: { x: number; y: number; px: number; py: number } | null = null;
	let panning = $state(false);

	/** The LED under (or nearest to, within ~1 key) a pointer position. The PC's LEDs aren't picked here. */
	function hitAt(e: PointerEvent): Hit | null {
		if (!deskEl) return null;
		const r = deskEl.getBoundingClientRect();
		const x = bounds.x0 + ((e.clientX - r.left) / r.width) * bounds.w;
		const y = bounds.y0 + ((e.clientY - r.top) / r.height) * bounds.h;
		let best: Hit | null = null;
		let bestD = 1.1;
		for (const d of desk) {
			if (d.kind === 'mousemat' || isPc(d.id)) continue;
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
		// Nothing close: the PC or the mat, if the pointer is on it.
		if (pcBox && x >= pcBox.x && x <= pcBox.x + pcBox.w && y >= pcBox.y && y <= pcBox.y + pcBox.h) return { device: pcDevs[0].id, shape: '', x, y };
		const mat = desk.find((d) => d.kind === 'mousemat');
		if (mat && x >= mat.x && x <= mat.x + mat.w && y >= mat.y && y <= mat.y + mat.h) return { device: mat.id, shape: mat.shapes[0]?.name ?? 'Edge', x, y };
		return null;
	}
	function startPan(e: PointerEvent) {
		panFrom = { x: e.clientX, y: e.clientY, px: pan.x, py: pan.y };
		panning = true;
		fitEl?.setPointerCapture(e.pointerId);
	}
	function down(e: PointerEvent) {
		if ((e.target as Element).closest('.tools, .handle')) return;
		if (e.button === 1 || arranging) return startPan(e);
		if (e.button !== 0) return;
		const h = onpick || ondevice ? hitAt(e) : null;
		if (!h || (onpick && !h.shape)) return startPan(e);
		if (!onpick) {
			ondevice?.(h.device, e.ctrlKey || e.shiftKey || e.metaKey);
			return;
		}
		picking = true;
		lastKey = `${h.device}/${h.shape}`;
		fitEl?.setPointerCapture(e.pointerId);
		onpick(h, 'start');
	}
	function move(e: PointerEvent) {
		if (panFrom) {
			pan = { x: panFrom.px + e.clientX - panFrom.x, y: panFrom.py + e.clientY - panFrom.y };
			return;
		}
		if (!onpick || !picking) return;
		const h = hitAt(e);
		if (!h || !h.shape) return;
		const k = `${h.device}/${h.shape}`;
		if (k === lastKey) return;
		lastKey = k;
		onpick(h, 'move');
	}
	function up() {
		picking = false;
		panFrom = null;
		panning = false;
	}

	// Dragging a device (Arrange): a live offset until it's dropped, then onmove writes the new place.
	let drag = $state<{ key: string; ids: string[]; from: { x: number; y: number }; dx: number; dy: number } | null>(null);
	const unitPx = () => (deskEl ? deskEl.getBoundingClientRect().width / bounds.w : 1);
	function grab(e: PointerEvent, it: (typeof items)[number]) {
		if (e.button !== 0) return;
		e.stopPropagation();
		(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
		drag = { key: it.key, ids: it.ids, from: { x: e.clientX, y: e.clientY }, dx: 0, dy: 0 };
	}
	function dragMove(e: PointerEvent) {
		if (!drag) return;
		const u = unitPx();
		const fine = e.altKey;
		drag.dx = snap((e.clientX - drag.from.x) / u, fine);
		drag.dy = snap((e.clientY - drag.from.y) / u, fine);
	}
	function drop() {
		if (!drag) return;
		const { ids, dx, dy } = drag;
		drag = null;
		if (dx || dy) onmove?.(ids, dx, dy);
	}
	function nudge(e: KeyboardEvent, it: (typeof items)[number]) {
		const step = e.shiftKey ? 1 : 0.25;
		const d = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[e.key];
		if (!d) return;
		e.preventDefault();
		onmove?.(it.ids, d[0], d[1]);
	}
	const shiftOf = (id: string) => (drag && drag.ids.includes(id) ? `calc(var(--u) * ${drag.dx}) calc(var(--u) * ${drag.dy})` : undefined);
	const pcShift = $derived(drag?.key === 'pc' ? `calc(var(--u) * ${drag.dx}) calc(var(--u) * ${drag.dy})` : undefined);

	// ---- marks --------------------------------------------------------------------------------------
	const outlines = $derived.by(() => {
		if (!marked?.size) return [];
		const out: { key: string; style: string; mat: boolean }[] = [];
		for (const d of desk) {
			if (isPc(d.id)) continue;
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
	// Draw the mat first, then everything sitting on it.
	const order = $derived(
		desk
			.map((d, i) => ({ d, i }))
			.filter(({ d }) => !isPc(d.id))
			.sort((a, b) => Number(b.d.kind === 'mousemat') - Number(a.d.kind === 'mousemat'))
	);
	const colorOf = (i: number, d: DeskDevice) => (shape: string) => {
		const k = d.shapes.findIndex((s) => s.name === shape);
		return k < 0 ? undefined : colors[i]?.[k];
	};
	/** The outline drawn round a chosen device. */
	const chosenBox = (d: DeskDevice) => (traced(d) ? box({ x: d.x - 0.35, y: d.y - 0.25, w: d.w + 0.7, h: d.h + 0.5 }) : box({ x: d.x - 0.15, y: d.y - 0.15, w: d.w + 0.3, h: d.h + 0.3 }));
	const pcChosen = $derived(!!chosen && pcDevs.some((d) => chosen.has(d.id)));
	const pcAway = $derived(pcDevs.length > 0 && app.status?.openrgb?.state !== 'connected');
	const interactive = $derived(arranging || !!onpick || !!ondevice);
</script>

{#if desk.length}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="fit"
		class:panning
		class:pickable={!arranging && !!onpick}
		class:choosable={!arranging && !onpick && !!ondevice}
		bind:this={fitEl}
		onpointerdown={down}
		onpointermove={move}
		onpointerup={up}
		onpointercancel={up}
		ondblclick={(e) => e.target === fitEl && fit()}
	>
		<div
			class="desk device-finish"
			class:arranging
			bind:this={deskEl}
			style:aspect-ratio="{bounds.w} / {bounds.h}"
			style:--ar={bounds.w / bounds.h}
			style:--u="calc(100cqw / {bounds.w})"
			style:transform="translate({pan.x}px, {pan.y}px) scale({zoom})"
			role={interactive ? 'application' : 'img'}
			aria-label={arranging ? 'Your desk. Drag a device, or focus it and use the arrow keys, to move it.' : interactive ? (pickLabel ?? 'Your desk') : 'Your desk, lit with the current effect'}
		>
			{#each order as { d, i } (d.id)}
				<div class="dev" class:away={away.has(d.id)} class:mouse={!traced(d) && !dock(d) && d.kind !== 'mousemat' && d.kind !== 'keyboard'} style={box(frame(d))} style:translate={shiftOf(d.id)}>
					{#if d.kind === 'mousemat'}
						<MatArt device={d} colors={colors[i] ?? []} />
					{:else if d.kind === 'keyboard'}
						<Keyboard device={d} colors={colors[i] ?? []} />
					{:else if dock(d)}
						<DockArt device={d} colors={colors[i] ?? []} />
					{:else if traced(d)}
						<MouseArt art={mouseArt(d.id)!} shapes={d.shapes.map((s) => s.name)} color={colorOf(i, d)} />
					{:else}
						{#each d.shapes as s, k (s.name)}
							<span class="mled" style:left="{((s.x - d.x) / d.w) * 100}%" style:top="{((s.y - d.y) / d.h) * 100}%" style:--c={colors[i]?.[k] ?? 'var(--led-off)'}></span>
						{/each}
					{/if}
				</div>
			{/each}
			{#if pcBox}
				<div class="dev pc" class:away={pcAway} style={box(pcBox)} style:translate={pcShift}>
					<PcInterior {parts} color={pcColor} />
				</div>
			{/if}
			{#if !arranging}
				{#each desk as d (d.id)}
					{#if chosen?.has(d.id) && !isPc(d.id)}<span class="chosen" class:mat-mark={d.kind === 'mousemat'} style={chosenBox(d)}></span>{/if}
				{/each}
				{#if pcChosen && pcBox}<span class="chosen" style={box({ x: pcBox.x - 0.15, y: pcBox.y - 0.15, w: pcBox.w + 0.3, h: pcBox.h + 0.3 })}></span>{/if}
				{#each outlines as o (o.key)}
					<span class="mark" class:mat-mark={o.mat} style={o.style}></span>
				{/each}
			{:else}
				{#each items as it (it.key)}
					<button
						type="button"
						class="handle"
						class:held={drag?.key === it.key}
						style={box(it.box)}
						style:translate={drag?.key === it.key ? `calc(var(--u) * ${drag.dx}) calc(var(--u) * ${drag.dy})` : undefined}
						aria-label="Move {it.label}"
						title="{it.label}: drag to move (Alt: finer steps), or arrow keys (Shift: a whole key)"
						onpointerdown={(e) => grab(e, it)}
						onpointermove={dragMove}
						onpointerup={drop}
						onpointercancel={() => (drag = null)}
						onkeydown={(e) => nudge(e, it)}
					><span class="tag">{it.label}</span></button>
				{/each}
			{/if}
		</div>

		<div class="tools" role="toolbar" aria-label="Desk view">
			{#if onmove}
				<button type="button" class="tool" class:on={arranging} aria-pressed={arranging} onclick={() => (arranging = !arranging)} title="Move devices to where they sit on your desk; the effects follow">
					<Move size={14} />{arranging ? 'Done' : 'Arrange'}
				</button>
				{#if arranging && onreset}
					<button type="button" class="tool" onclick={onreset} title="Put every device back where uncoil places it"><RotateCcw size={14} />Reset</button>
				{/if}
			{/if}
			<button type="button" class="tool icon" onclick={() => zoomBy(1 / 1.25)} aria-label="Zoom out" disabled={zoom <= 0.5}><Minus size={14} /></button>
			<span class="zoom" aria-live="polite">{Math.round(zoom * 100)}%</span>
			<button type="button" class="tool icon" onclick={() => zoomBy(1.25)} aria-label="Zoom in" disabled={zoom >= 6}><Plus size={14} /></button>
			<button type="button" class="tool icon" onclick={fit} aria-label="Fit the desk" title="Fit the desk (or double-click empty space)" disabled={fitted}><Maximize size={14} /></button>
		</div>
		{#if arranging}
			<p class="arrange-hint">Drag devices to where they sit on your desk. Effects like the wave follow.</p>
		{/if}
	</div>
{/if}

<style>
	/* Fill the parent's box (both directions) while keeping the desk's proportions; the desk zooms and pans
	   inside it. */
	.fit {
		position: relative;
		width: 100%;
		height: 100%;
		min-height: 0;
		container-type: size;
		display: grid;
		place-items: center;
		overflow: hidden;
		touch-action: none;
		cursor: grab;
	}
	.fit.panning {
		cursor: grabbing;
	}
	.fit.pickable {
		cursor: crosshair;
	}
	.fit.choosable {
		cursor: pointer;
	}
	.desk {
		position: relative;
		width: min(100cqw, calc(100cqh * var(--ar)));
		container-type: inline-size;
		transform-origin: 0 0;
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
	.dev {
		position: absolute;
		transition: opacity var(--t-slow) var(--ease);
	}
	.away {
		opacity: 0.3;
	}
	.mouse {
		border-radius: 48% 48% 44% 44% / 30% 30% 22% 22%;
		background: var(--case);
		box-shadow: 0 0 0 1px var(--case-edge);
	}
	.pc :global(svg) {
		display: block;
		width: 100%;
		height: 100%;
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
	/* Arrange: each device is a handle over its drawing */
	.handle {
		position: absolute;
		border-radius: calc(var(--u) * 0.35);
		outline: 1px dashed rgb(244 241 236 / 0.45);
		outline-offset: 2px;
		cursor: move;
		touch-action: none;
		background: transparent;
	}
	.handle:hover,
	.handle.held {
		outline: 2px dashed #f4f1ec;
		box-shadow: 0 0 0 2px rgb(15 14 13 / 0.85);
		z-index: 1;
	}
	.handle:focus-visible {
		outline: 2px solid #f4f1ec;
		box-shadow: 0 0 0 4px rgb(15 14 13 / 0.9);
		z-index: 1;
	}
	.tag {
		position: absolute;
		left: 0;
		top: 0;
		translate: 0 calc(-100% - 6px);
		padding: 2px 6px;
		border-radius: var(--radius-sm);
		background: var(--color-surface);
		color: var(--color-ink);
		font-size: 12px;
		white-space: nowrap;
		opacity: 0;
		pointer-events: none;
	}
	.handle:hover .tag,
	.handle:focus-visible .tag,
	.handle.held .tag {
		opacity: 1;
	}
	.tools {
		position: absolute;
		right: 8px;
		bottom: 8px;
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 3px;
		border-radius: var(--radius);
		background: var(--color-surface);
		box-shadow: 0 0 0 1px var(--color-seam);
		cursor: default;
	}
	.tool {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 28px;
		padding: 0 10px;
		border-radius: var(--radius-sm);
		font-size: 12px;
		font-weight: 600;
		color: var(--color-ink);
		transition: background-color var(--t-fast) var(--ease);
	}
	.tool.icon {
		width: 28px;
		padding: 0;
		justify-content: center;
	}
	.tool:hover:not(:disabled) {
		background: var(--color-surface-2);
	}
	.tool:disabled {
		opacity: 0.4;
	}
	.tool.on {
		background: var(--color-ink);
		color: var(--color-ground);
	}
	.tool:focus-visible {
		outline: 2px solid var(--color-select);
		outline-offset: 1px;
	}
	.zoom {
		min-width: 40px;
		text-align: center;
		font-size: 12px;
		font-variant-numeric: tabular-nums;
		color: var(--color-ink-2);
	}
	.arrange-hint {
		position: absolute;
		left: 8px;
		bottom: 8px;
		margin: 0;
		padding: 6px 10px;
		border-radius: var(--radius);
		background: var(--color-surface);
		box-shadow: 0 0 0 1px var(--color-seam);
		font-size: 12px;
		color: var(--color-ink-2);
		pointer-events: none;
	}
</style>
