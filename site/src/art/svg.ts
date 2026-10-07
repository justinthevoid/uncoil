// The desk as one static SVG string, from the same geometry as the canvas renderer: the still shown with no
// JavaScript and under reduced motion. Lit the way the app lights it (Keyboard.svelte colour mode: each cap in
// its LED colour, its skirt a shade darker; the mouse strip, wheel and spiral; the mat's edge band).
import { SPIRAL } from './mice';
import { FINISH, type DeskGeo, type RRect } from './geometry';

export interface SvgOptions {
	/** Hex colour per LED (global order). Missing = the LED's off colour. */
	colors?: (string | undefined)[];
	/** Crop to a box (desk units); defaults to the desk's bounds. */
	view?: { x: number; y: number; w: number; h: number };
	/** Gel tabs: key name -> colour. */
	gels?: Map<string, string>;
	/** Outlined keys: key name -> colour. */
	marks?: Map<string, string>;
	/** Prefix for ids (clip paths), unique per SVG on a page. */
	id?: string;
	/** Accessible label; without one the SVG is aria-hidden. */
	label?: string;
}

const f = (n: number) => +n.toFixed(3);
const rr = (r: RRect, attrs: string) => `<rect x="${f(r.x)}" y="${f(r.y)}" width="${f(r.w)}" height="${f(r.h)}" rx="${f(r.r)}" ${attrs}/>`;
function shade(hex: string, k: number) {
	const n = parseInt(hex.slice(1), 16);
	const c = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => Math.round(v * k));
	return `rgb(${c.join(',')})`;
}

export function deskSvg(geo: DeskGeo, o: SvgOptions = {}): string {
	const col = (i: number) => o.colors?.[i];
	const id = o.id ?? 'desk';
	const v = o.view ?? geo.bounds;
	const out: string[] = [];
	const hair = 'vector-effect="non-scaling-stroke"';
	for (const g of geo.devices) {
		if (g.kind === 'mat') {
			const b = g.box;
			out.push(rr({ ...g.hub }, `fill="${FINISH.shellTop}" stroke="${FINISH.caseEdge}" stroke-width="1" ${hair}`));
			out.push(rr({ ...b, r: g.r }, `fill="${FINISH.cloth}" stroke="rgba(0,0,0,0.35)" stroke-width="1" ${hair}`));
			if (g.ring !== undefined)
				out.push(
					rr({ x: b.x + g.E / 2, y: b.y + g.E / 2, w: b.w - g.E, h: b.h - g.E, r: g.r }, `fill="none" stroke="${col(g.ring) ?? FINISH.ledOff}" stroke-width="${g.E}"`),
				);
			for (const band of g.bands)
				out.push(
					`<path d="M${band.pts.map(([x, y]) => `${f(x)} ${f(y)}`).join(' L')}" fill="none" stroke="${col(band.led) ?? FINISH.ledOff}" stroke-width="${g.E}" stroke-linecap="round"/>`,
				);
		} else if (g.kind === 'keyboard') {
			for (const bar of g.bars) {
				const n = bar.leds.length;
				out.push(`<g><clipPath id="${id}-b${bar.leds[0]}">${rr({ ...bar, r: Math.min(bar.w, bar.h) / 2 }, '')}</clipPath><g clip-path="url(#${id}-b${bar.leds[0]})">`);
				bar.leds.forEach((led, k) => {
					const gap = 0.025;
					const seg = bar.vertical
						? { x: bar.x, y: bar.y + (bar.h * k) / n, w: bar.w, h: bar.h / n - gap }
						: { x: bar.x + (bar.w * k) / n, y: bar.y, w: bar.w / n - gap, h: bar.h };
					out.push(`<rect x="${f(seg.x)}" y="${f(seg.y)}" width="${f(seg.w)}" height="${f(seg.h)}" fill="${col(led) ?? FINISH.ledOff}"/>`);
				});
				out.push('</g></g>');
			}
			if (g.rest) out.push(rr(g.rest, `fill="${FINISH.shellTop}" stroke="${FINISH.caseEdge}" stroke-width="1" ${hair}`));
			if (g.dial) out.push(rr(g.dial, `fill="${FINISH.capSkirt}" stroke="${FINISH.caseEdge}" stroke-width="1" ${hair}`));
			for (const b of g.buttons) out.push(rr(b, `fill="${FINISH.capSkirt}" stroke="${FINISH.caseEdge}" stroke-width="1" ${hair}`));
			out.push(rr(g.shell, `fill="${FINISH.case}" stroke="${FINISH.caseEdge}" stroke-width="1" ${hair}`));
			if (g.lip) {
				out.push(`<line x1="${f(g.lip.x0 + g.shell.r * 0.3)}" y1="${f(g.lip.y)}" x2="${f(g.lip.x1 - g.shell.r * 0.3)}" y2="${f(g.lip.y)}" stroke="${FINISH.seam}" stroke-width="1" ${hair}/>`);
				const fs = Math.min(0.32, g.W * 0.0115 * 1.6);
				out.push(
					`<text x="${f((g.lip.x0 + g.lip.x1) / 2)}" y="${f((g.lip.y + g.lip.bottom) / 2)}" font-size="${f(fs)}" font-weight="600" letter-spacing="${f(fs * 0.12)}" fill="${FINISH.seam}" text-anchor="middle" dominant-baseline="central" font-family="system-ui, sans-serif">uncoil</text>`,
				);
			}
			for (const fr of g.frames) out.push(`<rect x="${f(fr.x)}" y="${f(fr.y)}" width="${f(fr.w)}" height="${f(fr.h)}" fill="${FINISH.plate}"/>`);
			if (g.oled) out.push(rr(g.oled, `fill="#1a1816" stroke="${FINISH.caseEdge}" stroke-width="1" ${hair}`));
			for (const d of g.dots) out.push(`<circle cx="${f(d.x)}" cy="${f(d.y)}" r="${d.r}" fill="${col(d.led) ?? FINISH.ledOff}"/>`);
			for (const c of g.caps) {
				const lit = col(c.led);
				out.push(rr(c, `fill="${lit ? shade(lit, 0.58) : FINISH.capSkirt}" stroke="${FINISH.capEdge}" stroke-width="1" ${hair}`));
				out.push(rr(c.top, `fill="${lit ?? FINISH.cap}"`));
				const gel = o.gels?.get(c.name);
				if (gel) out.push(`<rect x="${f(c.x)}" y="${f(c.y)}" width="${f(c.w)}" height="${f(Math.max(0.09, g.W * 0.0045))}" fill="${gel}"/>`);
				const mark = o.marks?.get(c.name);
				if (mark) out.push(rr({ x: c.x - 0.03, y: c.y - 0.03, w: c.w + 0.06, h: c.h + 0.06, r: c.r }, `fill="none" stroke="${mark}" stroke-width="2" ${hair}`));
			}
		} else if (g.kind === 'mouse') {
			const a = g.art;
			const S = g.S;
			out.push(`<g transform="matrix(${f(g.u)} 0 0 ${f(g.u)} ${f(g.tx)} ${f(g.ty)})">`);
			out.push(`<clipPath id="${id}-mb"><path d="${a.body}"/></clipPath>`);
			for (const s of g.strip) out.push(`<path d="${s.d}" fill="none" stroke-linecap="round" stroke-width="${9 * S}" stroke="${col(s.led) ?? FINISH.ledOff}"/>`);
			out.push(`<path d="${a.body}" fill="${FINISH.case}" stroke="${FINISH.seam}" stroke-width="1.4" ${hair}/>`);
			out.push(`<g clip-path="url(#${id}-mb)">`);
			for (const k of ['LEFT_CLICK', 'RIGHT_CLICK']) if (a.region[k]) out.push(`<path d="${a.region[k]}" fill="${FINISH.shellTop}"/>`);
			for (const d of a.seams) out.push(`<path d="${d}" fill="none" stroke="${FINISH.seam}" stroke-width="1.2" stroke-linecap="round" ${hair}/>`);
			out.push('</g>');
			out.push(`<path d="${a.well}" fill="${FINISH.case}" stroke="${FINISH.seam}" stroke-width="1.2" ${hair}/>`);
			out.push(`<path d="${a.region.WHEEL_CLICK}" fill="${(g.wheel !== undefined && col(g.wheel)) || FINISH.shellTop}"/>`);
			out.push(`<path d="${a.tread}" fill="none" stroke="${FINISH.seam}" stroke-width="1.2" stroke-linecap="round" ${hair}/>`);
			for (const k of g.side) out.push(`<path d="${a.region[k]}" fill="${FINISH.shellTop}" stroke="${FINISH.seam}" stroke-width="1.2" ${hair}/>`);
			out.push(
				`<path d="${SPIRAL}" transform="translate(${f(g.logoAt.x)} ${f(g.logoAt.y)}) scale(${f(g.logoAt.s)})" fill="none" stroke-linecap="round" stroke-width="1.7" ${hair} stroke="${(g.logo !== undefined && col(g.logo)) || FINISH.seam}"/>`,
			);
			out.push('</g>');
		} else {
			for (const d of g.dots) out.push(`<circle cx="${f(d.x)}" cy="${f(d.y)}" r="${d.r}" fill="${col(d.led) ?? FINISH.ledOff}"/>`);
		}
	}
	const a11y = o.label ? `role="img" aria-label="${o.label.replace(/"/g, '&quot;')}"` : 'aria-hidden="true"';
	return `<svg viewBox="${f(v.x)} ${f(v.y)} ${f(v.w)} ${f(v.h)}" xmlns="http://www.w3.org/2000/svg" ${a11y} preserveAspectRatio="xMidYMid meet">${out.join('')}</svg>`;
}
