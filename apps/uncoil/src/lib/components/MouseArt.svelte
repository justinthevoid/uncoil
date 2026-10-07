<script lang="ts">
	// A traced mouse from above, lit (lib/art/mice.ts): its silhouette from the product photo, the panel seams
	// as hairlines, grips as a fine dot texture, the wheel and uncoil's spiral (at the logo LED) in their LED
	// colours, and the rest of its LEDs as a strip round the sides and back.
	import { ledRoles, SPIRAL, stripParts, type MouseArt } from '#lib/art/mice.ts';

	interface Props {
		art: MouseArt;
		/** The device file's LED names, in order. */
		shapes: string[];
		/** Colour per LED name. */
		color: (shape: string) => string | undefined;
	}
	let { art, shapes, color }: Props = $props();
	const uid = $props.id();
	const off = 'var(--led-off)';
	const roles = $derived(ledRoles(shapes));
	const parts = $derived(stripParts(art, roles.strip.length));
	// sizes scale with the photo: the Basilisk V3 Pro's body is 496 px long
	const S = $derived(art.body_box.h / 496);
	const side = $derived(['FORWARD', 'BACK', 'CLUTCH', 'SCROLL_MODE', 'DPI_BUTTON'].filter((k) => art.region[k]));
</script>

<svg viewBox="{art.view.x} {art.view.y} {art.view.w} {art.view.h}" aria-hidden="true">
	<defs>
		<clipPath id="{uid}-body"><path d={art.body} /></clipPath>
		<pattern id="{uid}-grip" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
			<circle cx="2" cy="2" r="0.8" class="dot" />
		</pattern>
	</defs>
	{#each parts as d, i (i)}
		<path class="strip" {d} style:stroke-width={9 * S} style:stroke={color(roles.strip[i]) ?? off} />
	{/each}
	<path class="shell" d={art.body} />
	<g clip-path="url(#{uid}-body)">
		<path class="button" d={art.region.LEFT_CLICK} />
		<path class="button" d={art.region.RIGHT_CLICK} />
		{#each art.grips ?? [] as g (g)}<path d={g} fill="url(#{uid}-grip)" />{/each}
		{#each art.seams as d (d)}<path class="seam" {d} />{/each}
	</g>
	<path class="well" d={art.well} />
	<path class="wheel" d={art.region.WHEEL_CLICK} style:fill={(roles.wheel && color(roles.wheel)) ?? 'var(--shell-top)'} />
	<path class="tread" d={art.tread} />
	{#each side as k (k)}<path class="key" d={art.region[k]} />{/each}
	<path
		class="logo"
		d={SPIRAL}
		transform="translate({art.logo[0] - 12 * 1.7 * S} {art.logo[1] - 12 * 1.7 * S}) scale({1.7 * S})"
		style:stroke={(roles.logo && color(roles.logo)) ?? 'var(--seam-line)'}
	/>
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
		stroke-linecap: round;
		transition: stroke 120ms linear;
	}
	.shell,
	.seam,
	.tread,
	.well,
	.key,
	.logo {
		vector-effect: non-scaling-stroke;
	}
	.shell {
		fill: var(--case);
		stroke: var(--seam-line);
		stroke-width: 1.4;
	}
	.button {
		fill: var(--shell-top);
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
