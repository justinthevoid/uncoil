// The landing page's film: one fixed canvas, one set of 113 points (the maintainer's
// real LEDs), moved from scene to scene by scroll and never cut. Every lit colour comes from the desktop app's
// own port of the engine maths; the desk is the app's own device art (../art). Scroll drives where things
// are, through a spring, so the film has weight; the clock drives what colour they are.
//
// DEBUG (motion checks where requestAnimationFrame is throttled, e.g. headless browsers):
//   ?y=2400        render the film as if scrolled to 2400 px (the spring is skipped), and hold it there
//   ?t=6           hold the effect clock at 6 s
//   ?debug         expose window.__uncoil = { at(y, t?), bench(n?, y?) -> ms per frame, release() }
import { leds, devices, frameWith, WAVE, type Effect, type Rgb } from '../lib/desk';
import { deskInputs } from '$uncoil/effect';
import { deskGeometry, type KeyboardGeo } from '../art/geometry';
import { createDeskRenderer, type Camera } from '../art/renderer';

// ---------------------------------------------------------------------------------------- motion tokens
// One rhythm for the whole film: every move is one of these.
const easeInOut = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);
const easeOut = (t: number) => 1 - (1 - t) ** 3;
/** Ease out past the mark and back: a landing. */
const backOut = (t: number, s = 1.5) => 1 + (s + 1) * (t - 1) ** 3 + s * (t - 1) ** 2;
export const MOTION = {
	/** The film follows scroll through a critically damped spring (rad/s): weight on flicks, soft stops. */
	scrollOmega: 7,
	/** The camera's drift toward the pointer, and how far (px). */
	pointerOmega: 2.6,
	parallax: [9, 6] as const,
	/** Share of every move spent staggering along the wave's 35° direction. */
	stagger: 0.35,
	ease: easeInOut,
	out: easeOut,
	land: backOut,
	/** Light trails: share of the trail buffer cleared per frame (lower = longer streaks). */
	trailFade: 0.24,
	/** Copy: data types on at this pace; prose arrives with a quiet fade and lift (CSS uses the same). */
	typeMs: 32,
	revealMs: 760,
};

// ---------------------------------------------------------------------------------------------------- maths
type V = Camera;
type Box = { x0: number; y0: number; w: number; h: number };
type R = { x: number; y: number; w: number; h: number };

const clamp01 = (x: number) => (x < 0 ? 0 : x > 1 ? 1 : x);
const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const smooth = (a: number, b: number, x: number) => {
	const t = clamp01((x - a) / (b - a));
	return t * t * (3 - 2 * t);
};
const ease = MOTION.ease;
const bump = (x: number, a: number, b: number, c: number, d: number) => smooth(a, b, x) * (1 - smooth(c, d, x));

function rng(seed: number) {
	let a = seed >>> 0;
	return () => {
		a = (a + 0x6d2b79f5) | 0;
		let t = Math.imul(a ^ (a >>> 15), 1 | a);
		t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

/** Fit a desk-unit box into a screen rect. */
function fit(r: R, b: Box, pad = 0.94): V {
	const k = Math.max(0.01, Math.min(r.w / b.w, r.h / b.h) * pad);
	return { sx: r.x + r.w / 2, sy: r.y + r.h / 2, cx: b.x0 + b.w / 2, cy: b.y0 + b.h / 2, k };
}
const toX = (v: V, x: number) => v.sx + (x - v.cx) * v.k;
const toY = (v: V, y: number) => v.sy + (y - v.cy) * v.k;

/** A critically damped spring toward a target: x, v updated in place. */
function spring(s: { x: number; v: number }, target: number, omega: number, dt: number) {
	const d = s.x - target;
	const a = -2 * omega * s.v - omega * omega * d;
	s.v += a * dt;
	s.x += s.v * dt;
}

// ------------------------------------------------------------------------------------------------ the desk
const N = leds.length;
const BRASS: Rgb = [201, 164, 106];
const A35 = (35 * Math.PI) / 180;
const AX = Math.cos(A35);
const AY = Math.sin(A35);
const NX = -AY;
const NY = AX;

const GEO = deskGeometry(devices);
const kbGeo = GEO.devices.find((d): d is KeyboardGeo => d.kind === 'keyboard')!;
const kbDev = devices.find((d) => d.kind === 'keyboard')!;
const mouseDev = devices.find((d) => d.kind === 'mouse')!;
const DESK: Box = { x0: GEO.bounds.x, y0: GEO.bounds.y, w: GEO.bounds.w, h: GEO.bounds.h };
const KB: Box = { x0: kbGeo.box.x - 0.25, y0: kbGeo.box.y - 0.25, w: kbGeo.box.w + 0.5, h: kbGeo.box.h + 0.5 };
const INPUTS = deskInputs(devices);
const ANCHOR = GEO.anchors;

const proj = new Float32Array(N);
const qn = new Float32Array(N);
leds.forEach((l, i) => {
	proj[i] = l.x * AX + l.y * AY;
	qn[i] = ANCHOR[i * 2] * NX + ANCHOR[i * 2 + 1] * NY;
});
const projMin = Math.min(...proj);
const projMax = Math.max(...proj);
const span = projMax - projMin;
/** Where each LED lands along the wave's axis: its drawn place, projected. */
const projA = new Float32Array(N);
for (let i = 0; i < N; i++) projA[i] = ANCHOR[i * 2] * AX + ANCHOR[i * 2 + 1] * AY;
const qMid = (Math.min(...qn) + Math.max(...qn)) / 2;
/** Order along the wave's direction: 0 = first lit. */
const rankAsc = new Float32Array(N);
[...leds.keys()].sort((a, b) => proj[a] - proj[b]).forEach((i, r) => (rankAsc[i] = r / (N - 1)));
/** Position on the coil (0 = the inner end). The coil is the wave's axis, wound up: the last LED the wave reaches
 *  sits at the centre, so the outer end unrolls first, from the desk's top-left corner. */
const sigRank = new Float32Array(N);
const sigProj = new Float32Array(N);
const aMin = Math.min(...projA);
const aSpan = Math.max(...projA) - aMin;
for (let i = 0; i < N; i++) {
	sigRank[i] = 1 - rankAsc[i];
	sigProj[i] = 1 - (projA[i] - aMin) / aSpan;
}
const isKey = leds.map((l) => l.kind === 'keyboard' && l.key);
const isMat = leds.map((l) => l.kind === 'mousemat');
const kindOf = leds.map((l) => l.kind);
/** When the pen reaches each LED's part of the drawing (renderer pen order: mat, case, frames, caps, mouse, seams). */
const penAt = new Float32Array(N);
for (let i = 0; i < N; i++) {
	if (isMat[i]) penAt[i] = 0.12;
	else if (kindOf[i] === 'mouse') penAt[i] = 0.82;
	else if (!isKey[i]) penAt[i] = 0.3;
	else penAt[i] = 0.43 + 0.24 * ((ANCHOR[i * 2] - kbGeo.box.x) / kbGeo.box.w);
}

function deskR(i: number, k: number) {
	return kindOf[i] === 'mouse' ? 0.09 * k : isKey[i] ? 0.08 * k : 0.07 * k;
}
function deskG(i: number, k: number) {
	return isMat[i] ? 0 : kindOf[i] === 'mouse' ? 0.6 * k : isKey[i] ? 0.6 * k : 0.8 * k;
}
/** Where a point rests: where its LED is drawn. */
function deskXY(i: number, v: V): [number, number] {
	return [toX(v, ANCHOR[i * 2]), toY(v, ANCHOR[i * 2 + 1])];
}

// The engine's own effects. Per-device: the desk keeps the wave, the keyboard burns, the mouse twinkles.
const STUDIO: Effect = {
	kind: 'studio',
	layers: [
		{ name: 'Desk', enabled: true, opacity: 1, effect: WAVE as never, mask: { kind: 'all' } },
		{ name: 'Keyboard', enabled: true, opacity: 1, effect: { kind: 'fire', speed: 1, height: 0.8 }, mask: { kind: 'devices', ids: [kbDev.id] } },
		{ name: 'Mouse', enabled: true, opacity: 1, effect: { kind: 'static', color: [0, 0, 0] }, mask: { kind: 'devices', ids: [mouseDev.id] } },
		{
			name: 'Mouse stars',
			enabled: true,
			opacity: 1,
			effect: { kind: 'starlight', colors: [[255, 255, 255], [120, 180, 255]], density: 0.45, twinkle_s: 1.6 },
			mask: { kind: 'devices', ids: [mouseDev.id] },
		},
	],
} as Effect;

// The keyboard's own Fn layer (docs/PROTOCOL.md), gel by gel.
const FN_GELS: [string, string, string][] = [
	['Escape', 'Eco', '#5b84e0'],
	['F9', 'Macro', '#e0a33e'],
	['F10', 'Game', '#e0a33e'],
	['F11', 'Lit −', '#cf5aa0'],
	['F12', 'Lit +', '#cf5aa0'],
	['Delete', 'Sleep', '#5b84e0'],
	['P', 'PrtSc', '#58b06c'],
];

// ------------------------------------------------------------------------------------------------ the mark
// The uncoil mark, M12 12 a2 2 0 1 1 2-2 4 4 0 1 1-4-4 6 6 0 1 1-6 6 8 8 0 1 1 8 8: four 270° arcs.
const MARK_D = 'M12 12a2 2 0 1 1 2-2 4 4 0 1 1-4-4 6 6 0 1 1-6 6 8 8 0 1 1 8 8';
const ARCS = [
	{ cx: 12, cy: 10, r: 2, a0: Math.PI / 2 },
	{ cx: 10, cy: 10, r: 4, a0: 0 },
	{ cx: 10, cy: 12, r: 6, a0: 1.5 * Math.PI },
	{ cx: 12, cy: 12, r: 8, a0: Math.PI },
];
const SWEEP = 1.5 * Math.PI;
const LM = ARCS.reduce((n, a) => n + a.r * SWEEP, 0);
function markAt(sm: number): [number, number] {
	for (const a of ARCS) {
		const len = a.r * SWEEP;
		if (sm <= len) {
			const ang = a.a0 + sm / a.r;
			return [a.cx + a.r * Math.cos(ang), a.cy + a.r * Math.sin(ang)];
		}
		sm -= len;
	}
	return [12, 20];
}
function curvAt(sm: number) {
	for (const a of ARCS) {
		const len = a.r * SWEEP;
		if (sm <= len) return 1 / a.r;
		sm -= len;
	}
	return 1 / 8;
}

// The roll: the laid part of the coil lies on the local x axis; the rest, still wound, rests on it and is
// integrated backwards from the peel point with the mark's own curvature, so the shape is exactly the mark.
const ROLL_STEPS = 360;
const polyX = new Float32Array(ROLL_STEPS + 1);
const polyY = new Float32Array(ROLL_STEPS + 1);
let polyN = 0;
let polyDs = 1;
let polyPeel = 0;
function roll(L: number, kk: number, sPeel: number) {
	const M = Math.max(6, Math.ceil((ROLL_STEPS * sPeel) / Math.max(L, 1)));
	const ds = sPeel / M;
	let x = L - sPeel;
	let y = 0;
	let th = Math.PI;
	polyX[0] = x;
	polyY[0] = y;
	for (let j = 1; j <= M; j++) {
		const kap = curvAt((sPeel - (j - 0.5) * ds) / kk) / kk;
		const mid = th - (kap * ds) / 2;
		x -= ds * Math.cos(mid);
		y -= ds * Math.sin(mid);
		th -= kap * ds;
		polyX[j] = x;
		polyY[j] = y;
	}
	polyN = M;
	polyDs = ds;
	polyPeel = sPeel;
}
function rollAt(s: number): [number, number] {
	const f = Math.min(polyN, Math.max(0, (polyPeel - s) / polyDs));
	const j = Math.min(polyN - 1, Math.floor(f));
	const u = f - j;
	return [lerp(polyX[j], polyX[j + 1], u), lerp(polyY[j], polyY[j + 1], u)];
}

// ------------------------------------------------------------------------------------------------ state
class St {
	// per point: where it is, how big, how visible, brass -> colour, the field's colour, the LED's light in the
	// drawing, and the wave's bright leading edge
	x = new Float32Array(N);
	y = new Float32Array(N);
	r = new Float32Array(N);
	g = new Float32Array(N);
	a = new Float32Array(N);
	lit = new Float32Array(N);
	here = new Float32Array(N);
	light = new Float32Array(N);
	edge = new Float32Array(N);
	view: V = { sx: 0, sy: 0, cx: 0, cy: 0, k: 1 };
	eff: Effect = WAVE;
	effB: Effect | null = null;
	mix = 0;
	val = 1;
	/** The device art: opacity and how much of its line art is drawn. */
	art = 0;
	pen = 1;
	fn = 0;
	legends = 0;
	field = 0;
	shed = 0;
	shedP = 0;
	shedRect: R = { x: 0, y: 0, w: 1, h: 1 };
	brain = 0;
	cable = 0;
	node = 0;
	press = 0;
	bytes = 0;
	brainRect: R = { x: 0, y: 0, w: 1, h: 1 };
	strip = 0;
	hi = 0;
	mark = 0;
	markRect: R = { x: 0, y: 0, w: 1, h: 1 };
	path = 0;
	pathT: [number, number, number] = [0, 0, 0];
	scale = 0;
	lamp: [number, number, number] = [0, 0, 1];
	swirl = false;
	reset() {
		this.effB = null;
		this.mix = 0;
		this.val = 1;
		this.art = this.fn = this.legends = this.field = this.shed = this.brain = 0;
		this.pen = 1;
		this.cable = this.node = this.press = this.bytes = this.strip = this.hi = this.mark = this.path = this.scale = 0;
		this.swirl = false;
		this.here.fill(0);
		this.light.fill(0);
		this.edge.fill(0);
		this.a.fill(1);
	}
}

// ------------------------------------------------------------------------------------------------ the film
export function start() {
	const canvas = document.getElementById('film') as HTMLCanvasElement | null;
	const ctx = canvas?.getContext('2d');
	if (!canvas || !ctx) return;
	const root = document.documentElement;
	const reduce = matchMedia('(prefers-reduced-motion: reduce)');
	let RM = reduce.matches;
	const desk = createDeskRenderer(GEO);

	// Seeded variety: ?seed=N replays the same tangle, scatter and clock.
	const params = new URLSearchParams(location.search);
	const seedParam = Number.parseInt(params.get('seed') ?? '', 10);
	const seed = Number.isFinite(seedParam) && seedParam > 0 ? seedParam % 100000 : 1 + Math.floor(Math.random() * 9999);
	const rand = rng(seed);
	for (const a of document.querySelectorAll<HTMLAnchorElement>('[data-seed]')) {
		a.href = `?seed=${seed}`;
		a.textContent = `seed ${seed}`;
	}
	const forcedY = params.has('y') ? Number(params.get('y')) : null;
	const forcedT = params.has('t') ? Number(params.get('t')) : null;

	// Seventeen coils for seventeen processes, and the order they fall in.
	const coils = Array.from({ length: 17 }, (_, j) => {
		const ang = j * 2.39996 + rand() * 0.6;
		const rad = Math.sqrt((j + 0.5) / 17);
		return { x: Math.cos(ang) * rad, y: Math.sin(ang) * rad, s: 0.13 + rand() * 0.12, rot: rand() * Math.PI * 2, spin: (rand() - 0.5) * 0.25, fall: 0 };
	});
	coils
		.map((c, j) => ({ j, r: rand() }))
		.sort((a, b) => a.r - b.r)
		.forEach((o, n) => (coils[o.j].fall = 0.04 + (0.8 * n) / 16));
	const scatterU = Float32Array.from({ length: N }, () => rand());
	const scatterV = Float32Array.from({ length: N }, () => rand());
	const t0 = (seed % 140) / 10;

	const COIL = new Path2D(MARK_D);
	const MONO = 'ui-monospace, "Cascadia Mono", "Segoe UI Mono", Consolas, monospace';

	// Quality: tier 0 full, 1 lighter, 2 lightest. Steps down on slow hardware or a slow frame rate.
	const nav = navigator as Navigator & { deviceMemory?: number; connection?: { saveData?: boolean } };
	let tier = (navigator.hardwareConcurrency ?? 8) <= 4 || (nav.deviceMemory ?? 8) <= 4 || nav.connection?.saveData ? 1 : 0;
	if (innerWidth < 700 && tier === 0) tier = 1;

	// ---------------------------------------------------------------------------------------- measurement
	const sceneEls = [...document.querySelectorAll<HTMLElement>('[data-scene]')];
	type Sc = { kind: string; el: HTMLElement; pin: HTMLElement | null; slot: R; top: number; height: number; keys: number };
	const KEYS: Record<string, number> = { hero: 1, shed: 1, field: 2, brain: 2, written: 2, install: 1 };
	/** Reduced motion shows one still per beat; a scene with one beat shows this moment of it. */
	const RM_ONE: Record<string, number> = { hero: 1, shed: 0, install: 1 };
	const scenes: Sc[] = sceneEls.map((el) => ({
		kind: el.dataset.scene!,
		el,
		pin: el.querySelector<HTMLElement>('.pin'),
		slot: { x: 0, y: 0, w: 1, h: 1 },
		top: 0,
		height: 1,
		keys: KEYS[el.dataset.scene!] ?? 1,
	}));
	const idx = (k: string) => scenes.findIndex((s) => s.kind === k);
	const S = { hero: idx('hero'), shed: idx('shed'), field: idx('field'), brain: idx('brain'), written: idx('written'), install: idx('install') };
	const stripEl = document.querySelector<HTMLElement>('[data-strip]');
	const cellEls = stripEl ? [...stripEl.querySelectorAll<HTMLElement>('.cell')] : [];
	const cellOff: R[] = cellEls.map(() => ({ x: 0, y: 0, w: 0, h: 0 }));
	const barEls: Record<string, HTMLElement | null> = {
		keyboard: document.querySelector('[data-bar="keyboard"]'),
		mouse: document.querySelector('[data-bar="mouse"]'),
		mousemat: document.querySelector('[data-bar="mousemat"]'),
	};
	const markEl = document.querySelector<HTMLElement>('[data-mark-slot]');
	const pins = { hero: scenes[S.hero]?.pin ?? null, shed: scenes[S.shed]?.pin ?? null, written: scenes[S.written]?.pin ?? null };
	let W = innerWidth;
	let H = innerHeight;
	let dpr = 1;
	let cell = 12;
	let cols = 1;
	let rows = 1;
	const fieldCv = document.createElement('canvas');
	const fctx = fieldCv.getContext('2d')!;
	const layerCv = document.createElement('canvas');
	const lctx = layerCv.getContext('2d')!;
	const trailCv = document.createElement('canvas');
	const tctx = trailCv.getContext('2d')!;
	let fieldImg: ImageData | null = null;
	let dots: CanvasPattern | null = null;
	// Within-device order for the table bars: along the wave, so each bar is a slice of the gradient.
	const barSlot = new Int16Array(N);
	const barCount: Record<string, number> = { keyboard: 0, mouse: 0, mousemat: 0 };
	[...leds.keys()].sort((a, b) => proj[a] - proj[b]).forEach((i) => (barSlot[i] = barCount[kindOf[i]]++));
	const BAR_ROWS = 3;
	const kbCols = Math.ceil(barCount.keyboard / BAR_ROWS);

	const relTo = (el: Element, base: DOMRect): R => {
		const r = el.getBoundingClientRect();
		return { x: r.left - base.left, y: r.top - base.top, w: r.width, h: r.height };
	};

	function measure() {
		W = innerWidth;
		H = innerHeight;
		const sy = scrollY;
		for (const sc of scenes) {
			const r = sc.el.getBoundingClientRect();
			sc.top = r.top + sy;
			sc.height = r.height;
			const slotEl = sc.el.querySelector('[data-slot]');
			if (sc.pin && slotEl) sc.slot = relTo(slotEl, sc.pin.getBoundingClientRect());
		}
		if (stripEl) {
			const gr = stripEl.getBoundingClientRect();
			cellEls.forEach((c, i) => (cellOff[i] = relTo(c, gr)));
		}
		resize();
	}

	function resize() {
		const cap = tier === 0 ? 2 : tier === 1 ? 1.5 : 1;
		dpr = Math.min(cap, devicePixelRatio || 1);
		canvas!.width = trailCv.width = Math.round(W * dpr);
		canvas!.height = trailCv.height = Math.round(H * dpr);
		cell = (W < 700 ? 14 : 12) + tier * 5;
		cols = Math.ceil(W / cell);
		rows = Math.ceil(H / cell);
		fieldCv.width = cols;
		fieldCv.height = rows;
		fieldImg = fctx.createImageData(cols, rows);
		layerCv.width = cols * cell;
		layerCv.height = rows * cell;
		const tile = document.createElement('canvas');
		tile.width = tile.height = cell;
		const tc = tile.getContext('2d')!;
		tc.fillStyle = '#fff';
		tc.beginPath();
		tc.arc(cell / 2, cell / 2, cell * 0.3, 0, Math.PI * 2);
		tc.fill();
		dots = lctx.createPattern(tile, 'repeat');
		dirty = true;
	}

	// ------------------------------------------------------------------------------------------ scenes
	const slotOf = (s: number): R => scenes[s]?.slot ?? { x: 0, y: 0, w: W, h: H };

	function heroState(p: number, st: St) {
		const sl = slotOf(S.hero);
		const v = fit(sl, DESK, 0.97);
		// a slow push in as the light arrives
		v.k *= 1 + 0.04 * smooth(0.7, 1, p);
		st.view = v;
		st.eff = WAVE;
		const ccx = sl.x + sl.w / 2;
		const ccy = sl.y + sl.h / 2;
		// Tension, then release: the coil winds a touch tighter before it lets go.
		const tight = smooth(0, 0.045, p) * (1 - smooth(0.05, 0.1, p));
		const k0 = ((Math.min(sl.w, sl.h) * 0.86) / 16) * (1 - 0.07 * tight);
		const twist = -0.32 * tight;
		const uRaw = (p - 0.06) / 0.48;
		const u = clamp01(uRaw);
		const e = smooth(0, 0.4, u);
		const k1 = (aSpan * v.k) / LM;
		const kk = lerp(k0, k1, e);
		const L = LM * kk;
		const sPeel = L * (1 - u);
		roll(L, kk, sPeel);
		const ox = toX(v, 0);
		const oy = toY(v, 0);
		const startX = ox + (aMin * AX + qMid * NX) * v.k;
		const startY = oy + (aMin * AY + qMid * NY) * v.k;
		// the coil's centre at rest is 8 mark units above its outer end
		const tx = lerp(ccx, startX, e);
		const ty = lerp(ccy + 8 * k0, startY, e);
		const th = A35 * e + twist;
		const c = Math.cos(th);
		const s = Math.sin(th);
		st.pathT = [tx, ty, th];
		const glint = ((T * 0.2) % 1.6) - 0.3;
		const coilR = Math.max(1.6, k0 * 0.12);
		const pen = smooth(0.28, 0.8, p);
		st.pen = pen;
		const w = projMin - 3 + (span + 6) * clamp01((p - 0.7) / 0.24);
		for (let i = 0; i < N; i++) {
			const sig = lerp(sigRank[i], sigProj[i], e);
			const si = sig * L;
			let lx: number;
			let ly: number;
			let fanT = 0;
			let fan = 0;
			if (si >= sPeel) {
				// laid down: drop to its LED, past it, and settle
				fanT = clamp01((uRaw - (1 - sig) - 0.02) / 0.32);
				fan = MOTION.land(fanT);
				lx = L - si;
				ly = (qn[i] - qMid) * v.k * fan;
			} else {
				[lx, ly] = rollAt(si);
			}
			const x = tx + c * lx - s * ly;
			const y = ty + s * lx + c * ly;
			// The roll's frame converges on the desk exactly (line = the LEDs' drawn places along 35°), so a laid
			// point at fan 1 sits on its LED; past 1 it has overshot and is on its way back.
			st.x[i] = x;
			st.y[i] = y;
			st.r[i] = lerp(coilR, deskR(i, v.k), fanT);
			st.g[i] = lerp(coilR * 3.2, deskG(i, v.k), fanT);
			const gd = (1 - sigRank[i] - glint) * 9;
			const gl = Math.exp(-(gd * gd));
			const landed = smooth(0.78, 1, fanT);
			const shown = smooth(penAt[i], penAt[i] + 0.07, pen);
			st.a[i] = (0.62 + 0.38 * gl * (1 - u)) * (1 - landed * shown);
			const ahead = w - proj[i];
			st.lit[i] = easeInOut(clamp01(ahead / 3));
			st.edge[i] = bump(ahead, -0.4, 0.4, 0.9, 3.2);
			// the drawing's LED: dim brass where a point has landed, then the wave's colour
			st.light[i] = landed * shown * (0.16 + 0.84 * st.lit[i]);
		}
		st.path = 0.55 * (1 - smooth(0.9, 1.25, uRaw));
		st.art = 1;
		st.lamp = [sl.x + sl.w * 0.55, sl.y + sl.h * 0.3, Math.max(W, H) * 0.7];
	}

	function deskInto(st: St, v: V) {
		for (let i = 0; i < N; i++) {
			const [x, y] = deskXY(i, v);
			st.x[i] = x;
			st.y[i] = y;
			st.r[i] = deskR(i, v.k);
			st.g[i] = deskG(i, v.k);
			st.a[i] = 0;
			st.lit[i] = 1;
			st.light[i] = 1;
		}
		st.art = 1;
		st.pen = 1;
	}

	function shedState(p: number, st: St) {
		const sl = slotOf(S.shed);
		st.view = fit(sl, DESK, 0.9);
		const ly = sl.y + sl.h * 0.86;
		const rr = W < 700 ? 1.4 : 1.8;
		for (let i = 0; i < N; i++) {
			st.x[i] = sl.x + sl.w * (0.06 + 0.88 * rankAsc[i]);
			st.y[i] = ly;
			st.r[i] = rr;
			st.g[i] = rr * 4;
			st.lit[i] = 1;
		}
		st.pen = 0;
		st.shed = 1;
		st.shedP = p;
		st.shedRect = sl;
		st.lamp = [sl.x + sl.w * 0.5, sl.y + sl.h * 0.4, Math.max(W, H) * 0.6];
	}

	function fieldState(p: number, st: St) {
		const sl = slotOf(S.field);
		const v = fit(sl, DESK, 0.96);
		st.view = v;
		const back = smooth(0.04, 0.45, p);
		for (let i = 0; i < N; i++) {
			const t = easeInOut(clamp01((back - rankAsc[i] * 0.4) / 0.6));
			const fx = (Math.floor(scatterU[i] * cols) + 0.5) * cell;
			const fy = (Math.floor(scatterV[i] * rows) + 0.5) * cell;
			const [dx, dy] = deskXY(i, v);
			st.x[i] = lerp(fx, dx, t);
			st.y[i] = lerp(fy, dy, t);
			st.r[i] = lerp(cell * 0.32, deskR(i, v.k), t);
			st.g[i] = lerp(cell * 1.2, deskG(i, v.k), t);
			const landed = smooth(0.8, 1, t);
			st.a[i] = 1 - landed;
			st.lit[i] = 1;
			st.here[i] = 1 - t;
			st.light[i] = landed;
		}
		st.field = 1 - back;
		st.art = back;
		st.pen = 1;
		// two beats: the field, then the desk with its own effect per device (a pure Studio state at rest, so the
		// move into the keyboard crossfades cleanly)
		const studio = smooth(0.55, 0.82, p);
		if (studio >= 0.999) st.eff = STUDIO;
		else {
			st.eff = WAVE;
			st.effB = STUDIO;
			st.mix = studio;
		}
		st.scale = bump(p, 0.3, 0.42, 0.55, 0.68);
		st.lamp = [sl.x + sl.w * 0.5, sl.y + sl.h * 0.45, Math.max(W, H) * 0.65];
	}

	function brainState(p: number, st: St) {
		const sl = slotOf(S.brain);
		const v = fit(sl, KB, 0.96);
		st.view = v;
		deskInto(st, v);
		st.val = 0.32;
		st.fn = 1;
		st.legends = 1;
		st.brain = 1;
		st.cable = smooth(0.12, 0.42, p) * (1 - smooth(0.58, 0.84, p));
		st.node = 1 - 0.62 * smooth(0.62, 0.86, p);
		st.press = smooth(0.72, 0.95, p);
		st.bytes = bump(p, 0.34, 0.44, 0.56, 0.62);
		st.brainRect = sl;
		st.lamp = [sl.x + sl.w * 0.5, sl.y + sl.h * 0.5, Math.max(W, H) * 0.6];
	}

	let stripRect: DOMRect | null = null;
	function writtenState(p: number, st: St) {
		const sl = slotOf(S.written);
		st.view = fit(sl, KB, 0.96);
		const gr = stripRect ?? stripEl?.getBoundingClientRect() ?? null;
		const toTable = smooth(0.12, 0.5, p);
		const bars: Record<string, DOMRect | null> = { keyboard: null, mouse: null, mousemat: null };
		if (toTable > 0) for (const k in barEls) bars[k] = barEls[k]?.getBoundingClientRect() ?? null;
		const nc = cellOff.length || 1;
		for (let i = 0; i < N; i++) {
			// Strip: 113 points into 90 bytes, in wave order.
			const ci = Math.min(nc - 1, Math.floor(rankAsc[i] * nc * 0.9999));
			const co = cellOff[ci];
			const sx = gr && co ? gr.left + co.x + co.w / 2 : sl.x + sl.w * rankAsc[i];
			const sy = gr && co ? gr.top + co.y + co.h / 2 : sl.y + sl.h / 2;
			let x = sx;
			let y = sy;
			const t = easeInOut(clamp01((toTable - rankAsc[i] * 0.3) / 0.7));
			const bar = bars[kindOf[i]];
			let br = 2;
			if (t > 0 && bar) {
				const pitch = bar.width / (kindOf[i] === 'keyboard' ? kbCols : Math.max(1, Math.ceil(barCount[kindOf[i]] / BAR_ROWS)));
				const rowsHere = Math.min(BAR_ROWS, barCount[kindOf[i]]);
				const col = Math.floor(barSlot[i] / rowsHere);
				const row = barSlot[i] % rowsHere;
				x = lerp(sx, bar.left + (col + 0.5) * pitch, t);
				y = lerp(sy, bar.top + ((row + 0.5) * bar.height) / BAR_ROWS, t);
				br = Math.min(pitch, bar.height / BAR_ROWS) * 0.32;
			}
			st.x[i] = x;
			st.y[i] = y;
			st.r[i] = lerp(co ? co.w * 0.18 : 2, br, t);
			st.g[i] = lerp(co ? co.w * 0.5 : 6, br * 3, t);
			st.a[i] = t;
			st.lit[i] = t;
		}
		st.pen = 0;
		st.strip = 1 - toTable;
		st.hi = bump(p, -0.1, 0, 0.08, 0.16);
		st.lamp = [sl.x + sl.w * 0.5, sl.y + sl.h * 0.4, Math.max(W, H) * 0.6];
	}

	function installState(_p: number, st: St) {
		const r = markEl?.getBoundingClientRect();
		const mr: R = r ? { x: r.left, y: r.top, w: r.width, h: r.height } : { x: W * 0.1, y: H * 0.3, w: 160, h: 160 };
		st.view = fit(mr, DESK, 0.9);
		const size = Math.min(mr.w, mr.h);
		const k = (size * 0.9) / 16;
		const ox = mr.x + mr.w / 2 - 12 * k;
		const oy = mr.y + mr.h / 2 - 12 * k;
		for (let i = 0; i < N; i++) {
			const [mx, my] = markAt(sigRank[i] * LM);
			st.x[i] = ox + mx * k;
			st.y[i] = oy + my * k;
			st.r[i] = Math.max(1.3, size * 0.011);
			st.g[i] = Math.max(4, size * 0.04);
			st.lit[i] = 1;
		}
		st.pen = 0;
		st.mark = 1;
		st.markRect = mr;
		st.swirl = true;
		st.lamp = [mr.x + mr.w / 2, mr.y + mr.h / 2, size * 2.6];
	}

	const STATE: ((p: number, st: St) => void)[] = [];
	STATE[S.hero] = heroState;
	STATE[S.shed] = shedState;
	STATE[S.field] = fieldState;
	STATE[S.brain] = brainState;
	STATE[S.written] = writtenState;
	STATE[S.install] = installState;

	function stateOf(s: number, p: number, st: St) {
		st.reset();
		STATE[s]?.(p, st);
	}

	const A = new St();
	const B = new St();
	const O = new St();

	const PER = ['r', 'g', 'a', 'lit', 'here', 'light', 'edge'] as const;
	const SCALARS = ['art', 'pen', 'fn', 'legends', 'field', 'shed', 'brain', 'cable', 'node', 'press', 'bytes', 'strip', 'hi', 'mark', 'path', 'scale'] as const;
	function blend(a: St, b: St, t: number, o: St) {
		const sw = b.swirl;
		const mx = b.markRect.x + b.markRect.w / 2;
		const my = b.markRect.y + b.markRect.h / 2;
		const st = MOTION.stagger;
		for (let i = 0; i < N; i++) {
			const ti = ease(clamp01((t - rankAsc[i] * st) / (1 - st)));
			if (sw) {
				const ra = Math.hypot(a.x[i] - mx, a.y[i] - my);
				const rb = Math.hypot(b.x[i] - mx, b.y[i] - my);
				const ta = Math.atan2(a.y[i] - my, a.x[i] - mx);
				const tb = Math.atan2(b.y[i] - my, b.x[i] - mx);
				const d = ((((tb - ta) % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI)) + 2 * Math.PI;
				const ang = ta + d * ti;
				const rad = lerp(ra, rb, ti);
				o.x[i] = mx + rad * Math.cos(ang);
				o.y[i] = my + rad * Math.sin(ang);
			} else {
				o.x[i] = lerp(a.x[i], b.x[i], ti);
				o.y[i] = lerp(a.y[i], b.y[i], ti);
			}
			for (const k of PER) o[k][i] = lerp(a[k][i], b[k][i], ti);
		}
		const e = ease(t);
		o.view = {
			sx: lerp(a.view.sx, b.view.sx, e),
			sy: lerp(a.view.sy, b.view.sy, e),
			cx: lerp(a.view.cx, b.view.cx, e),
			cy: lerp(a.view.cy, b.view.cy, e),
			k: Math.exp(lerp(Math.log(a.view.k), Math.log(b.view.k), e)),
		};
		if (a === b || (a.effB && t < 0.5)) {
			o.eff = a.eff;
			o.effB = a.effB;
			o.mix = a.mix;
		} else if (b.effB) {
			o.eff = b.eff;
			o.effB = b.effB;
			o.mix = b.mix;
		} else if (a.eff === b.eff) {
			o.eff = a.eff;
			o.effB = null;
			o.mix = 0;
		} else {
			o.eff = a.eff;
			o.effB = b.eff;
			o.mix = e;
		}
		o.val = lerp(a.val, b.val, e);
		for (const k of SCALARS) o[k] = lerp(a[k], b[k], e);
		o.shedP = a.shed > 0 ? a.shedP : b.shedP;
		o.shedRect = a.shed > 0 ? a.shedRect : b.shedRect;
		o.brainRect = a.brain > 0 ? a.brainRect : b.brainRect;
		o.markRect = b.mark > 0 ? b.markRect : a.markRect;
		o.pathT = a.path > 0 ? a.pathT : b.pathT;
		o.lamp = [lerp(a.lamp[0], b.lamp[0], e), lerp(a.lamp[1], b.lamp[1], e), lerp(a.lamp[2], b.lamp[2], e)];
	}

	// ------------------------------------------------------------------------------------------ scroll
	function locate(y: number): { s: number; p: number; tau: number } {
		for (let s = 0; s < scenes.length; s++) {
			const sc = scenes[s];
			if (s === scenes.length - 1) return { s, p: 1, tau: -1 };
			const holdLen = Math.max(1, sc.height - H);
			const holdEnd = sc.top + holdLen;
			if (y < holdEnd) return { s, p: clamp01((y - sc.top) / holdLen), tau: -1 };
			const next = scenes[s + 1];
			if (y < next.top) return { s, p: 1, tau: clamp01((y - holdEnd) / Math.max(1, next.top - holdEnd)) };
		}
		return { s: 0, p: 0, tau: -1 };
	}

	// ------------------------------------------------------------------------------------------ drawing
	const CR = new Float32Array(N);
	const CG = new Float32Array(N);
	const CB = new Float32Array(N);
	const artRgb = new Float32Array(N * 3);
	const rgba = (r: number, g: number, b: number, a: number) => `rgba(${r | 0},${g | 0},${b | 0},${a.toFixed(3)})`;

	function colours(o: St) {
		const sA = frameWith(o.eff, T, 1, o.val, INPUTS);
		const sB = o.effB && o.mix > 0 ? frameWith(o.effB, T, 1, o.val, INPUTS) : null;
		const fA = frameWith(WAVE, T, 1, o.val, INPUTS);
		const v = o.view;
		for (let i = 0; i < N; i++) {
			const l = leds[i];
			let c = sA(l.device, l.name, l.x, l.y);
			if (sB) {
				const d = sB(l.device, l.name, l.x, l.y);
				const m = o.mix;
				c = [lerp(c[0], d[0], m), lerp(c[1], d[1], m), lerp(c[2], d[2], m)];
			}
			if (o.here[i] > 0) {
				const h = fA('', '', (o.x[i] - v.sx) / v.k + v.cx, (o.y[i] - v.sy) / v.k + v.cy);
				const m = o.here[i];
				c = [lerp(c[0], h[0], m), lerp(c[1], h[1], m), lerp(c[2], h[2], m)];
			}
			const lt = o.lit[i];
			// the wave's leading edge burns a little brighter as it arrives
			const ed = o.edge[i] * 0.45;
			CR[i] = lerp(lerp(BRASS[0], c[0], lt), 255, ed);
			CG[i] = lerp(lerp(BRASS[1], c[1], lt), 255, ed);
			CB[i] = lerp(lerp(BRASS[2], c[2], lt), 255, ed);
			artRgb[i * 3] = CR[i];
			artRgb[i * 3 + 1] = CG[i];
			artRgb[i * 3 + 2] = CB[i];
		}
	}

	/** The brass lamp: a composited layer, moved by transform (no full-screen canvas pass). */
	const lampEl = document.querySelector<HTMLElement>('.lamp');
	let lampKey = '';
	function placeLamp(o: St) {
		if (!lampEl) return;
		const [x, y, r] = o.lamp;
		const key = `${x | 0},${y | 0},${r | 0}`;
		if (key === lampKey) return;
		lampKey = key;
		lampEl.style.transform = `translate(${(x - 500).toFixed(1)}px, ${(y - 500).toFixed(1)}px) scale(${(r / 500).toFixed(3)})`;
	}

	function drawField(o: St) {
		if (!fieldImg || !dots) return;
		const v = o.view;
		const cxs = toX(v, DESK.x0 + DESK.w / 2);
		const cys = toY(v, DESK.y0 + DESK.h / 2);
		const far = Math.hypot(Math.max(cxs, W - cxs), Math.max(cys, H - cys)) + cell * 8;
		const R = o.field * far;
		const soft = cell * 7;
		const sA = frameWith(WAVE, T, 1, o.val, INPUTS);
		const d = fieldImg.data;
		for (let j = 0; j < rows; j++) {
			const py = (j + 0.5) * cell;
			const dy = (py - v.sy) / v.k + v.cy;
			for (let i = 0; i < cols; i++) {
				const px = (i + 0.5) * cell;
				const m = clamp01((R - Math.hypot(px - cxs, py - cys)) / soft);
				const n = (j * cols + i) * 4;
				if (m <= 0) {
					d[n + 3] = 0;
					continue;
				}
				const c = sA('', '', (px - v.sx) / v.k + v.cx, dy);
				d[n] = c[0];
				d[n + 1] = c[1];
				d[n + 2] = c[2];
				d[n + 3] = 150 * m;
			}
		}
		fctx.putImageData(fieldImg, 0, 0);
		lctx.globalCompositeOperation = 'copy';
		lctx.imageSmoothingEnabled = false;
		lctx.drawImage(fieldCv, 0, 0, cols * cell, rows * cell);
		lctx.globalCompositeOperation = 'destination-in';
		lctx.fillStyle = dots;
		lctx.fillRect(0, 0, cols * cell, rows * cell);
		ctx!.globalCompositeOperation = 'lighter';
		ctx!.drawImage(layerCv, 0, 0, cols * cell, rows * cell);
		ctx!.globalCompositeOperation = 'source-over';
	}

	function drawCoils(o: St) {
		const r = o.shedRect;
		const m = Math.min(r.w, r.h);
		ctx!.lineCap = 'round';
		for (const c of coils) {
			const f = clamp01((o.shedP - c.fall) / 0.12);
			if (f >= 1) continue;
			const sc = (m * c.s) / 16;
			const cx = r.x + r.w * (0.5 + c.x * 0.3);
			const cy = r.y + r.h * (0.4 + c.y * 0.27) + f * f * H * 0.9;
			const rot = c.rot + (RM ? 0 : T * c.spin) + f * 2.4;
			const cs = Math.cos(rot) * sc;
			const sn = Math.sin(rot) * sc;
			ctx!.setTransform(dpr * cs, dpr * sn, -dpr * sn, dpr * cs, dpr * cx, dpr * cy);
			ctx!.translate(-12, -12);
			ctx!.lineWidth = 1.4 / sc;
			ctx!.strokeStyle = rgba(201, 164, 106, 0.3 * (1 - f) * o.shed);
			ctx!.stroke(COIL);
		}
		ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
	}

	function drawGlows(o: St) {
		if (tier >= 2) return;
		ctx!.globalCompositeOperation = 'lighter';
		for (let i = 0; i < N; i++) {
			const g = o.g[i];
			const a = o.a[i];
			if (g < 1 || a <= 0.01) continue;
			const x = o.x[i];
			const y = o.y[i];
			if (x < -g || y < -g || x > W + g || y > H + g) continue;
			const grad = ctx!.createRadialGradient(x, y, 0, x, y, g);
			grad.addColorStop(0, rgba(CR[i], CG[i], CB[i], 0.4 * a * (0.3 + 0.7 * o.lit[i])));
			grad.addColorStop(1, rgba(CR[i], CG[i], CB[i], 0));
			ctx!.fillStyle = grad;
			ctx!.fillRect(x - g, y - g, g * 2, g * 2);
		}
		ctx!.globalCompositeOperation = 'source-over';
	}

	/** Points in flight go into a trail buffer that fades a little each frame, so they streak; at rest (or with
	 *  nothing in flight) they are drawn straight onto the frame. */
	let trailLive = false;
	function drawPoints(o: St, trails: boolean) {
		let any = false;
		for (let i = 0; i < N && !any; i++) any = o.a[i] > 0.01;
		const target = trails && any ? tctx : ctx!;
		if (trails && any) {
			tctx.setTransform(1, 0, 0, 1, 0, 0);
			tctx.globalCompositeOperation = 'destination-out';
			tctx.fillStyle = `rgba(0,0,0,${MOTION.trailFade})`;
			tctx.fillRect(0, 0, trailCv.width, trailCv.height);
			tctx.globalCompositeOperation = 'source-over';
			trailLive = true;
		} else if (trailLive) {
			tctx.setTransform(1, 0, 0, 1, 0, 0);
			tctx.clearRect(0, 0, trailCv.width, trailCv.height);
			trailLive = false;
		}
		if (!any) return;
		target.setTransform(dpr, 0, 0, dpr, 0, 0);
		if (target === ctx) ctx!.globalCompositeOperation = 'lighter';
		for (let i = 0; i < N; i++) {
			const a = o.a[i];
			if (a <= 0.01) continue;
			const x = o.x[i];
			const y = o.y[i];
			const r = o.r[i] * (tier >= 2 ? 1.5 : 1);
			if (x < -r || y < -r || x > W + r || y > H + r) continue;
			target.fillStyle = rgba(CR[i], CG[i], CB[i], a);
			target.beginPath();
			target.arc(x, y, Math.max(0.6, r), 0, Math.PI * 2);
			target.fill();
		}
		if (target === tctx) {
			ctx!.globalCompositeOperation = 'lighter';
			ctx!.setTransform(1, 0, 0, 1, 0, 0);
			ctx!.drawImage(trailCv, 0, 0);
			ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
		}
		ctx!.globalCompositeOperation = 'source-over';
	}

	function drawPath(o: St) {
		if (o.path <= 0.01 || polyN < 1) return;
		const [tx, ty, th] = o.pathT;
		const c = Math.cos(th);
		const s = Math.sin(th);
		ctx!.strokeStyle = rgba(201, 164, 106, 0.32 * o.path);
		ctx!.lineWidth = 1;
		ctx!.beginPath();
		for (let j = 0; j <= polyN; j++) {
			const x = polyX[j];
			const y = polyY[j];
			if (j === 0) ctx!.moveTo(tx + c * x - s * y, ty + s * x + c * y);
			else ctx!.lineTo(tx + c * x - s * y, ty + s * x + c * y);
		}
		ctx!.stroke();
	}

	function drawMark(o: St) {
		if (o.mark <= 0.01) return;
		const r = o.markRect;
		const size = Math.min(r.w, r.h);
		const k = (size * 0.9) / 16;
		ctx!.setTransform(dpr * k, 0, 0, dpr * k, dpr * (r.x + r.w / 2 - 12 * k), dpr * (r.y + r.h / 2 - 12 * k));
		ctx!.lineWidth = 1 / k;
		ctx!.lineCap = 'round';
		ctx!.strokeStyle = rgba(201, 164, 106, 0.38 * o.mark);
		ctx!.stroke(COIL);
		ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
	}

	function drawScale(o: St, cam: V) {
		if (o.scale <= 0.01) return;
		const x0 = toX(cam, kbDev.x);
		const y0 = toY(cam, DESK.y0 + DESK.h) + 10;
		const x1 = x0 + cam.k;
		ctx!.strokeStyle = rgba(236, 230, 218, 0.6 * o.scale);
		ctx!.lineWidth = 1;
		ctx!.beginPath();
		ctx!.moveTo(x0, y0 - 4);
		ctx!.lineTo(x0, y0 + 4);
		ctx!.moveTo(x0, y0);
		ctx!.lineTo(x1, y0);
		ctx!.moveTo(x1, y0 - 4);
		ctx!.lineTo(x1, y0 + 4);
		ctx!.stroke();
		ctx!.font = `12px ${MONO}`;
		ctx!.fillStyle = rgba(236, 230, 218, 0.8 * o.scale);
		ctx!.textBaseline = 'middle';
		ctx!.fillText('1u = 19.05 mm', x1 + 10, y0);
	}

	/** uncoild, its cable into the keyboard, the write, the unplug, and Fn+P still working. */
	function drawBrain(o: St, cam: V) {
		const w = o.brain;
		if (w <= 0.01) return;
		const r = o.brainRect;
		const nx = r.x + r.w - (W < 700 ? 22 : 40);
		const ny = r.y + (W < 700 ? 16 : 30);
		const cap = desk.cap('P');
		const [px, py] = cap ? desk.toScreen(cam, cap.x, cap.y) : [nx, ny + 100];
		const [px1, py1] = cap ? desk.toScreen(cam, cap.x + cap.w, cap.y + cap.h) : [nx + 10, ny + 110];
		const ex = (px + px1) / 2;
		const ey = py - 3;
		const bez = (u: number): [number, number] => {
			const c1y = ny + (ey - ny) * 0.65;
			const c2y = ey - (ey - ny) * 0.55;
			const m = 1 - u;
			return [m * m * m * nx + 3 * m * m * u * nx + 3 * m * u * u * ex + u * u * u * ex, m * m * m * (ny + 14) + 3 * m * m * u * c1y + 3 * m * u * u * c2y + u * u * u * ey];
		};
		if (o.cable > 0.005) {
			ctx!.strokeStyle = rgba(201, 164, 106, 0.7 * w);
			ctx!.lineWidth = 1.2;
			ctx!.beginPath();
			const steps = 48;
			for (let j = 0; j <= steps * o.cable; j++) {
				const [x, y] = bez(j / steps);
				if (j === 0) ctx!.moveTo(x, y);
				else ctx!.lineTo(x, y);
			}
			ctx!.stroke();
			if (o.bytes > 0.01) {
				ctx!.fillStyle = rgba(236, 230, 218, o.bytes * w);
				for (let j = 0; j < 7; j++) {
					const u = RM ? (j + 0.5) / 7 : (T * 0.35 + j / 7) % 1;
					if (u > o.cable) continue;
					const [x, y] = bez(u);
					ctx!.beginPath();
					ctx!.arc(x, y, 2.2, 0, Math.PI * 2);
					ctx!.fill();
				}
			}
		}
		const na = o.node * w;
		if (na > 0.01) {
			const s = 22 / 16;
			ctx!.setTransform(dpr * s, 0, 0, dpr * s, dpr * (nx - 12 * s), dpr * (ny - 12 * s));
			ctx!.lineWidth = 1.6 / s;
			ctx!.strokeStyle = rgba(201, 164, 106, na);
			ctx!.stroke(COIL);
			ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
			ctx!.font = `12px ${MONO}`;
			ctx!.textAlign = 'right';
			ctx!.textBaseline = 'middle';
			ctx!.fillStyle = rgba(236, 230, 218, 0.85 * w);
			ctx!.fillText(o.press > 0.5 ? 'uncoild · stopped' : 'uncoild', nx - 20, ny);
		}
		if (o.press > 0.01 && cap) {
			const kw = px1 - px;
			const ph = pressPhase();
			const cx = ex;
			const cy = (py + py1) / 2;
			ctx!.strokeStyle = rgba(201, 164, 106, (1 - ph) * o.press * w);
			ctx!.lineWidth = 1.5;
			ctx!.beginPath();
			ctx!.arc(cx, cy, kw * (0.55 + ph * 1.3), 0, Math.PI * 2);
			ctx!.stroke();
			ctx!.font = `${W < 700 ? 11 : 13}px ${MONO}`;
			ctx!.textAlign = 'center';
			ctx!.textBaseline = 'middle';
			ctx!.fillStyle = rgba(236, 230, 218, o.press * w);
			ctx!.fillText('Print Screen', cx, py - Math.max(14, cam.k * 0.5));
		}
		ctx!.textAlign = 'start';
	}
	const pressPhase = () => (RM ? 0.15 : (T % 1.6) / 1.6);

	// ------------------------------------------------------------------------------------------ the loop
	let T = forcedT ?? (RM ? 6 : t0);
	let playing = !RM;
	let userPaused = false;
	let raf = 0;
	let last = 0;
	let dirty = true;
	let slow = 0;
	let lastY = Number.NaN;
	let visibleContent = true;
	const film = { x: forcedY ?? scrollY, v: 0 };
	const par = { x: { x: 0, v: 0 }, y: { x: 0, v: 0 }, tx: 0, ty: 0 };
	let held: { y: number } | null = forcedY !== null ? { y: forcedY } : null;
	const domVals = new Map<string, number>();
	const setVar = (el: HTMLElement | null, name: string, val: number) => {
		if (!el) return;
		const key = name + (el.dataset.scene ?? el.className);
		const prev = domVals.get(key);
		if (prev !== undefined && Math.abs(prev - val) < 0.002) return;
		domVals.set(key, val);
		el.style.setProperty(name, val.toFixed(3));
	};

	function frame(now: number) {
		raf = 0;
		if (document.hidden) return;
		const dt = last ? Math.min(0.05, (now - last) / 1000) : 1 / 60;
		last = now;
		if (!RM && dt > 0.03) {
			slow++;
			if (slow > 50 && tier < 2) {
				tier++;
				slow = 0;
				resize();
			}
		} else slow = Math.max(0, slow - 0.5);
		const running = playing && !userPaused && forcedT === null;
		if (running) T += dt;
		// the film follows the page through a spring (reduced motion: no lag at all)
		const target = held ? held.y : scrollY;
		let moving = false;
		if (RM || held) {
			film.x = target;
			film.v = 0;
		} else {
			// sub-step for stability on long frames
			const n = Math.ceil(dt / (1 / 120));
			for (let k = 0; k < n; k++) spring(film, target, MOTION.scrollOmega, dt / n);
			if (Math.abs(film.x - target) < 0.3 && Math.abs(film.v) < 2) {
				film.x = target;
				film.v = 0;
			} else moving = true;
			for (const [s, tg] of [[par.x, par.tx], [par.y, par.ty]] as const) {
				spring(s, tg, MOTION.pointerOmega, dt);
				if (Math.abs(s.x - tg) > 0.002 || Math.abs(s.v) > 0.002) moving = true;
			}
		}
		const shifted = film.x !== lastY;
		if (moving || shifted || dirty || (running && visibleContent)) render(film.x, moving && !RM && !held);
		lastY = film.x;
		dirty = false;
		if (moving || (running && visibleContent)) raf = requestAnimationFrame(frame);
		else last = 0;
	}

	function render(y: number, trails = false) {
		let { s, p, tau } = locate(y);
		if (RM) {
			const ks = scenes[s].keys;
			p = ks <= 1 ? (RM_ONE[scenes[s].kind] ?? 1) : Math.round(p * (ks - 1)) / (ks - 1);
			if (tau >= 0) {
				if (tau >= 0.5) {
					s += 1;
					p = scenes[s].keys <= 1 ? (RM_ONE[scenes[s].kind] ?? 1) : 0;
				}
				tau = -1;
			}
		}
		// Read the DOM first, write it last.
		stripRect = stripEl?.getBoundingClientRect() ?? null;
		if (tau >= 0) {
			stateOf(s, p, A);
			stateOf(s + 1, 0, B);
			blend(A, B, tau, O);
		} else {
			stateOf(s, p, A);
			blend(A, A, 1, O);
		}
		const o = O;
		visibleContent = !(scenes[s].kind === 'install' && (o.markRect.y > H || o.markRect.y + o.markRect.h < 0));
		// one camera: the scene framing, plus a faint drift toward the pointer while the desk is up
		const cam: V = { ...o.view };
		cam.sx += par.x.x * MOTION.parallax[0] * o.art;
		cam.sy += par.y.x * MOTION.parallax[1] * o.art;

		colours(o);
		ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
		ctx!.clearRect(0, 0, W, H);
		ctx!.globalCompositeOperation = 'source-over';
		if (o.art > 0.002) {
			desk.draw(ctx!, cam, dpr, {
				rgb: artRgb,
				light: o.light,
				pen: o.pen,
				alpha: o.art,
				gels: o.fn > 0.01 ? gelsAt(o.fn) : undefined,
				legends: o.legends * o.fn,
				marks: o.fn > 0.01 ? marksAt(o.fn) : undefined,
				pressed: o.press > 0.01 ? new Map([['P', pressPhase() < 0.2 ? o.press : 0]]) : undefined,
				quality: tier,
			});
			ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
		}
		if (o.field > 0.002) drawField(o);
		if (o.shed > 0.002) drawCoils(o);
		drawPath(o);
		drawMark(o);
		drawGlows(o);
		drawPoints(o, trails);
		drawScale(o, cam);
		drawBrain(o, cam);

		// DOM: the hero caption, the shed labels, the strip.
		setVar(pins.hero, '--hp', s === S.hero && tau < 0 ? p : s >= S.hero ? 1 : 0);
		setVar(pins.hero, '--hx', s === S.hero ? Math.max(0, tau) : s > S.hero ? 1 : 0);
		setVar(pins.shed, '--sp', s === S.shed ? p : s > S.shed ? 1 : 0);
		setVar(pins.written, '--sa', o.strip);
		setVar(pins.written, '--hi', o.hi);
		placeLamp(o);
		if (!root.classList.contains('film')) root.classList.add('film');
	}
	let gelCache: { a: number; m: Map<string, { color: string; sub?: string }> } | null = null;
	function gelsAt(a: number) {
		if (gelCache && Math.abs(gelCache.a - a) < 0.01) return gelCache.m;
		const hex = (h: string) => {
			const n = parseInt(h.slice(1), 16);
			return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a.toFixed(2)})`;
		};
		gelCache = { a, m: new Map(FN_GELS.map(([k, sub, c]) => [k, { color: hex(c), sub }])) };
		return gelCache.m;
	}
	const marksAt = (a: number) =>
		new Map([
			['Right Fn', `rgba(201,164,106,${a.toFixed(2)})`],
			['P', `rgba(227,196,142,${a.toFixed(2)})`],
		]);

	const kick = () => {
		if (!raf && !document.hidden) raf = requestAnimationFrame(frame);
	};

	// Controls: one toggle for the light. Reduced motion starts held, on a still frame, until Play.
	const toggle = document.querySelector<HTMLButtonElement>('[data-light-toggle]');
	if (toggle) toggle.hidden = false;
	const syncToggle = () => {
		if (!toggle) return;
		const on = playing && !userPaused;
		toggle.setAttribute('aria-pressed', String(!on));
		const label = toggle.querySelector('.t');
		if (label) label.textContent = on ? 'Pause the light' : 'Play the light';
	};
	toggle?.addEventListener('click', () => {
		if (RM) playing = !playing;
		else userPaused = !userPaused;
		syncToggle();
		dirty = true;
		kick();
	});
	reduce.addEventListener('change', () => {
		RM = reduce.matches;
		playing = !RM;
		if (RM) T = 6;
		syncToggle();
		revealAll();
		dirty = true;
		kick();
	});
	// A faint drift toward the pointer (fine pointers only; never under reduced motion).
	if (matchMedia('(pointer: fine)').matches)
		addEventListener(
			'pointermove',
			(e) => {
				if (RM) return;
				par.tx = (e.clientX / W) * 2 - 1;
				par.ty = (e.clientY / H) * 2 - 1;
				kick();
			},
			{ passive: true },
		);

	// ---- copy: data types on; prose arrives with a quiet fade and lift ------------------------------------
	const typed = [...document.querySelectorAll<HTMLElement>('[data-type]')];
	const prose = [
		...document.querySelectorAll<HTMLElement>('.plate > *, .hero-side > *, .kicker, .install-body > *, .steps > li, .faq h2, .qs, .hero-cap'),
	].filter((el) => !el.closest('[data-type]') && !el.matches('.hero-cap'));
	function revealAll() {
		for (const el of typed) el.style.setProperty('--k', '1');
		for (const el of prose) el.classList.add('in');
	}
	if (!RM) {
		for (const el of typed) el.style.setProperty('--k', '0');
		prose.forEach((el) => {
			const sib = el.parentElement ? [...el.parentElement.children].indexOf(el) : 0;
			el.style.setProperty('--i', String(Math.min(sib, 6)));
			el.classList.add('rv');
		});
		// Watch each clipped element's container: a fully clipped element never counts as intersecting.
		const watch = new Map<Element, HTMLElement[]>();
		for (const el of typed) {
			const box = el.closest('p, pre') ?? el.parentElement ?? el;
			watch.set(box, [...(watch.get(box) ?? []), el]);
		}
		const io = new IntersectionObserver(
			(entries) => {
				for (const e of entries) {
					if (!e.isIntersecting) continue;
					io.unobserve(e.target);
					for (const el of watch.get(e.target) ?? []) typeOne(el);
					if (prose.includes(e.target as HTMLElement)) (e.target as HTMLElement).classList.add('in');
				}
			},
			{ threshold: 0.2, rootMargin: '0px 0px -8% 0px' },
		);
		watch.forEach((_, box) => io.observe(box));
		prose.forEach((el) => io.observe(el));
		function typeOne(el: HTMLElement) {
			const n = Math.max(1, (el.textContent ?? '').length);
			const delay = Number(el.dataset.typeDelay ?? 0);
			const start = performance.now() + delay;
			const step = (now: number) => {
				const k = clamp01((now - start) / (n * MOTION.typeMs));
				el.style.setProperty('--k', (Math.floor(k * n) / n).toFixed(3));
				if (k < 1) requestAnimationFrame(step);
			};
			requestAnimationFrame(step);
		}
	}

	// Grain: one noise tile, made here, stepped by CSS at a low frame rate.
	const grain = document.querySelector<HTMLElement>('.grain');
	if (grain) {
		const g = document.createElement('canvas');
		g.width = g.height = 140;
		const gc = g.getContext('2d')!;
		const img = gc.createImageData(140, 140);
		for (let i = 0; i < img.data.length; i += 4) {
			const n = rand() * 255;
			img.data[i] = img.data[i + 1] = img.data[i + 2] = n;
			img.data[i + 3] = 255;
		}
		gc.putImageData(img, 0, 0);
		grain.style.setProperty('--noise', `url(${g.toDataURL()})`);
	}

	measure();
	syncToggle();
	addEventListener('scroll', kick, { passive: true });
	addEventListener('resize', () => {
		measure();
		kick();
	});
	new ResizeObserver(() => {
		measure();
		kick();
	}).observe(document.body);
	document.addEventListener('visibilitychange', () => {
		last = 0;
		kick();
	});
	document.fonts?.ready.then(() => {
		measure();
		kick();
	});

	// Debug: render any moment on demand, and time the renderer (see the top of this file).
	if (params.has('debug') || forcedY !== null) {
		(window as unknown as { __uncoil: unknown }).__uncoil = {
			at(y: number, t?: number) {
				held = { y };
				if (t !== undefined) T = t;
				film.x = y;
				render(y, false);
			},
			/** ms per frame, rendering n frames at scroll y (CPU side: the GPU rasterises asynchronously). With
			 *  `sync`, each frame ends with a 1-pixel readback, which forces the raster but also pushes Chrome's canvas
			 *  onto the CPU, so treat that number as a worst case. */
			bench(n = 120, y?: number, sync = false) {
				const yy = y ?? held?.y ?? scrollY;
				render(yy, false);
				const t0b = performance.now();
				for (let k = 0; k < n; k++) {
					T += 1 / 60;
					render(yy, true);
					if (sync) ctx!.getImageData(0, 0, 1, 1);
				}
				return (performance.now() - t0b) / n;
			},
			release() {
				held = null;
				kick();
			},
		};
	}
	kick();
}
