import { expect, test } from "bun:test";
import { randomBytes } from "node:crypto";
import { key, fromKey } from "./timeline";
import { encode } from "./lexidecimal";

test("Timeline keys complement positive Milestones and list newest first", () => {
  const milestones = new Set<bigint>([1n]);
  for (const bits of [4n, 8n, 64n, 68n, 256n, 1024n]) {
    const n = 1n << bits;
    for (const m of [n - 1n, n, n + 1n]) milestones.add(m);
  }
  for (let i = 0; i < 256; i++)
    milestones.add(BigInt(`0x${randomBytes(200).toString("hex")}`) + 1n);
  const ordered = [...milestones].sort((a, b) => a < b ? -1 : a > b ? 1 : 0);
  for (const m of ordered) {
    expect(key(m)).toBe(`kernelspace/basin/timeline/${encode(~m)}`);
    expect(fromKey(key(m))).toBe(m);
  }
  expect(ordered.map(key).sort()).toEqual(ordered.map(key).reverse());
  for (const [n, wire] of [[1n, "0yve"], [15n, "0yv0"], [16n, "0yuef"], [256n, "0yteff"]] as const)
    expect(key(n)).toBe(`kernelspace/basin/timeline/${wire}`);
});

test("Timeline rejects zero, wrong signs/prefixes, noncanonical and oversized keys", () => {
  for (const m of [0n, -1n, 1n << 4096n]) expect(() => key(m)).toThrow();
  for (const k of ["kernelspace/basin/timeline/0yvf", "kernelspace/basin/timeline/0zg1", "kernelspace/basin/timeline/0w1z", "kernelspace/basin/timeline/0yFFE", "kernelspace/basin/timeline/0yve/extra", "kernelspace/basin/epilogue/0yve", `kernelspace/basin/timeline/${encode(~(1n << 4096n))}`])
    expect(() => fromKey(k)).toThrow();
});
