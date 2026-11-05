// The default shortcuts, as drawn on the landing page keyboard. Keep in step
// with src/content/docs/keybinds.md.

export type Binding = readonly [combo: string, action: string];

export const BINDINGS: Record<string, readonly Binding[]> = {
  Space: [['Space', 'Start inspection, hold and release to start the solve, press to stop']],
  Escape: [['Esc', 'Cancel the attempt, or leave the stats or charts screen']],
  F1: [['F1', 'Open the keyboard shortcuts']],
  F2: [['F2', 'Rename the session']],
  Digit2: [['2', 'Toggle +2 on the selected solve']],
  KeyN: [['N', 'New scramble']],
  KeyI: [['I', 'Turn WCA inspection on or off']],
  KeyP: [['P', 'Show or hide the scramble preview']],
  KeyM: [['M', 'Turn manual mode on or off']],
  KeyD: [
    ['D', 'Delete the selected solve'],
    ['Shift + D', 'Toggle DNF on the selected solve'],
  ],
  KeyE: [
    ['E', 'Next event'],
    ['Shift + E', 'Previous event'],
  ],
  KeyS: [
    ['S', 'New session'],
    ['Shift + S', 'Delete the session'],
  ],
  BracketRight: [[']', 'Next session']],
  BracketLeft: [['[', 'Previous session']],
  KeyA: [['A', 'Show stats, or return to the timer']],
  KeyG: [['G', 'Show charts, or return to the timer']],
  KeyB: [['B', 'Connect or disconnect a Bluetooth timer']],
  KeyO: [
    ['O', 'Open the selected average'],
    ['Shift + O', 'Open the best of the selected average'],
  ],
  KeyC: [['C', 'Copy the selected average as text']],
  KeyV: [['V', 'Show the next charts view']],
  KeyF: [['F', 'Filter charts by the next event']],
  KeyR: [
    ['R', 'Filter charts by the next range'],
    ['Shift + R', 'Filter charts by custom dates'],
  ],
  KeyW: [['W', 'Chart one session or all of them']],
  KeyZ: [
    ['Z', 'Zoom the times chart in'],
    ['Shift + Z', 'Zoom the times chart out'],
    ['Ctrl + Z', 'Undo the last deletion'],
  ],
  KeyY: [['Ctrl + Y', 'Redo an undone deletion']],
  KeyX: [['X', 'Reset the times chart zoom']],
  Comma: [['Ctrl + ,', 'Open settings']],
  Enter: [['Enter', "Open the selected solve's details"]],
  ArrowUp: [['Up', 'Select a newer solve']],
  ArrowDown: [['Down', 'Select an older solve']],
  ArrowLeft: [['Left', 'Select a shorter average']],
  ArrowRight: [['Right', 'Select a longer average']],
};

export type Cap = { code?: string; label?: string; width?: number };

const letters = (row: string): Cap[] => [...row].map((c) => ({ code: `Key${c}`, label: c }));
const fn = (from: number, to: number): Cap[] =>
  Array.from({ length: to - from + 1 }, (_, i) => ({ code: `F${from + i}`, label: `F${from + i}` }));

export const KEYBOARD: readonly (readonly Cap[])[] = [
  [
    { code: 'Escape', label: 'Esc' },
    { width: 1 },
    ...fn(1, 4),
    { width: 0.5 },
    ...fn(5, 8),
    { width: 0.5 },
    ...fn(9, 12),
  ],
  [
    { code: 'Backquote', label: '`' },
    ...[1, 2, 3, 4, 5, 6, 7, 8, 9, 0].map((n) => ({ code: `Digit${n}`, label: String(n) })),
    { code: 'Minus', label: '-' },
    { code: 'Equal', label: '=' },
    { code: 'Backspace', label: 'Backspace', width: 2 },
  ],
  [
    { code: 'Tab', label: 'Tab', width: 1.5 },
    ...letters('QWERTYUIOP'),
    { code: 'BracketLeft', label: '[' },
    { code: 'BracketRight', label: ']' },
    { code: 'Backslash', label: '\\', width: 1.5 },
  ],
  [
    { code: 'CapsLock', label: 'Caps', width: 1.75 },
    ...letters('ASDFGHJKL'),
    { code: 'Semicolon', label: ';' },
    { code: 'Quote', label: "'" },
    { code: 'Enter', label: 'Enter', width: 2.25 },
  ],
  [
    { code: 'ShiftLeft', label: 'Shift', width: 2.25 },
    ...letters('ZXCVBNM'),
    { code: 'Comma', label: ',' },
    { code: 'Period', label: '.' },
    { code: 'Slash', label: '/' },
    { code: 'ShiftRight', label: 'Shift', width: 2.75 },
  ],
  [
    { code: 'ControlLeft', label: 'Ctrl', width: 1.5 },
    { code: 'MetaLeft', label: 'Win', width: 1.25 },
    { code: 'AltLeft', label: 'Alt', width: 1.25 },
    { code: 'Space', label: 'Space', width: 6 },
    { code: 'AltRight', label: 'Alt', width: 1.25 },
    { code: 'ArrowLeft', label: 'Left', width: 0.9375 },
    { code: 'ArrowUp', label: 'Up', width: 0.9375 },
    { code: 'ArrowDown', label: 'Down', width: 0.9375 },
    { code: 'ArrowRight', label: 'Right', width: 0.9375 },
  ],
];
