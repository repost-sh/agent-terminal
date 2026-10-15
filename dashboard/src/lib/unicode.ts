import type { IUnicodeVersionProvider } from "@xterm/xterm";

import { UNICODE } from "./widths/version";
import { WIDE } from "./widths/wide";
import { ZERO } from "./widths/zero";

type Ranges = readonly (readonly [number, number])[];

const within = (ranges: Ranges, codepoint: number): boolean => {
  let low = 0;
  let high = ranges.length - 1;

  while (low <= high) {
    const middle = (low + high) >> 1;
    const range = ranges[middle];

    if (!range) {
      return false;
    }

    if (codepoint < range[0]) {
      high = middle - 1;
    } else if (codepoint > range[1]) {
      low = middle + 1;
    } else {
      return true;
    }
  }

  return false;
};

const searched = (codepoint: number): 0 | 1 | 2 => (within(ZERO, codepoint) ? 0 : within(WIDE, codepoint) ? 2 : 1);

const BASIC_PLANE = Uint8Array.from({ length: 0x10000 }, (_, codepoint) => searched(codepoint));

const wcwidth = (codepoint: number): 0 | 1 | 2 => {
  const width = BASIC_PLANE[codepoint];

  return width === 0 || width === 1 || width === 2 ? width : searched(codepoint);
};

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

      if (before === 0) {
        joins = false;
      } else if (before > width) {
        width = before;
      }
    }

    return propertiesOf(width, joins);
  },
};
