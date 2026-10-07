// The traced mice the site draws, without the app's whole registry: lib/art/mice.ts builds every traced mouse
// eagerly (each one a large path), which would ship ten mice to draw one. This assembles the Basilisk V3 Pro the
// same way mice.ts does, from the same data (lib/art/basilisk.ts), and copies its two small pure helpers.
// Keep `basilisk`, `stripParts` and `ledRoles` identical to lib/art/mice.ts. For another traced mouse, pass the
// app's `mouseArt` to `deskGeometry` (build time is fine; the browser then ships that mouse's path too).
import * as B from '$uncoil/art/basilisk';
import type { MouseArt, Pt } from '$uncoil/art/mice';

const points = (d: string): Pt[] => [...d.matchAll(/(-?[\d.]+) (-?[\d.]+)/g)].map((m) => [+m[1], +m[2]]);

/** lib/art/mice.ts BASILISK_V3_PRO. */
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
	drawn: true,
};

export const siteMouseArt = (id: string): MouseArt | undefined => (id === 'razer-basilisk-v3-pro' ? BASILISK_V3_PRO : undefined);

/** lib/art/mice.ts stripParts: which strip segment (and so which LED) each part of the outline belongs to. */
export function stripParts(art: MouseArt, count: number): string[] {
	const s = art.strip;
	if (count <= 0 || s.length < 2) return [];
	const d = [0];
	for (let i = 1; i < s.length; i++) d.push(d[i - 1] + Math.hypot(s[i][0] - s[i - 1][0], s[i][1] - s[i - 1][1]));
	const total = d[d.length - 1];
	if (total <= 0) return [];
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

/** lib/art/mice.ts ledRoles: the wheel, the logo, and the rest as its strip. */
export function ledRoles(shapes: string[]) {
	const wheel = shapes.find((s) => /wheel/i.test(s));
	const logo = shapes.find((s) => /logo/i.test(s));
	const strip = shapes.filter((s) => s !== wheel && s !== logo);
	return { wheel, logo, strip };
}

/** uncoil's spiral (the app mark), in a 24-unit box: lib/art/mice.ts SPIRAL. */
export const SPIRAL = 'M12 12a2 2 0 1 1 2-2 4 4 0 1 1-4-4 6 6 0 1 1-6 6 8 8 0 1 1 8 8';
