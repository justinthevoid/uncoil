// The site's desk renderer: the desktop app's device art (line art traced from product photos), as geometry
// (build time and browser), a canvas renderer (browser) and an SVG still (build time).
export { deskGeometry, FINISH, legendFor } from './geometry';
export type { DeskGeo, DeviceGeo, KeyboardGeo, MouseGeo, MatGeo, CapGeo, Rect, RRect } from './geometry';
export { createDeskRenderer } from './renderer';
export type { Camera, DeskFrame, DeskRenderer } from './renderer';
export { deskSvg } from './svg';
export type { SvgOptions } from './svg';
