import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after } from "node:test";

const made: string[] = [];

after(() => {
  for (const folder of made) {
    rmSync(folder, { recursive: true, force: true });
  }
});

export const scratchFolder = (prefix: string) => {
  const folder = mkdtempSync(join(tmpdir(), prefix));

  made.push(folder);

  return folder;
};
