// Runs on every page, including ones swapped in without a reload.

import { initHome } from './home';

function initDocsFilter(signal: AbortSignal) {
  const input = document.getElementById('docs-filter') as HTMLInputElement | null;
  input?.addEventListener(
    'input',
    () => {
      const query = input.value.toLowerCase().trim();
      for (const group of document.querySelectorAll<HTMLElement>('.docs-group')) {
        let visible = false;
        for (const link of group.querySelectorAll<HTMLElement>('a')) {
          const match = !query || (link.dataset.title ?? '').includes(query);
          link.hidden = !match;
          visible ||= match;
        }
        group.hidden = !visible;
      }
    },
    { signal },
  );
}

let page = new AbortController();

document.addEventListener('astro:page-load', () => {
  page.abort();
  page = new AbortController();
  if (document.getElementById('stage')) initHome(page.signal);
  initDocsFilter(page.signal);
});
