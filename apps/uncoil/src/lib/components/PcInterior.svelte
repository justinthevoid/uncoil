<script lang="ts">
	// The inside of a mid-tower PC seen through its glass side, drawn to ATX proportions: mesh front and top, rear
	// I/O and slot covers, the motherboard (VRM heatsink, I/O shroud, CPU, four DIMM slots, 24-pin, M.2 shields,
	// PCIe slots, chipset), the graphics card across it (inside the case, short of the front fans), an AIO pump
	// with its hoses up to a top radiator, fans at the top, front, rear and floor, and the PSU shroud with the PSU.
	// Each lit part OpenRGB reports (lib/pc.ts) is drawn where it sits, its LEDs in their live colours; fans and
	// sticks it doesn't report are left as empty mounts, and anything it can't place runs along the shroud.
	// Without `onselect` it is only a picture (the desk preview): nothing in it is focusable or announced.
	import { FAN_SLOTS, type Part } from '#lib/pc.ts';

	interface Props {
		parts: Part[];
		/** Colour of LED `i` of an OpenRGB device. */
		color: (device: string, i: number) => string | undefined;
		selected?: string | null;
		onselect?: (device: string) => void;
	}
	let { parts, color, selected = null, onselect }: Props = $props();
	const uid = $props.id();
	const off = 'var(--led-off)';
	const c = (p: Part, k: number) => color(p.device, p.leds[k]) ?? off;

	const of = (k: string) => parts.filter((p) => p.kind === k);
	const ram = $derived(of('ram').slice(0, 4));
	const gpu = $derived(of('gpu'));
	const board = $derived(of('board'));
	const pumps = $derived(of('pump'));
	const radiators = $derived(of('radiator'));
	const fans = $derived(of('fan'));
	const other = $derived([...of('other'), ...of('ram').slice(4), ...pumps.slice(1), ...fans.slice(FAN_SLOTS.length)]);

	// fan centres by slot (112 mm fans, front of the case on the right)
	const F = 112;
	const SLOT_AT: Record<string, [number, number]> = {
		'top 1': [222, 92], 'top 2': [342, 92], 'top 3': [462, 92],
		'front 1': [508, 232], 'front 2': [508, 352], 'front 3': [508, 472],
		rear: [92, 92],
		'floor 1': [262, 532], 'floor 2': [382, 532]
	};
	// The floor fans sit in front of the shroud, so they are drawn after it.
	const UPPER = FAN_SLOTS.filter((s) => !s.startsWith('floor'));
	const FLOOR = FAN_SLOTS.filter((s) => s.startsWith('floor'));
	const placedFans = $derived(fans.slice(0, UPPER.length).map((p, i) => ({ p, at: SLOT_AT[UPPER[i]] })));
	const floorFans = $derived(fans.slice(UPPER.length, FAN_SLOTS.length).map((p, i) => ({ p, at: SLOT_AT[FLOOR[i]] })));
	const emptyFans = $derived(UPPER.slice(placedFans.length).map((s) => SLOT_AT[s]));

	const P = (cx: number, cy: number, r: number, a: number) => `${(cx + r * Math.cos(a)).toFixed(1)} ${(cy + r * Math.sin(a)).toFixed(1)}`;
	/** A ring of n arcs round (cx, cy), one per LED. */
	function ring(cx: number, cy: number, r: number, n: number): string[] {
		const gap = n > 1 ? Math.min(0.08, (Math.PI * 2) / n / 4) : 0;
		return Array.from({ length: n }, (_, i) => {
			const a0 = -Math.PI / 2 + (i / n) * Math.PI * 2 + gap / 2;
			const a1 = -Math.PI / 2 + ((i + 1) / n) * Math.PI * 2 - gap / 2;
			return `M${P(cx, cy, r, a0)} A${r} ${r} 0 0 1 ${P(cx, cy, r, a1)}`;
		});
	}
	/** Seven swept blades between the hub and the frame. */
	function blades(cx: number, cy: number): string {
		const r0 = 15, r1 = 46;
		return Array.from({ length: 7 }, (_, i) => {
			const a = (i / 7) * Math.PI * 2;
			const w = 0.42;
			return `M${P(cx, cy, r0, a)} Q${P(cx, cy, (r0 + r1) / 2 + 6, a + 0.35)} ${P(cx, cy, r1, a + 0.75)} L${P(cx, cy, r1, a + 0.75 + w)} Q${P(cx, cy, (r0 + r1) / 2 - 2, a + 0.6 + w)} ${P(cx, cy, r0, a + w)} Z`;
		}).join(' ');
	}
	/** A straight run from (x0, y0) to (x1, y1) split into n segments, one per LED. */
	function run(x0: number, y0: number, x1: number, y1: number, n: number): string[] {
		return Array.from({ length: n }, (_, i) => {
			const t0 = (i + 0.1) / n, t1 = (i + 0.9) / n;
			return `M${(x0 + (x1 - x0) * t0).toFixed(1)} ${(y0 + (y1 - y0) * t0).toFixed(1)} L${(x0 + (x1 - x0) * t1).toFixed(1)} ${(y0 + (y1 - y0) * t1).toFixed(1)}`;
		});
	}
	/** Several parts' LEDs one after another along a run. */
	function runOf(list: Part[], x0: number, y0: number, x1: number, y1: number) {
		const all = list.flatMap((p) => p.leds.map((_, k) => ({ p, k })));
		return run(x0, y0, x1, y1, all.length).map((d, i) => ({ d, p: all[i].p, k: all[i].k }));
	}
	const boardLeds = $derived(board.flatMap((p) => p.leds.map((_, k) => ({ p, k }))));
	const boardHalf = $derived(Math.ceil(boardLeds.length / 2));
	const shroudRun = $derived(run(84, 192, 84, 282, boardHalf));
	const chipRun = $derived(run(232, 386, 300, 386, boardLeds.length - boardHalf));
	/** A part's button attributes, or none when the drawing is only a picture. */
	const act = (p: Part, label: string) =>
		onselect
			? {
					role: 'button',
					tabindex: 0,
					'aria-label': label,
					onclick: () => onselect(p.device),
					onkeydown: (e: KeyboardEvent) => {
						if (e.key === 'Enter' || e.key === ' ') {
							e.preventDefault();
							onselect(p.device);
						}
					}
				}
			: {};
	const svgAttrs = $derived(onselect ? { role: 'group', 'aria-label': 'Inside the PC' } : { 'aria-hidden': 'true' as const });
	const sel = (p: Part) => selected === p.device;
	// DIMM slots, left to right
	const DIMM = [252, 266, 280, 294];
</script>

<!-- selection (dashed) and keyboard focus (solid) round a part's whole area, light over dark so either reads -->
{#snippet outline(x: number, y: number, w: number, h: number)}
	<rect class="sel-under" {x} {y} width={w} height={h} rx="6" />
	<rect class="sel-over" {x} {y} width={w} height={h} rx="6" />
	<rect class="focus-under" x={x - 3} y={y - 3} width={w + 6} height={h + 6} rx="8" />
	<rect class="focus-over" x={x - 3} y={y - 3} width={w + 6} height={h + 6} rx="8" />
{/snippet}

{#snippet fan(x: number, y: number)}
	<rect class="frame" x={x - F / 2} y={y - F / 2} width={F} height={F} rx="10" />
	{#each [[-1, -1], [1, -1], [-1, 1], [1, 1]] as [sx, sy] (sx * 2 + sy)}
		<circle class="screw" cx={x + sx * (F / 2 - 8)} cy={y + sy * (F / 2 - 8)} r="3" />
	{/each}
	<circle class="throat" cx={x} cy={y} r="50" />
	<path class="blade" d={blades(x, y)} />
	<circle class="hub" cx={x} cy={y} r="14" />
{/snippet}

{#snippet litFan(p: Part, x: number, y: number)}
	<g class="part" class:sel={sel(p)} {...act(p, `Fan: ${p.zone}, ${p.deviceName}`)}>
		<title>Fan: {p.deviceName}</title>
		{@render fan(x, y)}
		{#each ring(x, y, 50, p.leds.length) as d, k (k)}<path class="led" {d} style:stroke={c(p, k)} />{/each}
		{@render outline(x - F / 2 - 4, y - F / 2 - 4, F + 8, F + 8)}
	</g>
{/snippet}

<svg viewBox="0 0 580 620" {...svgAttrs}>
	<defs>
		<pattern id="{uid}-mesh" width="5" height="5" patternUnits="userSpaceOnUse">
			<circle cx="2.5" cy="2.5" r="1.3" class="hole" />
		</pattern>
		<pattern id="{uid}-fins" width="4" height="10" patternUnits="userSpaceOnUse">
			<rect width="1.4" height="10" class="fin" />
		</pattern>
		<pattern id="{uid}-vfins" width="10" height="4" patternUnits="userSpaceOnUse">
			<rect width="10" height="1.4" class="fin" />
		</pattern>
		<clipPath id="{uid}-inside"><rect x="22" y="22" width="536" height="576" rx="10" /></clipPath>
	</defs>

	<!-- the case: frame, mesh front and top, rear wall with its I/O cut-out and slot covers -->
	<rect class="case" x="6" y="6" width="568" height="608" rx="18" />
	<rect class="back" x="22" y="22" width="536" height="576" rx="10" />
	<rect class="mesh" x="562" y="30" width="8" height="560" rx="3" fill="url(#{uid}-mesh)" />
	<rect class="mesh" x="40" y="10" width="500" height="8" rx="3" fill="url(#{uid}-mesh)" />
	<rect class="shell" x="26" y="168" width="22" height="118" rx="3" />
	{#each [0, 1, 2, 3, 4, 5, 6] as i (i)}<rect class="slot-cover" x="26" y={302 + i * 17} width="22" height="12" rx="2" />{/each}

	<g clip-path="url(#{uid}-inside)">
		<!-- cable grommets beside the board -->
		{#each [196, 266, 336] as y (y)}<rect class="grommet" x="334" {y} width="12" height="52" rx="6" />{/each}

		<!-- motherboard -->
		<rect class="pcb" x="60" y="168" width="244" height="268" rx="5" />
		<rect class="shell" x="64" y="172" width="40" height="114" rx="5" />
		<rect class="heatsink" x="108" y="172" width="116" height="24" rx="4" />
		<rect class="heatsink" x="108" y="172" width="116" height="24" rx="4" fill="url(#{uid}-fins)" />
		<rect class="socket" x="128" y="212" width="74" height="74" rx="6" />
		{#each DIMM as x (x)}<rect class="dimm" {x} y="186" width="9" height="122" rx="2" />{/each}
		<rect class="shell" x="306" y="214" width="8" height="58" rx="2" />
		<rect class="shell" x="112" y="364" width="96" height="16" rx="3" />
		<rect class="shell" x="112" y="404" width="96" height="16" rx="3" />
		{#each [354, 392, 426] as y (y)}<line class="seam" x1="90" y1={y} x2="226" y2={y} />{/each}
		<rect class="heatsink" x="228" y="378" width="76" height="52" rx="6" />
		<rect class="heatsink" x="228" y="378" width="76" height="52" rx="6" fill="url(#{uid}-vfins)" />

		<!-- memory: empty slots stay dark; each stick its heat spreader and light bar -->
		{#each ram as p, i (p.device + p.leds[0])}
			{@const x = DIMM[i]}
			<g class="part" class:sel={sel(p)} {...act(p, `Memory: ${p.deviceName}`)}>
				<title>Memory: {p.deviceName}</title>
				<rect class="stick" x={x - 1} y="184" width="11" height="126" rx="2" />
				{#each run(x + 4.5, 190, x + 4.5, 250, p.leds.length) as d, k (k)}<path class="led thin" {d} style:stroke={c(p, k)} />{/each}
				{@render outline(x - 4, 181, 17, 132)}
			</g>
		{/each}

		<!-- the motherboard's own LEDs: down the I/O shroud, then along the chipset -->
		{#if board.length}
			<g class="part" class:sel={board.some(sel)} {...act(board[0], `Motherboard: ${board[0].deviceName}`)}>
				<title>Motherboard: {board[0].deviceName}</title>
				{#each boardLeds.slice(0, boardHalf) as { p, k }, i (i)}<path class="led" d={shroudRun[i]} style:stroke={c(p, k)} />{/each}
				{#each boardLeds.slice(boardHalf) as { p, k }, i (i)}<path class="led thin" d={chipRun[i]} style:stroke={c(p, k)} />{/each}
				{@render outline(56, 164, 252, 276)}
			</g>
		{/if}

		<!-- the AIO: hoses up to the radiator, the pump on the CPU with its LCD -->
		{#if pumps.length || radiators.length}
			<path class="hose" d="M150 218 C140 180 168 150 200 150" />
			<path class="hose" d="M178 214 C176 186 210 158 250 152" />
		{/if}
		{#if radiators.length || pumps.length}
			<rect class="radiator" x="166" y="142" width="352" height="16" rx="3" />
			<rect class="radiator" x="166" y="142" width="352" height="16" rx="3" fill="url(#{uid}-fins)" />
		{/if}
		{#if radiators.length}
			<g class="part" class:sel={radiators.some(sel)} {...act(radiators[0], `Radiator: ${radiators[0].deviceName}`)}>
				<title>Radiator: {radiators[0].deviceName}</title>
				{#each runOf(radiators, 176, 162, 508, 162) as s, i (i)}<path class="led thin" d={s.d} style:stroke={c(s.p, s.k)} />{/each}
				{@render outline(162, 138, 360, 30)}
			</g>
		{/if}
		{#each pumps.slice(0, 1) as p (p.device + p.leds[0])}
			<g class="part" class:sel={sel(p)} {...act(p, `Cooler pump: ${p.deviceName}`)}>
				<title>Cooler pump: {p.deviceName}</title>
				<rect class="pump" x="130" y="214" width="70" height="70" rx="16" />
				<circle class="lcd" cx="165" cy="249" r="20" />
				{#each ring(165, 249, 28, p.leds.length) as d, k (k)}<path class="led" {d} style:stroke={c(p, k)} />{/each}
				{@render outline(126, 210, 78, 78)}
			</g>
		{/each}

		<!-- the graphics card: 310 mm against the board's 244, reaching past it but short of the front fans -->
		{#snippet card()}
			<rect class="bracket" x="40" y="318" width="20" height="58" rx="2" />
			<rect class="card" x="58" y="318" width="310" height="58" rx="7" />
			<rect class="shroud-line" x="66" y="336" width="294" height="32" rx="5" />
			<rect class="shell" x="330" y="312" width="26" height="8" rx="2" />
		{/snippet}
		{#if gpu.length}
			<g class="part" class:sel={gpu.some(sel)} {...act(gpu[0], `Graphics card: ${gpu[0].deviceName}`)}>
				<title>Graphics card: {gpu[0].deviceName}</title>
				{@render card()}
				{#each runOf(gpu, 76, 326, 350, 326) as s, i (i)}<path class="led" d={s.d} style:stroke={c(s.p, s.k)} />{/each}
				{@render outline(36, 308, 336, 72)}
			</g>
		{:else}
			<g class="dim">{@render card()}</g>
		{/if}

		<!-- fans: empty mounts, then the lit ones -->
		{#each emptyFans as [x, y] (`${x},${y}`)}<rect class="mount" x={x - F / 2} y={y - F / 2} width={F} height={F} rx="10" />{/each}
		{#each placedFans as { p, at: [x, y] } (p.device + p.leds[0])}
			{@render litFan(p, x, y)}
		{/each}

		<!-- the PSU shroud, the PSU's vented end showing at the back, anything else along its top edge -->
		<rect class="shroud" x="22" y="470" width="430" height="128" rx="4" />
		<rect class="psu" x="34" y="490" width="168" height="94" rx="6" />
		<rect class="psu" x="44" y="500" width="86" height="74" rx="4" fill="url(#{uid}-mesh)" />
		{#each floorFans as { p, at: [x, y] } (p.device + p.leds[0])}
			{@render litFan(p, x, y)}
		{/each}
		{#if other.length}
			<g class="part" class:sel={other.some(sel)} {...act(other[0], 'Other lighting')}>
				<title>Other lighting</title>
				{#each runOf(other, 34, 476, 440, 476) as s, i (i)}<path class="led thin" d={s.d} style:stroke={c(s.p, s.k)} />{/each}
				{@render outline(28, 468, 418, 16)}
			</g>
		{/if}
	</g>
	<!-- the glass: a hairline inside the frame -->
	<rect class="glass" x="22" y="22" width="536" height="576" rx="10" />
</svg>

<style>
	svg {
		display: block;
		width: 100%;
		height: 100%;
		overflow: visible;
	}
	rect,
	circle,
	path,
	line {
		vector-effect: non-scaling-stroke;
	}
	.case {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1.4;
	}
	.back {
		fill: var(--cloth);
	}
	.glass {
		fill: none;
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.mesh {
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.hole {
		fill: var(--plate);
	}
	.fin {
		fill: var(--seam-line);
	}
	.shell,
	.slot-cover,
	.stick,
	.psu {
		fill: var(--shell-top);
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.grommet {
		fill: var(--plate);
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.pcb {
		fill: var(--plate);
		stroke: var(--seam-line);
		stroke-width: 1.2;
	}
	.heatsink {
		fill: var(--shell-top);
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.socket {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.dimm {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.seam {
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.hose {
		fill: none;
		stroke: var(--case);
		stroke-width: 9;
		stroke-linecap: round;
	}
	.radiator {
		fill: var(--shell-top);
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.pump {
		fill: var(--shell-top);
		stroke: var(--seam-line);
		stroke-width: 1.2;
	}
	.lcd {
		fill: #0c0b0a;
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.card {
		fill: var(--shell-top);
		stroke: var(--seam-line);
		stroke-width: 1.2;
	}
	.bracket {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.shroud-line {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.dim {
		opacity: 0.55;
	}
	.mount {
		fill: none;
		stroke: var(--seam-line);
		stroke-width: 1;
		stroke-dasharray: 4 4;
	}
	.frame {
		fill: var(--shell-top);
		stroke: var(--seam-line);
		stroke-width: 1.2;
	}
	.screw {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 0.8;
	}
	.throat {
		fill: var(--case);
	}
	.blade {
		fill: var(--plate);
		stroke: var(--seam-line);
		stroke-width: 0.8;
	}
	.hub {
		fill: var(--shell-top);
		stroke: var(--seam-line);
		stroke-width: 1;
	}
	.shroud {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1.2;
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
	.part[role='button'] {
		cursor: pointer;
		outline: none;
	}
	.part[role='button']:hover .frame,
	.part[role='button']:hover .card,
	.part[role='button']:hover .stick,
	.part[role='button']:hover .pump {
		stroke: var(--color-ink-3);
	}
	/* the drawing wears the dark device finish, but the rings are two-tone so they read on any ground */
	.sel-under,
	.sel-over,
	.focus-under,
	.focus-over {
		display: none;
		fill: none;
		pointer-events: none;
	}
	.sel-under {
		stroke: #0c0b0a;
		stroke-width: 4;
	}
	.sel-over {
		stroke: #f4f1ec;
		stroke-width: 2;
		stroke-dasharray: 6 4;
	}
	.focus-under {
		stroke: #0c0b0a;
		stroke-width: 5;
	}
	.focus-over {
		stroke: #f4f1ec;
		stroke-width: 2.5;
	}
	.part.sel .sel-under,
	.part.sel .sel-over,
	.part[role='button']:focus-visible .focus-under,
	.part[role='button']:focus-visible .focus-over {
		display: inline;
	}
</style>
