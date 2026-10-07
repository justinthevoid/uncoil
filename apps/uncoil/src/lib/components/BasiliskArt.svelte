<script lang="ts" module>
	/** uncoil's spiral (the app mark), in a 24-unit box. */
	export const SPIRAL = 'M12 12a2 2 0 1 1 2-2 4 4 0 1 1-4-4 6 6 0 1 1-6 6 8 8 0 1 1 8 8';
</script>

<script lang="ts">
	// The Basilisk V3 Pro from above, lit: its silhouette traced from the product photo, the panel seams as
	// hairlines, the grips' rubber as a fine dot texture, the wheel and uncoil's spiral (where the logo LED
	// is) in their LED colours, and the underglow strip round the sides and back. Geometry: lib/art/basilisk.ts.
	import { BODY, GRIPS, LOGO, REGION, SEAMS, STRIP_SEGMENTS, TREAD, VIEW, WELL } from '#lib/art/basilisk.ts';

	interface Props {
		/** Colour per LED shape name ("Scroll Wheel", "Logo", "Strip 1" … "Strip 11"). */
		color: (shape: string) => string | undefined;
	}
	let { color }: Props = $props();
	const uid = $props.id();
	const off = 'var(--led-off)';
	const S = 1.7;
</script>

<svg viewBox="{VIEW.x} {VIEW.y} {VIEW.w} {VIEW.h}" aria-hidden="true">
	<defs>
		<clipPath id="{uid}-body"><path d={BODY} /></clipPath>
		<pattern id="{uid}-grip" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
			<circle cx="2" cy="2" r="0.8" class="dot" />
		</pattern>
	</defs>
	<!-- underglow: a band just outside the shell, lit LED by LED -->
	{#each STRIP_SEGMENTS as s (s.led)}
		<path class="strip" d={s.d} style:stroke={color(`Strip ${s.led + 1}`) ?? off} />
	{/each}
	<path class="shell" d={BODY} />
	<g clip-path="url(#{uid}-body)">
		<path class="button" d={REGION.LEFT_CLICK} />
		<path class="button" d={REGION.RIGHT_CLICK} />
		{#each GRIPS as g (g)}<path class="grip" d={g} fill="url(#{uid}-grip)" />{/each}
		{#each SEAMS as d (d)}<path class="seam" {d} />{/each}
	</g>
	<path class="well" d={WELL} />
	<path class="wheel" d={REGION.WHEEL_CLICK} style:fill={color('Scroll Wheel') ?? off} />
	<path class="tread" d={TREAD} />
	<path class="key" d={REGION.SCROLL_MODE} />
	<path class="key" d={REGION.DPI_BUTTON} />
	<path class="key" d={REGION.FORWARD} />
	<path class="key" d={REGION.BACK} />
	<path class="key" d={REGION.CLUTCH} />
	<path class="logo" d={SPIRAL} transform="translate({LOGO[0] - 12 * S} {LOGO[1] - 12 * S}) scale({S})" style:stroke={color('Logo') ?? off} />
</svg>

<style>
	svg {
		display: block;
		width: 100%;
		height: 100%;
		overflow: visible;
	}
	.strip {
		fill: none;
		stroke-width: 9;
		stroke-linecap: round;
		transition: stroke 120ms linear;
	}
	.shell {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1.4;
	}
	.button {
		fill: var(--shell-top);
	}
	.grip {
		opacity: 0.9;
	}
	.dot {
		fill: var(--seam-line);
	}
	.seam,
	.tread {
		fill: none;
		stroke: var(--seam-line);
		stroke-width: 1.2;
		stroke-linecap: round;
	}
	.tread {
		stroke-width: 1.4;
	}
	.well {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1.2;
	}
	.key {
		fill: var(--shell-top);
		stroke: var(--seam-line);
		stroke-width: 1.2;
	}
	.wheel {
		transition: fill 120ms linear;
	}
	.logo {
		fill: none;
		stroke-width: 1.7;
		stroke-linecap: round;
		transition: stroke 120ms linear;
	}
</style>
