// A 3x3x3 simulator for the landing page: enough to draw the net of a scramble
// the way the app's preview does. Stickers are tracked by position and normal,
// so a move is one quarter-turn rotation of the stickers in its layer.

type Vec = readonly [number, number, number];
type Sticker = { pos: Vec; normal: Vec; color: Color };

export type Color = 'white' | 'yellow' | 'green' | 'blue' | 'red' | 'orange';
export type Face = 'U' | 'D' | 'F' | 'B' | 'R' | 'L';

const FACES: Record<Face, { normal: Vec; color: Color; right: Vec; down: Vec }> = {
  U: { normal: [0, 1, 0], color: 'white', right: [1, 0, 0], down: [0, 0, 1] },
  D: { normal: [0, -1, 0], color: 'yellow', right: [1, 0, 0], down: [0, 0, -1] },
  F: { normal: [0, 0, 1], color: 'green', right: [1, 0, 0], down: [0, -1, 0] },
  B: { normal: [0, 0, -1], color: 'blue', right: [-1, 0, 0], down: [0, -1, 0] },
  R: { normal: [1, 0, 0], color: 'red', right: [0, 0, -1], down: [0, -1, 0] },
  L: { normal: [-1, 0, 0], color: 'orange', right: [0, 0, 1], down: [0, -1, 0] },
};

/** Where each face sits in the unfolded net, in units of one face. */
export const NET_LAYOUT: readonly { face: Face; col: number; row: number }[] = [
  { face: 'U', col: 1, row: 0 },
  { face: 'L', col: 0, row: 1 },
  { face: 'F', col: 1, row: 1 },
  { face: 'R', col: 2, row: 1 },
  { face: 'B', col: 3, row: 1 },
  { face: 'D', col: 1, row: 2 },
];

const add = (a: Vec, b: Vec, c: Vec): Vec => [a[0] + b[0] + c[0], a[1] + b[1] + c[1], a[2] + b[2] + c[2]];
const scale = (a: Vec, k: number): Vec => [a[0] * k, a[1] * k, a[2] * k];
const dot = (a: Vec, b: Vec) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const same = (a: Vec, b: Vec) => a[0] === b[0] && a[1] === b[1] && a[2] === b[2];
const cross = (a: Vec, b: Vec): Vec => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];

/** Turns a vector a quarter clockwise, seen from outside along `axis`. */
function quarter(v: Vec, axis: Vec): Vec {
  const along = scale(axis, dot(v, axis));
  const across: Vec = [v[0] - along[0], v[1] - along[1], v[2] - along[2]];
  const turned = scale(cross(axis, across), -1);
  return [along[0] + turned[0], along[1] + turned[1], along[2] + turned[2]];
}

function solved(): Sticker[] {
  const stickers: Sticker[] = [];
  for (const { normal, color, right, down } of Object.values(FACES)) {
    for (let r = -1; r <= 1; r++) {
      for (let c = -1; c <= 1; c++) {
        stickers.push({ pos: add(normal, scale(right, c), scale(down, r)), normal, color });
      }
    }
  }
  return stickers;
}

/**
 * The colors of a scrambled cube, in NET_LAYOUT order and reading order inside
 * each face: 54 entries.
 */
export function netColors(scramble: string): Color[] {
  let stickers = solved();
  for (const move of scramble.trim().split(/\s+/).filter(Boolean)) {
    const axis = FACES[move[0] as Face].normal;
    const turns = move[1] === '2' ? 2 : move[1] === "'" ? 3 : 1;
    for (let t = 0; t < turns; t++) {
      stickers = stickers.map((s) =>
        dot(s.pos, axis) === 1
          ? { pos: quarter(s.pos, axis), normal: quarter(s.normal, axis), color: s.color }
          : s,
      );
    }
  }
  const colors: Color[] = [];
  for (const { face } of NET_LAYOUT) {
    const { normal, right, down } = FACES[face];
    for (let r = -1; r <= 1; r++) {
      for (let c = -1; c <= 1; c++) {
        const pos = add(normal, scale(right, c), scale(down, r));
        colors.push(stickers.find((s) => same(s.pos, pos) && same(s.normal, normal))!.color);
      }
    }
  }
  return colors;
}

const AXIS: Record<Face, number> = { U: 0, D: 0, F: 1, B: 1, R: 2, L: 2 };

/** A random-move scramble. The app itself uses official random-state scrambles. */
export function randomScramble(length = 20): string {
  const faces = Object.keys(AXIS) as Face[];
  const moves: string[] = [];
  let last: Face | null = null;
  let before: Face | null = null;
  while (moves.length < length) {
    const face = faces[Math.floor(Math.random() * faces.length)];
    if (face === last) continue;
    if (last && before && AXIS[face] === AXIS[last] && AXIS[last] === AXIS[before]) continue;
    moves.push(face + ['', "'", '2'][Math.floor(Math.random() * 3)]);
    before = last;
    last = face;
  }
  return moves.join(' ');
}
