<script lang="ts">
	// Direction knob: a pointer showing the way the bands travel across the desk, with a red tip.
	// Same orientation as the preview: 0° = left to right, 90° = back to front.
	import { Spring } from 'svelte/motion';
	import { reducedMotion } from '#lib/motion.ts';

	interface Props {
		label: string;
		value: number;
		disabled?: boolean;
	}
	let { label, value = $bindable(), disabled = false }: Props = $props();

	const id = $props.id();
	const C = 64;
	const R = 56;

	let svg: SVGSVGElement;
	let dragging = $state(false);

	// The needle eases to keyboard and click changes but follows the pointer exactly while dragging.
	const shown = new Spring(0, { stiffness: 0.18, damping: 0.72 });
	let lastTarget = 0;
	$effect(() => {
		// unwrap so the needle takes the short way round 0/360
		let target = value;
		while (target - lastTarget > 180) target -= 360;
		while (target - lastTarget < -180) target += 360;
		lastTarget = target;
		shown.set(target, dragging || reducedMotion() ? { instant: true } : undefined);
	});
	const rad = $derived((shown.current * Math.PI) / 180);

	const wrap = (deg: number) => ((Math.round(deg) % 360) + 360) % 360;

	/** Plain-words direction, in desk terms: x runs left to right, y runs toward you. */
	const describe = (deg: number) => {
		const a = (deg * Math.PI) / 180;
		const x = Math.cos(a);
		const y = Math.sin(a);
		const across = x > 0.2 ? 'left to right' : x < -0.2 ? 'right to left' : '';
		const depth = y > 0.2 ? 'toward you' : y < -0.2 ? 'away from you' : '';
		return [across, depth].filter(Boolean).join(', ');
	};

	function setFromPointer(e: PointerEvent) {
		const r = svg.getBoundingClientRect();
		const dx = e.clientX - (r.left + r.width / 2);
		const dy = e.clientY - (r.top + r.height / 2);
		if (dx * dx + dy * dy < 16) return;
		let deg = (Math.atan2(dy, dx) * 180) / Math.PI;
		if (e.shiftKey) deg = Math.round(deg / 15) * 15;
		value = wrap(deg);
	}
	function onpointerdown(e: PointerEvent) {
		if (disabled || e.button !== 0) return;
		svg.setPointerCapture(e.pointerId);
		dragging = true;
		setFromPointer(e);
	}
	function onpointermove(e: PointerEvent) {
		if (dragging) setFromPointer(e);
	}
	function onpointerup() {
		dragging = false;
	}
	function onkeydown(e: KeyboardEvent) {
		if (disabled) return;
		const big = e.shiftKey ? 15 : 1;
		const step: Record<string, number> = { ArrowRight: big, ArrowUp: big, ArrowLeft: -big, ArrowDown: -big, PageUp: 15, PageDown: -15 };
		if (e.key in step) value = wrap(value + step[e.key]);
		else if (e.key === 'Home') value = 0;
		else if (e.key === 'End') value = 270;
		else return;
		e.preventDefault();
	}
</script>

<div class="dial" class:disabled>
	<div class="head">
		<span class="label" id="{id}-label">{label}</span>
		<output class="num">{value}°</output>
	</div>
	<svg
		bind:this={svg}
		viewBox="0 0 128 128"
		width="128"
		height="128"
		role="slider"
		tabindex={disabled ? -1 : 0}
		aria-labelledby="{id}-label"
		aria-valuemin={0}
		aria-valuemax={359}
		aria-valuenow={value}
		aria-valuetext="{value} degrees, {describe(value)}"
		aria-disabled={disabled}
		aria-describedby="{id}-help"
		class:dragging
		{onpointerdown}
		{onpointermove}
		{onpointerup}
		onpointercancel={onpointerup}
		{onkeydown}
	>
		<circle cx={C} cy={C} r={R} class="face" />
		<line class="needle" x1={C} y1={C} x2={C + Math.cos(rad) * (R - 14)} y2={C + Math.sin(rad) * (R - 14)} />
		<circle class="tip" cx={C + Math.cos(rad) * (R - 14)} cy={C + Math.sin(rad) * (R - 14)} r="6" />
		<circle cx={C} cy={C} r="4" class="hub" />
	</svg>
	<p class="caption">Bands travel {describe(value)}.</p>
	<p class="sr-only" id="{id}-help">Drag, or use the arrow keys. Hold Shift for 15 degree steps.</p>
</div>

<style>
	.dial {
		display: grid;
		gap: 6px;
		justify-items: start;
	}
	.disabled {
		opacity: 0.35;
	}
	.head {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		width: 100%;
	}
		output {
		font-size: 13px;
	}
	svg {
		touch-action: none;
		justify-self: center;
		cursor: grab;
	}
	svg.dragging {
		cursor: grabbing;
	}
	svg:focus-visible {
		outline-offset: 2px;
	}
	.face {
		fill: var(--color-surface-2);
		stroke: var(--color-seam-2);
		stroke-width: 1;
		transition: fill var(--t-mid) var(--ease);
	}
	svg:hover .face {
		fill: var(--color-surface-3);
	}
	.needle {
		stroke: var(--color-ink-2);
		stroke-width: 2;
		stroke-linecap: round;
	}
	.tip {
		fill: var(--color-ink);
	}
	.hub {
		fill: var(--color-ink-2);
	}
	.caption {
		color: var(--color-ink-3);
		font-size: 12px;
		line-height: 1.4;
		max-width: 22ch;
	}
</style>
