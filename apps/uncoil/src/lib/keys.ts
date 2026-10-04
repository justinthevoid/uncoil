// Key-mapping vocabulary for the Keys screen: the spec strings uncoild understands
// (uncoil_core::features::keymap::Function::parse_spec), their human labels, and the choices the editor offers.

/** HID keyboard-page names, grouped for the picker. Mirrors `usage_name` / `USAGES` in keymap.rs. */
export const KEY_GROUPS: { label: string; keys: string[] }[] = [
	{ label: 'Letters', keys: 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'.split('') },
	{ label: 'Numbers', keys: '1234567890'.split('') },
	{ label: 'Function', keys: Array.from({ length: 24 }, (_, i) => `F${i + 1}`) },
	{
		label: 'Editing and navigation',
		keys: ['ENTER', 'ESCAPE', 'BACKSPACE', 'TAB', 'SPACE', 'CAPS_LOCK', 'PRINT_SCREEN', 'SCROLL_LOCK', 'PAUSE', 'INSERT', 'HOME', 'PAGE_UP', 'DELETE', 'END', 'PAGE_DOWN', 'RIGHT', 'LEFT', 'DOWN', 'UP', 'APPLICATION']
	},
	{
		label: 'Symbols',
		keys: ['MINUS', 'EQUAL', 'LEFT_BRACKET', 'RIGHT_BRACKET', 'BACKSLASH', 'SEMICOLON', 'APOSTROPHE', 'GRAVE', 'COMMA', 'PERIOD', 'SLASH', 'NON_US_HASH', 'NON_US_BACKSLASH']
	},
	{
		label: 'Keypad',
		keys: ['NUM_LOCK', 'KP_SLASH', 'KP_ASTERISK', 'KP_MINUS', 'KP_PLUS', 'KP_ENTER', 'KP_PERIOD', 'KP_EQUAL', ...Array.from({ length: 10 }, (_, i) => `KP_${i}`)]
	},
	{ label: 'Media', keys: ['MUTE', 'VOLUME_UP', 'VOLUME_DOWN', 'POWER'] },
	{ label: 'International', keys: ['INTL_RO', 'KATAKANA_HIRAGANA', 'YEN', 'HENKAN', 'MUHENKAN', 'HANGEUL', 'HANJA'] }
];

/** Modifier bits as the spec names them (`+lctrl`). The editor offers the left-hand four. */
export const MODIFIERS = [
	{ spec: 'lctrl', label: 'Ctrl' },
	{ spec: 'lshift', label: 'Shift' },
	{ spec: 'lalt', label: 'Alt' },
	{ spec: 'lwin', label: 'Win' }
] as const;

export const MOUSE_BUTTONS: { value: number; label: string }[] = [
	{ value: 1, label: 'Left click' },
	{ value: 2, label: 'Right click' },
	{ value: 3, label: 'Middle click' },
	{ value: 4, label: 'Back' },
	{ value: 5, label: 'Forward' },
	{ value: 9, label: 'Wheel up' },
	{ value: 10, label: 'Wheel down' },
	{ value: 104, label: 'Wheel tilt left' },
	{ value: 105, label: 'Wheel tilt right' }
];

/** The BlackWidow V4 Pro 75%'s Razer-only key codes. Other models use the same codes for other things. */
const BLACKWIDOW = 'razer-blackwidow-v4-pro-75';
const BW_KEYS: Record<number, string> = {
	1: 'Fn held',
	3: 'game mode',
	4: 'macro record',
	8: 'backlight up',
	9: 'backlight down',
	11: 'low power mode',
	76: 'system sleep',
	82: 'dial click',
	83: 'next',
	84: 'previous',
	85: 'play/pause',
	96: 'dial'
};

/** Keycap-sized names for the BlackWidow's Razer keys. */
const BW_CAPS: Record<number, string> = {
	1: 'Fn',
	3: 'Game',
	4: 'Macro',
	8: 'Lit +',
	9: 'Lit −',
	11: 'Eco',
	76: 'Sleep',
	82: 'Dial',
	83: 'Next',
	84: 'Prev',
	85: 'Play',
	96: 'Dial'
};

const SHORT: Record<string, string> = {
	PRINT_SCREEN: 'PrtSc',
	SCROLL_LOCK: 'ScrLk',
	PAGE_UP: 'PgUp',
	PAGE_DOWN: 'PgDn',
	BACKSPACE: 'Bksp',
	CAPS_LOCK: 'Caps',
	ESCAPE: 'Esc',
	DELETE: 'Del',
	INSERT: 'Ins',
	APPLICATION: 'Menu',
	VOLUME_UP: 'Vol+',
	VOLUME_DOWN: 'Vol−',
	LEFT_BRACKET: '[',
	RIGHT_BRACKET: ']',
	BACKSLASH: '\\',
	SEMICOLON: ';',
	APOSTROPHE: "'",
	GRAVE: '`',
	COMMA: ',',
	PERIOD: '.',
	SLASH: '/',
	MINUS: '-',
	EQUAL: '=',
	SPACE: 'Space',
	ENTER: 'Enter',
	RIGHT: '→︎',
	LEFT: '←︎',
	UP: '↑︎',
	DOWN: '↓︎'
};

/** Razer-only key codes per device id: what each does, and a keycap-sized name. Unknown devices get neither. */
const RAZER_KEYS: Record<string, Record<number, string>> = { [BLACKWIDOW]: BW_KEYS };
const RAZER_CAPS: Record<string, Record<number, string>> = { [BLACKWIDOW]: BW_CAPS };
const razerKey = (n: number, device?: string | null) => (device ? RAZER_KEYS[device]?.[n] : undefined);

const ACRONYMS = /\b(dpi|oled|usb|led|kp)\b/g;
const title = (s: string) =>
	s.toLowerCase().replace(/_/g, ' ').replace(ACRONYMS, (m) => m.toUpperCase()).replace(/^\w/, (c) => c.toUpperCase());

/** A parsed spec, as the editor's form holds it. */
export type Mapping =
	| { type: 'key'; key: string; mods: string[] }
	| { type: 'button'; button: number }
	| { type: 'off' }
	| { type: 'other'; spec: string };

export function parseSpec(spec: string): Mapping {
	const [head, ...rest] = spec.trim().split(/\s+/);
	switch (head?.toLowerCase()) {
		case 'key': {
			const mods = rest.filter((t) => t.startsWith('+')).map((t) => t.slice(1).toLowerCase());
			const key = rest.find((t) => !t.startsWith('+')) ?? 'NONE';
			return { type: 'key', key: key.toUpperCase(), mods };
		}
		case 'button':
		case 'mouse':
			return { type: 'button', button: Number(rest[0]) };
		case 'off':
		case 'none':
		case 'disabled':
			return { type: 'off' };
		default:
			return { type: 'other', spec };
	}
}

export function toSpec(m: Mapping): string {
	switch (m.type) {
		case 'key':
			return ['key', m.key, ...m.mods.map((x) => `+${x}`)].join(' ');
		case 'button':
			return `button ${m.button}`;
		case 'off':
			return 'off';
		case 'other':
			return m.spec;
	}
}

/** Close to `Function::describe` in keymap.rs; only used where the daemon hasn't described it (the mock). */
export function describeFunction(spec: string, device?: string | null): string {
	const m = parseSpec(spec);
	if (m.type === 'off') return 'nothing';
	if (m.type === 'button') return `mouse button ${m.button}`;
	if (m.type === 'key') {
		if (m.key === 'NONE' && !m.mods.length) return 'empty key code (does nothing)';
		const mods = m.mods.map((x) => x[0].toUpperCase() + title(x.slice(1)));
		return [...mods, ...(m.key === 'NONE' ? [] : [title(m.key)])].join('+');
	}
	const [head, n] = spec.split(/\s+/);
	if (head === 'razer') {
		const name = razerKey(Number(n), device);
		return name ? `Razer key ${n} (${name}; handled by host software)` : `Razer key ${n} (handled by host software)`;
	}
	return spec;
}

/** A few characters for the key cap in the keyboard drawing. */
export function shortLabel(spec: string, device?: string | null): string {
	const m = parseSpec(spec);
	if (m.type === 'off') return 'Off';
	if (m.type === 'button') return MOUSE_BUTTONS.find((b) => b.value === m.button)?.label.split(' ')[0] ?? `M${m.button}`;
	if (m.type === 'key') {
		if (m.key === 'NONE') return m.mods.length ? m.mods.map((x) => x[1].toUpperCase()).join('') : '—';
		const base = SHORT[m.key] ?? (m.key.length <= 4 ? m.key : title(m.key).slice(0, 5));
		return m.mods.length ? `${m.mods.map((x) => x[1].toUpperCase()).join('')}+${base}` : base;
	}
	const [head, n] = spec.split(/\s+/);
	if (head === 'razer') return (device ? RAZER_CAPS[device]?.[Number(n)] : undefined) ?? `Rz${n}`;
	if (head === 'dpi') return 'DPI';
	if (head === 'profile') return 'Prof';
	return head;
}

/** Human names for the command dial's modes (uncoil_core::features::dial::DialMode). */
export const DIAL_MODES: Record<string, string> = {
	VOLUME: 'Volume',
	TRACK_SELECTOR: 'Track select',
	OLED_BRIGHTNESS: 'Screen brightness',
	LIGHTNING_BRIGHTNESS: 'Lighting brightness',
	SWITCH_APPS: 'Switch apps',
	ZOOM: 'Zoom',
	TRACK_JOGGING: 'Track jog',
	SCROLL_VERTICAL: 'Scroll',
	SCROLL_HORIZONTAL: 'Scroll sideways'
};

/** Key names as the keymap reports them, made readable ("OPEN_SQUARE_BRACKET" → "Open square bracket"). */
export const keyTitle = (name: string) => (name.length <= 3 ? name : title(name));

/** The swatch book's gels: one job each. */
export type Gel = 'yours' | 'media' | 'light' | 'system';
export const GEL_NAMES: Record<Gel, string> = {
	yours: 'Your change',
	media: 'Media & macros',
	light: 'Lighting',
	system: 'System'
};

const BW_GEL: Record<number, Gel> = { 1: 'system', 3: 'media', 4: 'media', 8: 'light', 9: 'light', 11: 'system', 76: 'system', 82: 'media', 83: 'media', 84: 'media', 85: 'media', 96: 'media' };
const MEDIA_KEYS = new Set(['MUTE', 'VOLUME_UP', 'VOLUME_DOWN']);
const RAZER_GEL: Record<string, Record<number, Gel>> = { [BLACKWIDOW]: BW_GEL };

/**
 * Which gel a mapping carries, or null for a key doing its ordinary job. `changed` says the mapping differs
 * from what the key does normally (on the Fn layer: differs from the normal layer).
 */
export function gelFor(spec: string, changed: boolean, device?: string | null): Gel | null {
	if (!changed) return null;
	const [head, n] = spec.trim().split(/\s+/);
	if (head === 'razer') return (device ? RAZER_GEL[device]?.[Number(n)] : undefined) ?? 'system';
	if (head === 'power' || head === 'profile' || head === 'dpi') return 'system';
	if (head === 'lighting') return 'light';
	if (head === 'media' || head === 'macro') return 'media';
	const m = parseSpec(spec);
	if (m.type === 'key' && MEDIA_KEYS.has(m.key) && !m.mods.length) return 'media';
	return 'yours';
}

/** A short, human name for what a mapping does (lists and the details panel). */
export function actionName(spec: string, description: string, device?: string | null): string {
	const [head, ...rest] = spec.trim().split(/\s+/);
	const n = Number(rest[0]);
	switch (head) {
		case 'razer': {
			const name = razerKey(n, device);
			return name ? name[0].toUpperCase() + name.slice(1) : `Synapse-only key ${n}`;
		}
		case 'button':
		case 'mouse':
			return MOUSE_BUTTONS.find((b) => b.value === n)?.label ?? `Mouse button ${n}`;
		case 'dpi':
			return n === 5 ? 'DPI clutch (hold for slow aim)' : n === 6 ? 'Next DPI stage' : n === 7 ? 'Previous DPI stage' : 'DPI change';
		case 'profile':
			return n === 4 ? 'Next profile' : 'Profile switch';
		case 'shortcut':
			return 'Scroll wheel mode';
		case 'off':
			return 'Does nothing';
		default: {
			const m = parseSpec(spec);
			if (m.type === 'key' && m.key === 'NONE' && !m.mods.length) return 'Does nothing';
			return description.replace(/ \(.*\)$/, '');
		}
	}
}
