// Every traced mouse in one shape: the generated ones (tools/art/mouse.py, ./mice/<id>.json) and the
// Basilisk V3 Pro, whose seams and buttons are drawn by hand (./basilisk.ts, tools/art/basilisk.py).
// Units: the source photo's pixels, front at the top.
import * as B from './basilisk';

export type Pt = [number, number];
export interface Box {
	x: number;
	y: number;
	w: number;
	h: number;
}
export interface MouseArt {
	view: Box;
	body_box: Box;
	body: string;
	seams: string[];
	/** Button regions by keymap name; LEFT_CLICK and RIGHT_CLICK are clipped to the body when drawn. */
	region: Record<string, string>;
	well: string;
	tread: string;
	logo: Pt;
	/** The outline round the sides and back, front left first: underglow strips are spread along it. */
	strip: Pt[];
	/** Where each named callout's line starts. */
	callouts: Record<string, Pt>;
	grips?: string[];
	/** Hand-drawn (the Basilisk V3 Pro) rather than placed by proportion. */
	drawn?: boolean;
}

const points = (d: string): Pt[] => [...d.matchAll(/(-?[\d.]+) (-?[\d.]+)/g)].map((m) => [+m[1], +m[2]]);

const BASILISK_V3_PRO: MouseArt = {
	view: B.VIEW,
	body_box: B.BODY_BOX,
	body: B.BODY,
	seams: B.SEAMS,
	region: { ...B.REGION },
	well: B.WELL,
	tread: B.TREAD,
	logo: B.LOGO,
	strip: B.STRIP_SEGMENTS.flatMap((s) => points(s.d)),
	callouts: { WHEEL_CLICK: [262, 238], SCROLL_MODE: [255, 330], DPI_BUTTON: [255, 362], FORWARD: [108, 242], BACK: [106, 284], CLUTCH: [84, 342] },
	grips: B.GRIPS,
	drawn: true
};

const generated = import.meta.glob<MouseArt>('./mice/*.json', { eager: true, import: 'default' });
const ART: Record<string, MouseArt> = { 'razer-basilisk-v3-pro': BASILISK_V3_PRO };
for (const [path, art] of Object.entries(generated)) ART[path.replace(/^.*\/(.+)\.json$/, '$1')] = art;

export const mouseArt = (id: string): MouseArt | undefined => ART[id];

/** Which strip segment (and so which LED) each part of the outline belongs to, for `count` strip LEDs. Each
 * part runs from its window's start to its end along the outline, so the parts meet without gaps however
 * coarse the outline is, and every one has at least two points. */
export function stripParts(art: MouseArt, count: number): string[] {
	const s = art.strip;
	if (count <= 0 || s.length < 2) return [];
	const d = [0];
	for (let i = 1; i < s.length; i++) d.push(d[i - 1] + Math.hypot(s[i][0] - s[i - 1][0], s[i][1] - s[i - 1][1]));
	const total = d[d.length - 1];
	if (total <= 0) return [];
	/** The point `len` along the outline. */
	const at = (len: number): Pt => {
		let i = 1;
		while (i < s.length - 1 && d[i] < len) i++;
		const span = d[i] - d[i - 1];
		const u = span > 0 ? Math.min(1, Math.max(0, (len - d[i - 1]) / span)) : 0;
		return [s[i - 1][0] + (s[i][0] - s[i - 1][0]) * u, s[i - 1][1] + (s[i][1] - s[i - 1][1]) * u];
	};
	const fmt = ([x, y]: Pt) => `${+x.toFixed(2)} ${+y.toFixed(2)}`;
	const out: string[] = [];
	for (let k = 0; k < count; k++) {
		const a = (total * k) / count;
		const b = (total * (k + 1)) / count;
		const inner = s.filter((_, i) => d[i] > a && d[i] < b);
		out.push('M' + [at(a), ...inner, at(b)].map(fmt).join(' L'));
	}
	return out;
}

/** The roles of a mouse's LEDs by their device-file names: the wheel, the logo, and the rest as its strip. */
export function ledRoles(shapes: string[]) {
	const wheel = shapes.find((s) => /wheel/i.test(s));
	const logo = shapes.find((s) => /logo/i.test(s));
	const strip = shapes.filter((s) => s !== wheel && s !== logo);
	return { wheel, logo, strip };
}

/** uncoil's spiral (the app mark), in a 24-unit box. */
export const SPIRAL = 'M12 12a2 2 0 1 1 2-2 4 4 0 1 1-4-4 6 6 0 1 1-6 6 8 8 0 1 1 8 8';
