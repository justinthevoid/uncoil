// The desk on a canvas: the app's device art (./geometry) drawn at any camera, lit per LED, with light that
// behaves like light (a soft spill from the LEDs onto the mat, caps lit from within). Build once, draw per
// frame:
//
//   const desk = createDeskRenderer(deskGeometry(devices));
//   desk.draw(ctx, camera, dpr, { rgb, light, pen: 1 });
//
// Static line art is cached in offscreen canvases per zoom step, so a settled frame is a few drawImage calls
// plus the lit parts. While `pen` < 1 the line art draws itself along its paths, in pen order: mat edge,
// keyboard case, frames, caps, mouse body, seams.
import { SPIRAL } from './mice';
import { FINISH, type DeskGeo, type KeyboardGeo, type MatGeo, type MouseGeo, type RRect } from './geometry';

/** Where the desk sits on screen: desk point (cx, cy) lands on screen point (sx, sy), k px per key unit. */
export interface Camera {
	sx: number;
	sy: number;
	cx: number;
	cy: number;
	k: number;
	/** Radians, about (sx, sy). */
	rot?: number;
}

export interface DeskFrame {
	/** The colour each LED shows when lit: r, g, b (0..255) per LED, global order. */
	rgb: ArrayLike<number>;
	/** How lit each LED is, 0 (off: the finish's dark LED) to 1. */
	light: ArrayLike<number>;
	/** Line art drawn in, 0..1 (1 = complete, cached). */
	pen?: number;
	/** The whole desk's opacity. */
	alpha?: number;
	/** Light spill strength (1 = the default, subtle). */
	glow?: number;
	/** Gel tabs and second lines: key name -> { colour, sub }. */
	gels?: Map<string, { color: string; sub?: string }>;
	/** Legend opacity on caps (Keys-screen look). */
	legends?: number;
	/** Outlined keys: key name -> CSS colour. */
	marks?: Map<string, string>;
	/** Keys held down, 0..1. */
	pressed?: Map<string, number>;
	/** 0 full, 1 lighter, 2 lightest. */
	quality?: number;
}

export interface DeskRenderer {
	geo: DeskGeo;
	draw(ctx: CanvasRenderingContext2D, cam: Camera, dpr: number, f: DeskFrame): void;
	/** Screen position of LED i's drawing under a camera. */
	toScreen(cam: Camera, x: number, y: number): [number, number];
	/** A key's cap in desk units. */
	cap(name: string): RRect | undefined;
}

const PEN_ORDER = ['mat', 'case', 'frames', 'caps', 'mouse', 'seams'] as const;
const PEN_WEIGHT = [1, 1.1, 0.7, 1.7, 1, 1];
const MONO = 'ui-monospace, "Cascadia Mono", Consolas, monospace';
const clamp01 = (x: number) => (x < 0 ? 0 : x > 1 ? 1 : x);

function hexRgb(h: string): [number, number, number] {
	const n = parseInt(h.slice(1), 16);
	return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
const OFF = hexRgb(FINISH.ledOff);
const CAP = hexRgb(FINISH.cap);
const SKIRT = hexRgb(FINISH.capSkirt);
const SHELL = hexRgb(FINISH.shellTop);
const rgbStr = (r: number, g: number, b: number) => `rgb(${r | 0},${g | 0},${b | 0})`;

function rrPath(p: Path2D | CanvasRenderingContext2D, r: RRect, grow = 0) {
	p.roundRect(r.x - grow, r.y - grow, r.w + grow * 2, r.h + grow * 2, Math.max(0, r.r + grow));
}
const rrLen = (r: RRect) => 2 * (r.w + r.h) - (8 - 2 * Math.PI) * r.r;

/** Length of an SVG path, by the DOM where it can, else by its points. */
function pathLength(d: string): number {
	try {
		const el = document.createElementNS('http://www.w3.org/2000/svg', 'path');
		el.setAttribute('d', d);
		const l = el.getTotalLength();
		if (l > 0) return l;
	} catch {
		/* fall through */
	}
	const n = [...d.matchAll(/-?\d+(?:\.\d+)?/g)].map((m) => +m[0]);
	let l = 0;
	for (let i = 2; i + 1 < n.length; i += 2) l += Math.hypot(n[i] - n[i - 2], n[i + 1] - n[i - 1]);
	return l;
}

export function createDeskRenderer(geo: DeskGeo): DeskRenderer {
	const kb = geo.devices.find((d): d is KeyboardGeo => d.kind === 'keyboard');
	const mouse = geo.devices.find((d): d is MouseGeo => d.kind === 'mouse');
	const mat = geo.devices.find((d): d is MatGeo => d.kind === 'mat');
	const B = geo.bounds;
	const capByName = new Map(kb?.caps.map((c) => [c.name, c]) ?? []);

	// ---- paths, built once (desk units; the mouse's in photo pixels under its own transform) -----------
	const P = {
		matSurface: new Path2D(),
		hub: new Path2D(),
		matRing: new Path2D(),
		shell: new Path2D(),
		lip: new Path2D(),
		oled: new Path2D(),
		dial: new Path2D(),
		buttons: new Path2D(),
		rest: new Path2D(),
		frames: new Path2D(),
		body: mouse ? new Path2D(mouse.art.body) : new Path2D(),
		clicks: new Path2D(),
		seams: new Path2D(),
		well: mouse ? new Path2D(mouse.art.well) : new Path2D(),
		wheel: mouse ? new Path2D(mouse.art.region.WHEEL_CLICK) : new Path2D(),
		tread: mouse ? new Path2D(mouse.art.tread) : new Path2D(),
		side: new Path2D(),
		strip: mouse ? mouse.strip.map((s) => new Path2D(s.d)) : [],
		spiral: new Path2D(SPIRAL),
		grips: new Path2D(),
	};
	if (mat) {
		const b = mat.box;
		rrPath(P.matSurface, { ...b, r: mat.r });
		rrPath(P.hub, mat.hub);
		rrPath(P.matRing, { x: b.x + mat.E / 2, y: b.y + mat.E / 2, w: b.w - mat.E, h: b.h - mat.E, r: mat.r });
	}
	if (kb) {
		rrPath(P.shell, kb.shell);
		if (kb.lip) {
			P.lip.moveTo(kb.lip.x0 + kb.shell.r * 0.3, kb.lip.y);
			P.lip.lineTo(kb.lip.x1 - kb.shell.r * 0.3, kb.lip.y);
		}
		if (kb.oled) rrPath(P.oled, kb.oled);
		if (kb.dial) rrPath(P.dial, kb.dial);
		for (const b of kb.buttons) rrPath(P.buttons, b);
		if (kb.rest) rrPath(P.rest, kb.rest);
		for (const fr of kb.frames) P.frames.rect(fr.x, fr.y, fr.w, fr.h);
	}
	if (mouse) {
		const a = mouse.art;
		for (const k of ['LEFT_CLICK', 'RIGHT_CLICK']) if (a.region[k]) P.clicks.addPath(new Path2D(a.region[k]));
		for (const d of a.seams) P.seams.addPath(new Path2D(d));
		for (const k of mouse.side) P.side.addPath(new Path2D(a.region[k]));
		for (const g of a.grips ?? []) P.grips.addPath(new Path2D(g));
	}

	// ---- pen: each stroke's length, in pen order ------------------------------------------------------
	const pen = {
		matLen: mat ? rrLen({ ...mat.box, r: mat.r }) : 0,
		caseLen: kb ? rrLen(kb.shell) : 0,
		bodyLen: mouse ? pathLength(mouse.art.body) : 0,
		seamLens: mouse ? mouse.art.seams.map(pathLength) : [],
		capOrder: kb ? kb.caps.map((c) => (c.x - kb.box.x) / kb.box.w) : [],
	};
	const penW = PEN_WEIGHT.reduce((a, b) => a + b, 0);
	function penGroups(p: number) {
		const out: Record<(typeof PEN_ORDER)[number], number> = { mat: 1, case: 1, frames: 1, caps: 1, mouse: 1, seams: 1 };
		let acc = 0;
		PEN_ORDER.forEach((k, i) => {
			const w = PEN_WEIGHT[i] / penW;
			// groups overlap a little, so the pen never stops
			const from = Math.max(0, acc - w * 0.25);
			out[k] = clamp01((p - from) / (w * 1.25));
			acc += w;
		});
		return out;
	}
	const fillOf = (g: number) => clamp01((g - 0.55) / 0.45);

	// ---- the camera --------------------------------------------------------------------------------------
	function setCam(ctx: CanvasRenderingContext2D, cam: Camera, dpr: number) {
		const r = cam.rot ?? 0;
		const c = Math.cos(r) * cam.k * dpr;
		const s = Math.sin(r) * cam.k * dpr;
		ctx.setTransform(c, s, -s, c, dpr * cam.sx - (c * cam.cx - s * cam.cy), dpr * cam.sy - (s * cam.cx + c * cam.cy));
	}
	function toScreen(cam: Camera, x: number, y: number): [number, number] {
		const r = cam.rot ?? 0;
		const dx = (x - cam.cx) * cam.k;
		const dy = (y - cam.cy) * cam.k;
		return [cam.sx + dx * Math.cos(r) - dy * Math.sin(r), cam.sy + dx * Math.sin(r) + dy * Math.cos(r)];
	}
	function mouseT(ctx: CanvasRenderingContext2D) {
		if (mouse) ctx.transform(mouse.u, 0, 0, mouse.u, mouse.tx, mouse.ty);
	}

	// ---- static layers ----------------------------------------------------------------------------------
	// under: the mat; over: device bodies. `px` = one CSS pixel in desk units at this scale.
	function drawUnder(ctx: CanvasRenderingContext2D, px: number, g?: ReturnType<typeof penGroups>) {
		if (!mat) return;
		const fa = g ? fillOf(g.mat) : 1;
		ctx.globalAlpha *= 1;
		if (fa > 0) {
			ctx.save();
			ctx.globalAlpha *= fa;
			ctx.fillStyle = FINISH.shellTop;
			ctx.fill(P.hub);
			ctx.lineWidth = px;
			ctx.strokeStyle = FINISH.caseEdge;
			ctx.stroke(P.hub);
			ctx.fillStyle = FINISH.cloth;
			ctx.fill(P.matSurface);
			ctx.restore();
		}
		ctx.lineWidth = px;
		ctx.strokeStyle = g && g.mat < 1 ? 'rgba(236,230,218,0.35)' : 'rgba(0,0,0,0.35)';
		if (g && g.mat < 1) ctx.setLineDash([pen.matLen * g.mat, pen.matLen]);
		if (!g || g.mat > 0) ctx.stroke(P.matSurface);
		ctx.setLineDash([]);
	}
	function drawOver(ctx: CanvasRenderingContext2D, px: number, g?: ReturnType<typeof penGroups>) {
		if (kb) {
			const gc = g ? g.case : 1;
			const fa = fillOf(gc);
			const draft = g && gc < 1;
			ctx.save();
			ctx.globalAlpha *= fa;
			if (kb.rest) {
				ctx.fillStyle = FINISH.shellTop;
				ctx.fill(P.rest);
			}
			ctx.fillStyle = FINISH.capSkirt;
			ctx.fill(P.dial);
			ctx.fill(P.buttons);
			if (kb.dial) {
				// the dial's knurl: fine stripes across it
				ctx.save();
				ctx.clip(P.dial);
				ctx.fillStyle = FINISH.cap;
				for (let y = kb.dial.y; y < kb.dial.y + kb.dial.h; y += 4 * px) ctx.fillRect(kb.dial.x, y + 2 * px, kb.dial.w, 2 * px);
				ctx.restore();
			}
			ctx.fillStyle = FINISH.case;
			ctx.fill(P.shell);
			ctx.restore();
			ctx.lineWidth = px;
			ctx.strokeStyle = draft ? 'rgba(236,230,218,0.4)' : FINISH.caseEdge;
			if (draft) ctx.setLineDash([pen.caseLen * gc, pen.caseLen]);
			if (gc > 0) {
				ctx.stroke(P.shell);
				ctx.stroke(P.dial);
				ctx.stroke(P.buttons);
				if (kb.rest) ctx.stroke(P.rest);
			}
			ctx.setLineDash([]);
			if (kb.lip && gc > 0.3) {
				ctx.save();
				ctx.globalAlpha *= clamp01((gc - 0.3) / 0.7);
				ctx.strokeStyle = FINISH.seam;
				ctx.stroke(P.lip);
				ctx.restore();
			}
			// frames, the screen
			const gf = g ? g.frames : 1;
			if (gf > 0) {
				ctx.save();
				ctx.globalAlpha *= fillOf(gf);
				ctx.fillStyle = FINISH.plate;
				ctx.fill(P.frames);
				ctx.fillStyle = '#1a1816';
				ctx.fill(P.oled);
				ctx.restore();
				ctx.lineWidth = px;
				ctx.strokeStyle = FINISH.caseEdge;
				ctx.save();
				ctx.globalAlpha *= clamp01(gf * 1.5);
				ctx.stroke(P.oled);
				ctx.restore();
				if (g && gf < 1) {
					ctx.strokeStyle = 'rgba(236,230,218,0.22)';
					ctx.setLineDash([gf * 6, 6]);
					ctx.stroke(P.frames);
					ctx.setLineDash([]);
				}
			}
		}
		if (mouse) {
			const gm = g ? g.mouse : 1;
			const gs = g ? g.seams : 1;
			ctx.save();
			mouseT(ctx);
			const mpx = px / mouse.u;
			ctx.save();
			ctx.globalAlpha *= fillOf(gm);
			ctx.fillStyle = FINISH.case;
			ctx.fill(P.body);
			ctx.restore();
			ctx.lineWidth = 1.4 * mpx;
			ctx.strokeStyle = g && gm < 1 ? 'rgba(236,230,218,0.45)' : FINISH.seam;
			if (g && gm < 1) ctx.setLineDash([pen.bodyLen * gm, pen.bodyLen]);
			if (gm > 0) ctx.stroke(P.body);
			ctx.setLineDash([]);
			if (gs > 0) {
				ctx.save();
				ctx.clip(P.body);
				ctx.save();
				ctx.globalAlpha *= fillOf(gs);
				ctx.fillStyle = FINISH.shellTop;
				ctx.fill(P.clicks);
				// grips: a fine dot texture
				if (!g || gs >= 1) {
					ctx.clip(P.grips);
					ctx.fillStyle = FINISH.seam;
					const step = 4;
					const bb = mouse.art.view;
					for (let y = bb.y; y < bb.y + bb.h; y += step)
						for (let x = bb.x + ((y / step) % 2) * (step / 2); x < bb.x + bb.w; x += step) {
							ctx.beginPath();
							ctx.arc(x, y, 0.8, 0, Math.PI * 2);
							ctx.fill();
						}
				}
				ctx.restore();
				ctx.lineWidth = 1.2 * mpx;
				ctx.lineCap = 'round';
				ctx.strokeStyle = FINISH.seam;
				if (g && gs < 1) {
					mouse.art.seams.forEach((d, i) => {
						const l = pen.seamLens[i];
						ctx.setLineDash([l * gs, l]);
						ctx.stroke(new Path2D(d));
					});
					ctx.setLineDash([]);
				} else ctx.stroke(P.seams);
				ctx.restore();
				ctx.save();
				ctx.globalAlpha *= fillOf(gs);
				ctx.lineWidth = 1.2 * mpx;
				ctx.fillStyle = FINISH.case;
				ctx.strokeStyle = FINISH.seam;
				ctx.fill(P.well);
				ctx.stroke(P.well);
				ctx.fillStyle = FINISH.shellTop;
				ctx.fill(P.side);
				ctx.stroke(P.side);
				ctx.restore();
			}
			ctx.restore();
		}
	}

	// Cache: two canvases (under, over) of the desk bounds at a zoom step; rebuilt when the step changes.
	const cache = { step: -1, under: null as HTMLCanvasElement | null, over: null as HTMLCanvasElement | null, s: 1 };
	function cached(scale: number, dprPx: number) {
		const step = Math.round(Math.log2(scale) * 8);
		const s = 2 ** (step / 8);
		const w = Math.ceil(B.w * s);
		const h = Math.ceil(B.h * s);
		if (w > 4096 || h > 4096 || w < 8) return null;
		if (cache.step === step && cache.under) return cache;
		const mk = (fn: (c: CanvasRenderingContext2D, px: number) => void) => {
			const cv = document.createElement('canvas');
			cv.width = w;
			cv.height = h;
			const c = cv.getContext('2d')!;
			c.setTransform(s, 0, 0, s, -B.x * s, -B.y * s);
			fn(c, dprPx / s);
			return cv;
		};
		cache.under = mk((c, px) => drawUnder(c, px));
		cache.over = mk((c, px) => drawOver(c, px));
		cache.step = step;
		cache.s = s;
		return cache;
	}

	// ---- light: a low-res buffer of the LED field, blurred by down/up-sampling, added onto the mat -------
	const LPS = 7; // buffer px per key unit
	const lb = document.createElement('canvas');
	lb.width = Math.ceil(B.w * LPS);
	lb.height = Math.ceil(B.h * LPS);
	const lctx = lb.getContext('2d')!;
	const half = document.createElement('canvas');
	half.width = Math.ceil(lb.width / 2);
	half.height = Math.ceil(lb.height / 2);
	const hctx = half.getContext('2d')!;
	const quarter = document.createElement('canvas');
	quarter.width = Math.ceil(lb.width / 4);
	quarter.height = Math.ceil(lb.height / 4);
	const qctx = quarter.getContext('2d')!;

	const N = geo.ledCount;
	/** Per LED: the colour as shown (mixed with the dark finish by how lit it is), the light itself, how lit. */
	const C = { r: new Float32Array(N), g: new Float32Array(N), b: new Float32Array(N), on: new Float32Array(N), raw: new Uint8ClampedArray(N * 3) };
	function resolve(f: DeskFrame) {
		for (let i = 0; i < N; i++) {
			const L = clamp01(f.light[i] ?? 0);
			const r = f.rgb[i * 3];
			const g = f.rgb[i * 3 + 1];
			const b = f.rgb[i * 3 + 2];
			C.on[i] = L;
			C.raw[i * 3] = r;
			C.raw[i * 3 + 1] = g;
			C.raw[i * 3 + 2] = b;
			C.r[i] = OFF[0] + (r - OFF[0]) * L;
			C.g[i] = OFF[1] + (g - OFF[1]) * L;
			C.b[i] = OFF[2] + (b - OFF[2]) * L;
		}
	}
	/** The light itself at LED i, faded by how lit it is. */
	const lit = (i: number, a = 1) => `rgba(${C.raw[i * 3]},${C.raw[i * 3 + 1]},${C.raw[i * 3 + 2]},${(a * C.on[i]).toFixed(3)})`;

	function paintLight() {
		lctx.setTransform(1, 0, 0, 1, 0, 0);
		lctx.clearRect(0, 0, lb.width, lb.height);
		lctx.setTransform(LPS, 0, 0, LPS, -B.x * LPS, -B.y * LPS);
		lctx.globalCompositeOperation = 'lighter';
		if (mat?.ring !== undefined && C.on[mat.ring] > 0) {
			lctx.lineWidth = 0.45;
			lctx.strokeStyle = lit(mat.ring, 0.65);
			lctx.stroke(P.matRing);
		}
		for (const band of mat?.bands ?? []) {
			lctx.lineWidth = 0.6;
			lctx.strokeStyle = lit(band.led, 0.9);
			lctx.beginPath();
			band.pts.forEach(([x, y], k) => (k ? lctx.lineTo(x, y) : lctx.moveTo(x, y)));
			lctx.stroke();
		}
		if (kb) {
			for (const bar of kb.bars) {
				const n = bar.leds.length;
				bar.leds.forEach((led, k) => {
					if (!C.on[led]) return;
					lctx.fillStyle = lit(led, 1);
					if (bar.vertical) lctx.fillRect(bar.x - 0.35, bar.y + (bar.h * k) / n, bar.w + 0.7, bar.h / n);
					else lctx.fillRect(bar.x + (bar.w * k) / n, bar.y - 0.35, bar.w / n, bar.h + 0.7);
				});
			}
			for (const d of kb.dots) {
				if (!C.on[d.led]) continue;
				lctx.fillStyle = lit(d.led, 1);
				lctx.fillRect(d.x - 0.4, d.y - 0.4, 0.8, 0.8);
			}
			for (const c of kb.caps) {
				if (!C.on[c.led]) continue;
				lctx.fillStyle = lit(c.led, 0.42);
				lctx.fillRect(c.x - 0.05, c.y - 0.05, c.w + 0.1, c.h + 0.1);
			}
		}
		if (mouse) {
			lctx.save();
			lctx.transform(mouse.u, 0, 0, mouse.u, mouse.tx, mouse.ty);
			lctx.lineWidth = 26 * mouse.S;
			lctx.lineCap = 'round';
			mouse.strip.forEach((s, k) => {
				if (!C.on[s.led]) return;
				lctx.strokeStyle = lit(s.led, 0.85);
				lctx.stroke(P.strip[k]);
			});
			if (mouse.logo !== undefined && C.on[mouse.logo]) {
				lctx.fillStyle = lit(mouse.logo, 0.6);
				lctx.beginPath();
				lctx.arc(mouse.art.logo[0], mouse.art.logo[1], 26 * mouse.S, 0, Math.PI * 2);
				lctx.fill();
			}
			lctx.restore();
		}
		lctx.globalCompositeOperation = 'source-over';
		// blur: down twice, up once (smoothing does the rest when it is drawn to the screen)
		hctx.globalCompositeOperation = 'copy';
		hctx.drawImage(lb, 0, 0, half.width, half.height);
		qctx.globalCompositeOperation = 'copy';
		qctx.drawImage(half, 0, 0, quarter.width, quarter.height);
		hctx.drawImage(quarter, 0, 0, half.width, half.height);
	}

	// ---- the lit parts, drawn live each frame -------------------------------------------------------------
	function drawLitUnder(ctx: CanvasRenderingContext2D, px: number) {
		if (mat?.ring !== undefined) {
			ctx.lineWidth = mat.E;
			ctx.strokeStyle = rgbStr(C.r[mat.ring], C.g[mat.ring], C.b[mat.ring]);
			ctx.stroke(P.matRing);
		}
		for (const band of mat?.bands ?? []) {
			ctx.lineWidth = mat!.E;
			ctx.lineCap = 'round';
			ctx.strokeStyle = rgbStr(C.r[band.led], C.g[band.led], C.b[band.led]);
			ctx.beginPath();
			band.pts.forEach(([x, y], k) => (k ? ctx.lineTo(x, y) : ctx.moveTo(x, y)));
			ctx.stroke();
		}
		if (kb) {
			for (const bar of kb.bars) {
				const n = bar.leds.length;
				ctx.save();
				ctx.beginPath();
				ctx.roundRect(bar.x, bar.y, bar.w, bar.h, Math.min(bar.w, bar.h) / 2);
				ctx.clip();
				bar.leds.forEach((led, k) => {
					ctx.fillStyle = rgbStr(C.r[led], C.g[led], C.b[led]);
					if (bar.vertical) ctx.fillRect(bar.x, bar.y + (bar.h * k) / n, bar.w, bar.h / n - px);
					else ctx.fillRect(bar.x + (bar.w * k) / n, bar.y, bar.w / n - px, bar.h);
				});
				ctx.restore();
			}
		}
		if (mouse) {
			ctx.save();
			mouseT(ctx);
			ctx.lineWidth = 9 * mouse.S;
			ctx.lineCap = 'round';
			mouse.strip.forEach((s, k) => {
				ctx.strokeStyle = rgbStr(C.r[s.led], C.g[s.led], C.b[s.led]);
				ctx.stroke(P.strip[k]);
			});
			ctx.restore();
		}
	}

	function drawLitOver(ctx: CanvasRenderingContext2D, cam: Camera, dpr: number, px: number, f: DeskFrame, gCaps: number, gCase = 1, gSeams = 1) {
		if (kb && gCase > 0) {
			ctx.globalAlpha = (f.alpha ?? 1) * fillOf(gCase);
			for (const d of kb.dots) {
				ctx.fillStyle = rgbStr(C.r[d.led], C.g[d.led], C.b[d.led]);
				ctx.beginPath();
				ctx.arc(d.x, d.y, d.r, 0, Math.PI * 2);
				ctx.fill();
			}
			ctx.globalAlpha = f.alpha ?? 1;
		}
		if (kb) {
			const drafting = gCaps < 1;
			ctx.lineWidth = px;
			for (let n = 0; n < kb.caps.length; n++) {
				const c = kb.caps[n];
				const gi = drafting ? clamp01(gCaps * 1.5 - pen.capOrder[n] * 0.5) : 1;
				if (gi <= 0) continue;
				const L = C.on[c.led];
				const lr = C.r[c.led];
				const lg = C.g[c.led];
				const lb2 = C.b[c.led];
				const fa = fillOf(gi);
				const press = f.pressed?.get(c.name) ?? 0;
				if (fa > 0) {
					ctx.globalAlpha = fa * (f.alpha ?? 1);
					// skirt: the LED's light a shade darker (Keyboard.svelte: 58% toward black)
					const kr = SKIRT[0] + (lr * 0.58 - SKIRT[0]) * L;
					const kg = SKIRT[1] + (lg * 0.58 - SKIRT[1]) * L;
					const kbb = SKIRT[2] + (lb2 * 0.58 - SKIRT[2]) * L;
					ctx.fillStyle = rgbStr(kr, kg, kbb);
					ctx.beginPath();
					ctx.roundRect(c.x, c.y, c.w, c.h, c.r);
					ctx.fill();
					ctx.strokeStyle = FINISH.capEdge;
					ctx.stroke();
					// top face: lit from within
					const t = c.top;
					const dy = press * 0.06;
					ctx.fillStyle = rgbStr(CAP[0] + (lr - CAP[0]) * L, CAP[1] + (lg - CAP[1]) * L, CAP[2] + (lb2 - CAP[2]) * L);
					ctx.beginPath();
					ctx.roundRect(t.x, t.y + dy, t.w, t.h, t.r);
					ctx.fill();
					if (L > 0.05) {
						// lit from within: the face is a touch brighter toward its back edge
						ctx.fillStyle = `rgba(255,255,255,${(0.07 * L).toFixed(3)})`;
						ctx.beginPath();
						ctx.roundRect(t.x, t.y + dy, t.w, t.h * 0.45, [t.r, t.r, 0, 0]);
						ctx.fill();
					}
				}
				if (drafting && gi < 1) {
					ctx.globalAlpha = (f.alpha ?? 1) * (1 - fa * 0.7);
					ctx.strokeStyle = 'rgba(236,230,218,0.45)';
					const len = rrLen(c);
					ctx.setLineDash([len * gi, len]);
					ctx.beginPath();
					ctx.roundRect(c.x, c.y, c.w, c.h, c.r);
					ctx.stroke();
					ctx.setLineDash([]);
				}
			}
			ctx.globalAlpha = f.alpha ?? 1;
		}
		if (mouse && gSeams > 0) {
			ctx.save();
			ctx.globalAlpha = (f.alpha ?? 1) * fillOf(gSeams);
			mouseT(ctx);
			const mpx = px / mouse.u;
			if (mouse.wheel !== undefined) {
				const w = mouse.wheel;
				const L = C.on[w];
				ctx.fillStyle = rgbStr(SHELL[0] + (f.rgb[w * 3] - SHELL[0]) * L, SHELL[1] + (f.rgb[w * 3 + 1] - SHELL[1]) * L, SHELL[2] + (f.rgb[w * 3 + 2] - SHELL[2]) * L);
				ctx.fill(P.wheel);
			}
			ctx.lineWidth = 1.2 * mpx;
			ctx.lineCap = 'round';
			ctx.strokeStyle = FINISH.seam;
			ctx.stroke(P.tread);
			if (mouse.logo !== undefined) {
				const L = C.on[mouse.logo];
				ctx.save();
				ctx.translate(mouse.logoAt.x, mouse.logoAt.y);
				ctx.scale(mouse.logoAt.s, mouse.logoAt.s);
				ctx.lineWidth = (1.7 * mpx) / mouse.logoAt.s;
				ctx.strokeStyle = L > 0.01 ? rgbStr(C.r[mouse.logo], C.g[mouse.logo], C.b[mouse.logo]) : FINISH.seam;
				ctx.stroke(P.spiral);
				ctx.restore();
			}
			ctx.restore();
		}
	}

	// Words on caps (legends, gel second lines) go on in screen space, so they stay crisp at any zoom.
	function drawKeyText(ctx: CanvasRenderingContext2D, cam: Camera, dpr: number, f: DeskFrame) {
		if (!kb) return;
		const capPx = cam.k;
		const legends = f.legends ?? 0;
		// lit caps: the legend reads through, lighter than the cap
		const shine = capPx >= 26 && C.on.some((v) => v > 0.3);
		if (capPx < 18 || (legends <= 0 && !shine && !f.gels?.size)) return;
		ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
		ctx.textAlign = 'center';
		ctx.textBaseline = 'middle';
		const fs = Math.max(9, Math.min(15, capPx * 0.27));
		for (const c of kb.caps) {
			const [x0, y0] = toScreen(cam, c.top.x, c.top.y);
			const [x1, y1] = toScreen(cam, c.top.x + c.top.w, c.top.y + c.top.h);
			const cx = (x0 + x1) / 2;
			const gel = f.gels?.get(c.name);
			const L = C.on[c.led];
			if (shine && legends <= 0 && c.legend && L > 0.3) {
				ctx.font = `600 ${fs}px ${MONO}`;
				const lr = C.raw[c.led * 3], lg = C.raw[c.led * 3 + 1], lbb = C.raw[c.led * 3 + 2];
				ctx.fillStyle = `rgba(${(lr + 255) >> 1},${(lg + 255) >> 1},${(lbb + 255) >> 1},${(0.55 * L).toFixed(3)})`;
				ctx.fillText(c.legend, cx, (y0 + y1) / 2);
			}
			if (legends > 0 && c.legend) {
				ctx.font = `600 ${fs}px ${MONO}`;
				ctx.fillStyle = L > 0.5 ? `rgba(255,255,255,${(0.55 * legends).toFixed(3)})` : `rgba(236,230,218,${(0.82 * legends).toFixed(3)})`;
				ctx.fillText(c.legend, cx, gel?.sub && capPx > 34 ? y0 + (y1 - y0) * 0.36 : (y0 + y1) / 2);
			}
			if (gel?.sub && capPx > 34) {
				ctx.font = `500 ${Math.max(8, fs * 0.72)}px ${MONO}`;
				ctx.fillStyle = 'rgba(236,230,218,0.62)';
				ctx.fillText(gel.sub, cx, y0 + (y1 - y0) * 0.76);
			}
		}
		ctx.textAlign = 'start';
		ctx.textBaseline = 'alphabetic';
	}

	function drawGels(ctx: CanvasRenderingContext2D, px: number, f: DeskFrame) {
		if (!kb) return;
		if (f.gels)
			for (const [name, g] of f.gels) {
				const c = capByName.get(name);
				if (!c) continue;
				ctx.fillStyle = g.color;
				ctx.save();
				ctx.beginPath();
				ctx.roundRect(c.x, c.y, c.w, c.h, c.r);
				ctx.clip();
				ctx.fillRect(c.x, c.y, c.w, Math.max(3 * px, kb.W * 0.0045));
				ctx.restore();
			}
		if (f.marks)
			for (const [name, col] of f.marks) {
				const c = capByName.get(name);
				if (!c) continue;
				ctx.strokeStyle = col;
				ctx.lineWidth = 2 * px;
				ctx.beginPath();
				ctx.roundRect(c.x - 1.5 * px, c.y - 1.5 * px, c.w + 3 * px, c.h + 3 * px, c.r + 1.5 * px);
				ctx.stroke();
			}
	}

	function draw(ctx: CanvasRenderingContext2D, cam: Camera, dpr: number, f: DeskFrame) {
		const alpha = f.alpha ?? 1;
		if (alpha <= 0.002) return;
		const q = f.quality ?? 0;
		const glow = (f.glow ?? 1) * (q >= 2 ? 0.6 : 1);
		const p = f.pen ?? 1;
		const g = p < 1 ? penGroups(p) : undefined;
		resolve(f);
		ctx.save();
		ctx.globalAlpha = alpha;
		const px = 1 / cam.k; // one CSS pixel, in desk units
		const c = !g && !(cam.rot ?? 0) ? cached(cam.k * dpr, dpr) : null;
		// 1. the mat
		setCam(ctx, cam, dpr);
		if (c) ctx.drawImage(c.under!, B.x, B.y, c.under!.width / c.s, c.under!.height / c.s);
		else drawUnder(ctx, px, g);
		// 2. light spilling onto it
		const anyLit = C.on.some((v) => v > 0.01);
		if (anyLit && glow > 0) {
			paintLight();
			ctx.globalCompositeOperation = 'lighter';
			ctx.globalAlpha = alpha * 0.45 * glow;
			ctx.imageSmoothingEnabled = true;
			ctx.drawImage(half, B.x, B.y, B.w, B.h);
			ctx.globalCompositeOperation = 'source-over';
			ctx.globalAlpha = alpha;
		}
		// 3. the light sources under the bodies: mat edge, underglow, the mouse strip
		const gLit = g ? Math.min(g.mat, g.case) : 1;
		if (gLit > 0) {
			ctx.save();
			ctx.globalAlpha = alpha * fillOf(gLit);
			drawLitUnder(ctx, px);
			ctx.restore();
		}
		// 4. the bodies
		if (c) ctx.drawImage(c.over!, B.x, B.y, c.over!.width / c.s, c.over!.height / c.s);
		else drawOver(ctx, px, g);
		// 5. caps, the wheel, the spiral; then gels and marks
		drawLitOver(ctx, cam, dpr, px, f, g ? g.caps : 1, g ? g.case : 1, g ? g.seams : 1);
		drawGels(ctx, px, f);
		// 6. a faint bloom over everything lit, then words on caps
		if (anyLit && glow > 0 && q < 2) {
			ctx.globalCompositeOperation = 'lighter';
			ctx.globalAlpha = alpha * 0.12 * glow;
			ctx.drawImage(half, B.x, B.y, B.w, B.h);
			ctx.globalCompositeOperation = 'source-over';
			ctx.globalAlpha = alpha;
		}
		drawKeyText(ctx, cam, dpr, f);
		ctx.restore();
	}

	return { geo, draw, toScreen, cap: (name) => capByName.get(name) };
}
