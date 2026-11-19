import { expect, test } from "bun:test";
import { randomBytes } from "node:crypto";
import { decode, encode, key, sizePrefix } from "./lexidecimal";

test("Canonical vectors, prefix boundaries, arbitrary precision, descending viewpoints", () => {
  for (const [n, wire] of [[0n,"0zg0"],[16n,"0zh10"],[-1n,"0yvf"],[-17n,"0yuef"],[-257n,"0yteff"]] as const) {
    expect(encode(n)).toBe(wire);
    expect(decode(wire)).toBe(n);
  }
  const values = new Set<bigint>([0n]);
  for (const bits of [4n,8n,64n,68n,256n,1024n,4096n,262144n]) {
    const n = 1n << bits;
    for (const x of [n-1n,n,n+1n]) { values.add(x); values.add(-x); }
  }
  for (let i=0; i<256; i++) values.add(BigInt(`0x${randomBytes(256).toString("hex")}`) * (i%2 ? 1n : -1n));
  const ordered = [...values].sort((a,b) => a < b ? -1 : a > b ? 1 : 0);
  expect(ordered.map(encode).sort()).toEqual(ordered.map(encode));
  for (const n of ordered) expect(decode(encode(n))).toBe(n);
  const viewpoints = ordered.filter(n => n >= 0n);
  expect(viewpoints.map(key).sort()).toEqual(viewpoints.map(key).reverse());
});

test("reject noncanonical encodings", () => {
  for (const bad of ["", "0x000", "0z00", "0z0000", "0z010", "0z1000", "0z0100", "0yggg", "0z00F", "0zg", "0zg00", "0zwgg0", "0zwgh10", "0yZvvf", "0yWvvvvvvvvvvvvvvvvf", "0zZgg0", "0ywggf", "0zgF"])
    expect(() => decode(bad)).toThrow();
});

test("all size powers without allocating their implied payloads", () => {
  for(const [size,positive,negative] of [[0,"g","v"],[15,"v","g"],[16,"whg","Zuv"],[255,"wvv","Zgg"],[256,"xghgg","Yvuvv"],[65535,"xvvvv","Ygggg"],[65536,"yggghgggg","Xvvvuvvvv"],[0x100000000,"zggggggghgggggggg","Wvvvvvvvuvvvvvvvv"]] as const){expect(sizePrefix(size)).toBe(positive);expect(sizePrefix(size,true)).toBe(negative);}
});
