import { decode, encode } from "./lexidecimal";

export function key(milestone: bigint): string {
  if (milestone < 1n) throw new Error("Milestone starts at one");
  const result = `kernelspace/datashed/timeline/${encode(~milestone)}`;
  if (result.length > 1024) throw new Error("S3 key too long");
  return result;
}

export function fromKey(key: string): bigint {
  if (!key.startsWith("kernelspace/datashed/timeline/")) throw new Error("invalid Timeline prefix");
  const milestone = ~decode(key.slice("kernelspace/datashed/timeline/".length));
  if (key !== `kernelspace/datashed/timeline/${encode(~milestone)}` || milestone < 1n)
    throw new Error("invalid Milestone");
  if (key.length > 1024) throw new Error("S3 key too long");
  return milestone;
}
