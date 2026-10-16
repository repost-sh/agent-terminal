import type { IUnicodeVersionProvider } from "@xterm/xterm";

import { UNICODE } from "./widths/version";
import { WIDE } from "./widths/wide";
import { ZERO } from "./widths/zero";

const WIDTHS = new Uint8Array(0x110000).fill(1);

for (const [start, end] of WIDE) {
  WIDTHS.fill(2, start, end + 1);
}

for (const [start, end] of ZERO) {
  WIDTHS.fill(0, start, end + 1);
}

const wcwidth = (codepoint: number): 0 | 1 | 2 => {
  const width = WIDTHS[codepoint];

  return width === 0 ? 0 : width === 2 ? 2 : 1;
};

const widthOf = (properties: number) => (properties >> 1) & 3;

const propertiesOf = (width: number, joins: boolean) => ((width & 3) << 1) | (joins ? 1 : 0);

export const daemonWidths: IUnicodeVersionProvider = {
  version: UNICODE,
  wcwidth,
  charProperties: (codepoint, preceding) => {
    const width = wcwidth(codepoint);
    const before = widthOf(preceding);
    const joins = width === 0 && before !== 0;

    return propertiesOf(joins ? before : width, joins);
  },
};
