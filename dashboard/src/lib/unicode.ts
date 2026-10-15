import type { IUnicodeVersionProvider } from "@xterm/xterm";

import { UNICODE, WIDE, ZERO } from "./widths";

type Ranges = readonly (readonly [number, number])[];

const within = (ranges: Ranges, codepoint: number): boolean => {
  let low = 0;
  let high = ranges.length - 1;
  while (low <= high) {
    const middle = (low + high) >> 1;
    const range = ranges[middle];
    if (!range) return false;
    if (codepoint < range[0]) high = middle - 1;
    else if (codepoint > range[1]) low = middle + 1;
    else return true;
  }
  return false;
};

const wcwidth = (codepoint: number): 0 | 1 | 2 => (within(ZERO, codepoint) ? 0 : within(WIDE, codepoint) ? 2 : 1);

const widthOf = (properties: number) => (properties >> 1) & 3;

const propertiesOf = (width: number, joins: boolean) => ((width & 3) << 1) | (joins ? 1 : 0);

export const daemonWidths: IUnicodeVersionProvider = {
  version: UNICODE,
  wcwidth,
  charProperties: (codepoint, preceding) => {
    let width: number = wcwidth(codepoint);
    let joins = width === 0 && preceding !== 0;
    if (joins) {
      const before = widthOf(preceding);
      if (before === 0) joins = false;
      else if (before > width) width = before;
    }
    return propertiesOf(width, joins);
  },
};
