// The landing page's timer and keyboard. Pages are swapped without a reload, so
// this runs each time the home page arrives and everything it attaches to the
// window goes away with `signal` when the page leaves.

import { netColors, randomScramble } from '../lib/cube';
import { BINDINGS } from '../lib/keys';

export function initHome(signal: AbortSignal) {
  const on = <K extends keyof WindowEventMap>(type: K, listener: (event: WindowEventMap[K]) => void) =>
    window.addEventListener(type, listener, { signal });

  const HOLD_MS = 300; // cube-core's DEFAULT_HOLD

  const root = document.documentElement;
  const stage = document.getElementById('stage')!;
  const digits = document.getElementById('digits')!;
  const scrambleText = document.getElementById('scramble')!;
  const status = document.getElementById('status')!;
  const hint = document.getElementById('hint')!;
  const rows = document.getElementById('session-rows')!;
  const note = document.getElementById('session-note')!;
  const netCells = document.querySelectorAll('#net rect');

  const touch = matchMedia('(hover: none)').matches;
  hint.innerHTML = touch
    ? 'Touch and hold the digits until they turn green, then let go to start. Tap to stop.'
    : hint.innerHTML;
  hint.hidden = false;

  // The primary button names the visitor's system.
  const system = /Windows/.test(navigator.userAgent)
    ? 'Windows'
    : /Mac/.test(navigator.userAgent)
      ? 'macOS'
      : /Linux/.test(navigator.userAgent) && !/Android/.test(navigator.userAgent)
        ? 'Linux'
        : null;
  if (system) document.getElementById('download-primary')!.textContent = `Download for ${system}`;

  type Phase = 'idle' | 'holding' | 'ready' | 'running' | 'stopped';
  let phase: Phase = 'idle';
  let holdTimer = 0;
  let startedAt = 0;
  let frame = 0;
  const solves: number[] = [];

  function setPhase(next: Phase) {
    phase = next;
    root.dataset.phase = next === 'stopped' ? 'idle' : next;
  }

  function format(ms: number) {
    const minutes = Math.floor(ms / 60000);
    const seconds = Math.floor(ms / 1000) % 60;
    const rest = String(Math.floor(ms % 1000)).padStart(3, '0');
    return minutes > 0
      ? `${minutes}:${String(seconds).padStart(2, '0')}.${rest}`
      : `${seconds}.${rest}`;
  }

  function show(ms: number) {
    const text = format(ms);
    digits.textContent = text;
    digits.classList.toggle('long', text.length > 6);
  }

  // As in cube-core: a mean of three keeps every solve, and longer averages
  // drop the best and the worst.
  function average(times: number[]) {
    const kept = times.length === 3 ? times : [...times].sort((a, b) => a - b).slice(1, -1);
    return kept.reduce((a, b) => a + b, 0) / kept.length;
  }

  function rolling(size: number) {
    const values: number[] = [];
    for (let end = size; end <= solves.length; end++) {
      values.push(average(solves.slice(end - size, end)));
    }
    return values;
  }

  function renderSession() {
    const lines: [string, number[]][] = [
      ['Single', solves],
      ['mo3', rolling(3)],
      ['ao5', rolling(5)],
      ['ao12', rolling(12)],
    ];
    rows.replaceChildren(
      ...lines.map(([name, values]) => {
        const row = document.createElement('tr');
        const label = document.createElement('th');
        label.scope = 'row';
        label.textContent = name;
        const current = document.createElement('td');
        const best = document.createElement('td');
        current.textContent = values.length ? format(values[values.length - 1]) : '–';
        best.textContent = values.length ? format(Math.min(...values)) : '–';
        row.append(label, current, best);
        return row;
      }),
    );
    note.hidden = false;
  }

  function nextScramble() {
    const scramble = randomScramble();
    scrambleText.textContent = scramble;
    const colors = netColors(scramble);
    netCells.forEach((cell, i) => cell.setAttribute('class', `fill-${colors[i]}`));
  }

  function press() {
    if (phase !== 'idle') return;
    setPhase('holding');
    show(0);
    holdTimer = window.setTimeout(() => setPhase('ready'), HOLD_MS);
  }

  function release() {
    window.clearTimeout(holdTimer);
    if (phase === 'holding') {
      setPhase('idle');
    } else if (phase === 'ready') {
      setPhase('running');
      startedAt = performance.now();
      const tick = () => {
        show(performance.now() - startedAt);
        frame = requestAnimationFrame(tick);
      };
      tick();
    } else if (phase === 'stopped') {
      setPhase('idle');
    }
  }

  function stop(record: boolean) {
    cancelAnimationFrame(frame);
    const elapsed = performance.now() - startedAt;
    setPhase('stopped');
    if (record) {
      show(elapsed);
      solves.push(elapsed);
      renderSession();
      nextScramble();
      status.textContent = `Solve recorded: ${format(elapsed)}`;
    } else {
      show(0);
      status.textContent = 'Solve cancelled';
    }
  }

  // Space is only the timer while the timer is on screen; further down it
  // scrolls as usual, and a focused link or button keeps its own Space.
  const timerInView = () => window.scrollY < stage.offsetHeight * 0.5;
  const onControl = (target: EventTarget | null) =>
    target instanceof Element && target.closest('a, button, input, pre') !== null;

  on('keydown', (event) => {
    if (phase === 'running') {
      event.preventDefault();
      if (!event.repeat) stop(event.code !== 'Escape');
      return;
    }
    if (event.code !== 'Space' || !timerInView() || onControl(event.target)) return;
    event.preventDefault();
    if (!event.repeat) press();
  });

  on('keyup', (event) => {
    if (phase === 'stopped') {
      setPhase('idle');
    } else if (event.code === 'Space') {
      release();
    }
  });

  // Touch has no Space key, so it holds the digits. A mouse click never starts
  // the timer.
  digits.addEventListener('pointerdown', (event) => {
    if (phase === 'idle' && event.pointerType === 'touch') {
      event.preventDefault();
      digits.setPointerCapture(event.pointerId);
      press();
    }
  });
  stage.addEventListener('pointerdown', () => {
    if (phase === 'running') stop(true);
  });
  on('pointerup', release);
  on('pointercancel', release);
  on('blur', () => {
    if (phase === 'holding' || phase === 'ready') {
      window.clearTimeout(holdTimer);
      setPhase('idle');
    }
  });

  // The keyboard section answers real key presses while it is on screen.
  const readout = document.getElementById('readout-list')!;
  const keyboard = document.getElementById('keyboard')!;
  let keysInView = false;
  const observer = new IntersectionObserver(([entry]) => (keysInView = entry.isIntersecting), { threshold: 0.3 });
  observer.observe(keyboard);

  function describe(code: string, label: string) {
    const bindings = BINDINGS[code] ?? [[label, 'Free by default. Bind any action to it.']];
    readout.replaceChildren(
      ...bindings.map(([combo, action]) => {
        const line = document.createElement('div');
        const key = document.createElement('dt');
        const text = document.createElement('dd');
        key.textContent = combo;
        text.textContent = action;
        line.append(key, text);
        return line;
      }),
    );
  }

  const capFor = (code: string) => keyboard.querySelector<HTMLElement>(`[data-code="${code}"]`);

  keyboard.addEventListener('pointerover', (event) => {
    const cap = (event.target as Element).closest<HTMLElement>('.cap');
    if (cap) describe(cap.dataset.code!, cap.textContent!);
  });
  keyboard.addEventListener('click', (event) => {
    const cap = (event.target as Element).closest<HTMLElement>('.cap');
    if (cap) describe(cap.dataset.code!, cap.textContent!);
  });

  on('keydown', (event) => {
    if (!keysInView || event.ctrlKey || event.altKey || event.metaKey) return;
    if (event.target instanceof HTMLInputElement) return;
    const cap = capFor(event.code);
    if (!cap) return;
    cap.classList.add('down');
    if (event.code.startsWith('Shift')) return;
    if (event.code === 'F1') event.preventDefault();
    describe(event.code, cap.textContent!);
  });
  on('keyup', (event) => capFor(event.code)?.classList.remove('down'));

  signal.addEventListener('abort', () => {
    observer.disconnect();
    cancelAnimationFrame(frame);
    window.clearTimeout(holdTimer);
  });
}
