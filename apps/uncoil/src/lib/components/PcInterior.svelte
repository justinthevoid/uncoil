<script lang="ts">
	// The inside of a PC seen through its side panel, as a schematic: the motherboard with its I/O shroud, CPU and
	// four DIMM slots, the graphics card across it, fans at the top, front, rear and floor, and the PSU shroud.
	// Each lit part OpenRGB reports (lib/pc.ts) is drawn where it sits, its LEDs in their live colours; parts it
	// doesn't report are not drawn, and anything it can't place runs along the shroud. A click picks a device.
	import { FAN_SLOTS, type Part } from '#lib/pc.ts';

	interface Props {
		parts: Part[];
		/** Colour of LED `i` of an OpenRGB device. */
		color: (device: string, i: number) => string | undefined;
		selected?: string | null;
		onselect?: (device: string) => void;
	}
	let { parts, color, selected = null, onselect }: Props = $props();
	const off = 'var(--led-off)';
	const c = (p: Part, k: number) => color(p.device, p.leds[k]) ?? off;

	const of = (k: string) => parts.filter((p) => p.kind === k);
	const ram = $derived(of('ram').slice(0, 4));
	const gpu = $derived(of('gpu'));
	const board = $derived(of('board'));
	const pumps = $derived(of('pump'));
	const radiators = $derived(of('radiator'));
	const fans = $derived(of('fan'));
	const other = $derived([...of('other'), ...of('ram').slice(4), ...fans.slice(FAN_SLOTS.length)]);

	// fan positions by slot, in the drawing's units (front of the case on the right)
	const SLOT_AT: Record<string, [number, number]> = {
		'top 1': [158, 62], 'top 2': [250, 62], 'top 3': [342, 62],
		'front 1': [436, 168], 'front 2': [436, 262], 'front 3': [436, 356],
		rear: [62, 168],
		'floor 1': [170, 490], 'floor 2': [262, 490], 'floor 3': [354, 490]
	};
	const placedFans = $derived(fans.slice(0, FAN_SLOTS.length).map((p, i) => ({ p, at: SLOT_AT[FAN_SLOTS[i]] })));

	/** A ring of n arcs round (cx, cy), one per LED. */
	function ring(cx: number, cy: number, r: number, n: number): string[] {
		const gap = n > 1 ? Math.min(0.08, (Math.PI * 2) / n / 4) : 0;
		return Array.from({ length: n }, (_, i) => {
			const a0 = -Math.PI / 2 + (i / n) * Math.PI * 2 + gap / 2;
			const a1 = -Math.PI / 2 + ((i + 1) / n) * Math.PI * 2 - gap / 2;
			const p = (a: number) => `${(cx + r * Math.cos(a)).toFixed(1)} ${(cy + r * Math.sin(a)).toFixed(1)}`;
			return `M${p(a0)} A${r} ${r} 0 0 1 ${p(a1)}`;
		});
	}
	/** A straight run from (x0, y0) to (x1, y1) split into n segments, one per LED. */
	function run(x0: number, y0: number, x1: number, y1: number, n: number): string[] {
		return Array.from({ length: n }, (_, i) => {
			const t0 = (i + 0.08) / n, t1 = (i + 0.92) / n;
			return `M${(x0 + (x1 - x0) * t0).toFixed(1)} ${(y0 + (y1 - y0) * t0).toFixed(1)} L${(x0 + (x1 - x0) * t1).toFixed(1)} ${(y0 + (y1 - y0) * t1).toFixed(1)}`;
		});
	}
	/** Several parts' LEDs one after another along a run. */
	function runOf(list: Part[], x0: number, y0: number, x1: number, y1: number) {
		const all = list.flatMap((p) => p.leds.map((_, k) => ({ p, k })));
		return run(x0, y0, x1, y1, all.length).map((d, i) => ({ d, p: all[i].p, k: all[i].k }));
	}
	const boardHalf = $derived(Math.ceil(board.reduce((n, p) => n + p.leds.length, 0) / 2));
	const boardLeds = $derived(board.flatMap((p) => p.leds.map((_, k) => ({ p, k }))));
	const pick = (p: Part) => () => onselect?.(p.device);
	const key = (p: Part) => (e: KeyboardEvent) => {
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			onselect?.(p.device);
		}
	};
	const sel = (p: Part) => selected === p.device;
</script>

{#snippet hit(p: Part, label: string)}
	<title>{label}: {p.deviceName}</title>
{/snippet}

<svg viewBox="0 0 500 560" role="group" aria-label="Inside the PC">
	<!-- case, glass and the parts that are always there -->
	<rect class="case" x="10" y="10" width="480" height="540" rx="16" />
	<rect class="inside" x="22" y="22" width="456" height="516" rx="10" />
	<rect class="board" x="110" y="116" width="240" height="300" rx="6" />
	<rect class="shell" x="114" y="120" width="40" height="110" rx="4" />
	<circle class="shell" cx="210" cy="176" r="28" />
	{#each [268, 281, 294, 307] as x (x)}<rect class="slot" {x} y="126" width="9" height="126" rx="2" />{/each}
	<rect class="shell" x="276" y="366" width="60" height="40" rx="5" />
	{#each [318, 344, 392] as y (y)}<line class="seam" x1="160" y1={y} x2="330" y2={y} />{/each}
	<rect class="shell" x="22" y="440" width="456" height="98" rx="8" />

	<!-- memory: each stick in its slot, its light bar down its length -->
	{#each ram as p, i (p.device + p.leds[0])}
		{@const x = 268 + i * 13}
		<g class="part" class:sel={sel(p)} role="button" tabindex="0" aria-label="Memory: {p.deviceName}" onclick={pick(p)} onkeydown={key(p)}>
			{@render hit(p, 'Memory')}
			<rect class="stick" {x} y="126" width="9" height="126" rx="2" />
			{#each run(x + 4.5, 130, x + 4.5, 200, p.leds.length) as d, k (k)}<path class="led thin" {d} style:stroke={c(p, k)} />{/each}
		</g>
	{/each}

	<!-- the motherboard's own LEDs: along the I/O shroud, then the chipset -->
	{#if board.length}
		<g class="part" class:sel={board.some(sel)} role="button" tabindex="0" aria-label="Motherboard: {board[0].deviceName}" onclick={pick(board[0])} onkeydown={key(board[0])}>
			{@render hit(board[0], 'Motherboard')}
			{#each boardLeds.slice(0, boardHalf) as { p, k }, i (i)}
				<path class="led" d={run(158, 124, 158, 226, boardHalf)[i]} style:stroke={c(p, k)} />
			{/each}
			{#each boardLeds.slice(boardHalf) as { p, k }, i (i)}
				<path class="led" d={run(282, 362, 330, 362, boardLeds.length - boardHalf)[i]} style:stroke={c(p, k)} />
			{/each}
		</g>
	{/if}

	<!-- the graphics card across the board, its light bar along the top edge -->
	{#if gpu.length}
		<g class="part" class:sel={gpu.some(sel)} role="button" tabindex="0" aria-label="Graphics card: {gpu[0].deviceName}" onclick={pick(gpu[0])} onkeydown={key(gpu[0])}>
			{@render hit(gpu[0], 'Graphics card')}
			<rect class="card" x="110" y="270" width="300" height="62" rx="6" />
			{#each runOf(gpu, 122, 276, 398, 276) as s, i (i)}<path class="led" d={s.d} style:stroke={c(s.p, s.k)} />{/each}
		</g>
	{:else}
		<rect class="card dim" x="110" y="270" width="300" height="62" rx="6" />
	{/if}

	<!-- an AIO pump on the CPU: a ring round it -->
	{#each pumps.slice(0, 1) as p (p.device + p.leds[0])}
		<g class="part" class:sel={sel(p)} role="button" tabindex="0" aria-label="Cooler pump: {p.deviceName}" onclick={pick(p)} onkeydown={key(p)}>
			{@render hit(p, 'Cooler pump')}
			{#each ring(210, 176, 33, p.leds.length) as d, k (k)}<path class="led" {d} style:stroke={c(p, k)} />{/each}
		</g>
	{/each}

	<!-- a radiator's own lighting: under the top fans -->
	{#if radiators.length}
		<g class="part" class:sel={radiators.some(sel)} role="button" tabindex="0" aria-label="Radiator: {radiators[0].deviceName}" onclick={pick(radiators[0])} onkeydown={key(radiators[0])}>
			{@render hit(radiators[0], 'Radiator')}
			{#each runOf(radiators, 120, 104, 380, 104) as s, i (i)}<path class="led" d={s.d} style:stroke={c(s.p, s.k)} />{/each}
		</g>
	{/if}

	<!-- fans: frame, hub and the lit ring -->
	{#each placedFans as { p, at: [x, y] } (p.device + p.leds[0])}
		<g class="part" class:sel={sel(p)} role="button" tabindex="0" aria-label="Fan: {p.zone}, {p.deviceName}" onclick={pick(p)} onkeydown={key(p)}>
			{@render hit(p, 'Fan')}
			<rect class="frame" x={x - 38} y={y - 38} width="76" height="76" rx="8" />
			<circle class="hub" cx={x} cy={y} r="11" />
			{#each ring(x, y, 31, p.leds.length) as d, k (k)}<path class="led" {d} style:stroke={c(p, k)} />{/each}
		</g>
	{/each}

	<!-- everything else along the PSU shroud -->
	{#if other.length}
		<g class="part" class:sel={other.some(sel)} role="button" tabindex="0" aria-label="Other lighting" onclick={pick(other[0])} onkeydown={key(other[0])}>
			{@render hit(other[0], 'Other lighting')}
			{#each runOf(other, 40, 446, 460, 446) as s, i (i)}<path class="led" d={s.d} style:stroke={c(s.p, s.k)} />{/each}
		</g>
	{/if}
</svg>

<style>
	svg {
		display: block;
		width: 100%;
		height: 100%;
		overflow: visible;
	}
	.case {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1.4;
	}
	.inside {
		fill: var(--cloth);
	}
	.board {
		fill: var(--plate);
		stroke: var(--seam-line);
		stroke-width: 1.2;
	}
	.shell,
	.card,
	.frame,
	.stick {
		fill: var(--shell-top);
		stroke: var(--seam-line);
		stroke-width: 1.2;
	}
	.card.dim {
		opacity: 0.5;
	}
	.slot {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.hub {
		fill: var(--case);
		stroke: var(--seam-line);
	}
	.seam {
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.led {
		fill: none;
		stroke-width: 6;
		stroke-linecap: round;
		transition: stroke 120ms linear;
	}
	.led.thin {
		stroke-width: 4;
	}
	.part {
		cursor: pointer;
		outline: none;
	}
	.part:hover .frame,
	.part:hover .card,
	.part:hover .stick {
		stroke: var(--color-ink-3);
	}
	.part.sel .frame,
	.part.sel .card,
	.part.sel .stick {
		stroke: #f4f1ec;
		stroke-width: 2;
		stroke-dasharray: 5 3;
	}
	.part:focus-visible .led {
		stroke-width: 8;
	}
</style>
