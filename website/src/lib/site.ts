export const REPO = 'https://github.com/enekocm7/cube-app';
export const RELEASES = `${REPO}/releases/latest`;

/** The sticker that marks each docs section, in reading order. */
export const DOC_SECTIONS: Record<string, string> = {
  'Start here': 'green',
  Timing: 'red',
  History: 'yellow',
  Reference: 'blue',
};

export const docSlug = (id: string) => id.replace(/\.md$/, '');
