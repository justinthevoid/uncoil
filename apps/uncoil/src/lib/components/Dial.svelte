<script lang="ts">
	// Rotary direction control. The needle points the way the colour bands travel across the desk,
	// in the same screen orientation as the preview (0° = left to right, 90° = back to front).
	interface Props {
		label: string;
		value: number;
		disabled?: boolean;
	}
	let { label, value = $bindable(), disabled = false }: Props = $props();

	const id = $props.id();
	const R = 54;
	const C = 66;
	const ticks = Array.from({ length: 24 }, (_, i) => i * 15);

	let svg: SVGSVGElement;
	let dragging = $state(false);

	const wrap = (deg: number) => ((Math.round(deg) % 360) + 360) % 360;
	const rad = $derived((value * Math.PI) / 180);

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
		const step: Record<string, number> = {
			ArrowRight: big,
			ArrowUp: big,
			ArrowLeft: -big,
			ArrowDown: -big,
			PageUp: 15,
			PageDown: -15
		};
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
		viewBox="0 0 132 132"
		width="132"
		height="132"
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
		<circle cx={C} cy={C} r={R + 6} class="face" />
		{#each ticks as t (t)}
			{@const a = (t * Math.PI) / 180}
			{@const major = t % 90 === 0}
			<line
				x1={C + Math.cos(a) * (R - (major ? 8 : 4))}
				y1={C + Math.sin(a) * (R - (major ? 8 : 4))}
				x2={C + Math.cos(a) * R}
				y2={C + Math.sin(a) * R}
				class:major
			/>
		{/each}
		<line
			class="needle"
			x1={C - Math.cos(rad) * 14}
			y1={C - Math.sin(rad) * 14}
			x2={C + Math.cos(rad) * (R - 12)}
			y2={C + Math.sin(rad) * (R - 12)}
		/>
		<polygon
			class="tip"
			points="{C + Math.cos(rad) * (R - 4)},{C + Math.sin(rad) * (R - 4)} {C +
				Math.cos(rad + 2.6) * 9 +
				Math.cos(rad) * (R - 12)},{C + Math.sin(rad + 2.6) * 9 + Math.sin(rad) * (R - 12)} {C +
				Math.cos(rad - 2.6) * 9 +
				Math.cos(rad) * (R - 12)},{C + Math.sin(rad - 2.6) * 9 + Math.sin(rad) * (R - 12)}"
		/>
		<circle cx={C} cy={C} r="4" class="hub" />
	</svg>
	<p class="caption">Bands move {describe(value)}.</p>
	<p class="sr-only" id="{id}-help">Drag, or use the arrow keys. Hold Shift for 15 degree steps.</p>
</div>

<style>
	.dial {
		display: grid;
		gap: 8px;
		justify-items: start;
	}
	.disabled {
		opacity: 0.4;
	}
	.head {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		width: 100%;
	}
	.label {
		font-size: 13px;
	}
	output {
		color: var(--color-dim);
		font-size: 13px;
	}
	svg {
		touch-action: none;
		border-radius: 50%;
		justify-self: center;
	}
	svg:focus-visible {
		outline-offset: 0;
		border-radius: 50%;
	}
	.face {
		fill: var(--color-well);
		stroke: var(--color-line);
	}
	line {
		stroke: var(--color-ink-3);
		stroke-width: 1.5;
		stroke-linecap: round;
	}
	line.major {
		stroke: var(--color-faint);
	}
	.needle {
		stroke: var(--color-brass);
		stroke-width: 2.5;
	}
	.tip {
		fill: var(--color-brass);
	}
	.hub {
		fill: var(--color-ink-1);
		stroke: var(--color-brass);
		stroke-width: 2;
	}
	.caption {
		color: var(--color-faint);
		font-size: 12px;
		line-height: 1.4;
		max-width: 26ch;
	}
</style>
