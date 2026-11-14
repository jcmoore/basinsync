import { readFileSync, readdirSync, realpathSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(import.meta.url);
export const defaultDirectory = resolve(dirname(script), "../../../diy");

// Positive Lexidecimal: width-of-size, size-minus-one, then big-endian hex.
export function encodeDepth(depth) {
  if (typeof depth !== "bigint" || depth < 0n) throw new Error("depth must be nonnegative");
  const hex = depth.toString(16);
  const size = (hex.length - 1).toString(16);
  const width = (size.length - 1).toString(16);
  if (width.length !== 1) throw new Error("depth allocation is too large");
  return `0z${width}${size}${hex}`;
}

export function decodeDepth(encoded) {
  if (!/^0z[0-9a-f]{3,}$/.test(encoded)) throw new Error("depth must be unsigned Lexidecimal");
  const body = encoded.slice(2);
  const end = 2 + Number.parseInt(body[0], 16);
  const size = body.slice(1, end);
  const hex = body.slice(end);
  if (!size || !hex || BigInt(`0x${size}`) !== BigInt(hex.length - 1)) {
    throw new Error("incorrect Lexidecimal size");
  }
  const depth = BigInt(`0x${hex}`);
  if (encodeDepth(depth) !== encoded) throw new Error("noncanonical Lexidecimal depth");
  return depth;
}

// An intentionally small YAML schema: prerequisites: [] or an unquoted block list.
function prerequisites(text) {
  const lines = text.replace(/^\uFEFF/, "").replace(/\r\n/g, "\n").split("\n");
  if (lines[0] !== "---") throw new Error("missing frontmatter");
  const end = lines.indexOf("---", 1);
  if (end < 0) throw new Error("unterminated frontmatter");
  const fields = lines.slice(1, end).filter(line => line.trim() && !line.trimStart().startsWith("#"));
  if (fields[0] === "prerequisites: []" && fields.length === 1) return [];
  if (fields[0] !== "prerequisites:" || fields.length === 1) {
    throw new Error("expected prerequisites: [] or prerequisites: with a block list");
  }
  return fields.slice(1).map(line => {
    const item = /^  - ([a-z0-9]+(?:-[a-z0-9]+)*\.md)$/.exec(line);
    if (!item) throw new Error("prerequisites must be unquoted sibling Markdown filenames, one per list item");
    return item[1];
  });
}

export function lintTutorials(directory) {
  const errors = [];
  const graph = new Map();
  const suffixes = new Map();
  for (const entry of readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name < b.name ? -1 : 1)) {
    if (!/\.md$/i.test(entry.name)) continue;
    if (!entry.isFile()) {
      errors.push(`${entry.name}: tutorials must be regular files`);
      continue;
    }
    let depth = null;
    let dependencies = [];
    try {
      const match = /^(0z[0-9a-f]+)-([a-z0-9]+(?:-[a-z0-9]+)*)\.md$/.exec(entry.name);
      if (!match) throw new Error("expected <unsigned-Lexidecimal-depth>-<lowercase-slug>.md");
      const previous = suffixes.get(match[2]);
      if (previous) {
        errors.push(`${entry.name}: duplicate tutorial suffix "${match[2]}" (also ${previous})`);
      } else {
        suffixes.set(match[2], entry.name);
      }
      depth = decodeDepth(match[1]);
    } catch (error) {
      errors.push(`${entry.name}: ${error.message}`);
    }
    try {
      dependencies = prerequisites(readFileSync(resolve(directory, entry.name), "utf8"));
    } catch (error) {
      errors.push(`${entry.name}: ${error.message}`);
    }
    graph.set(entry.name, { depth, dependencies });
  }
  if (!graph.size) errors.push("no tutorials found");

  const dependents = new Map([...graph.keys()].map(name => [name, []]));
  const indegree = new Map([...graph.keys()].map(name => [name, 0]));
  let edges = 0;
  for (const [name, node] of graph) {
    const seen = new Set();
    for (const dependency of node.dependencies) {
      if (seen.has(dependency)) {
        errors.push(`${name}: duplicate prerequisite ${dependency}`);
        continue;
      }
      seen.add(dependency);
      if (dependency === name) errors.push(`${name}: self prerequisite`);
      const target = graph.get(dependency);
      if (!target) {
        errors.push(`${name}: missing prerequisite ${dependency} (filenames are case-sensitive)`);
        continue;
      }
      edges++;
      dependents.get(dependency).push(name);
      indegree.set(name, indegree.get(name) + 1);
      if (node.depth !== null && target.depth !== null && node.depth <= target.depth) {
        errors.push(`${name}: depth ${node.depth} must exceed ${dependency}'s depth ${target.depth}`);
      }
    }
  }
  // Kahn's algorithm also detects cycles independently of the depth check.
  const ready = [...indegree].filter(([, count]) => count === 0).map(([name]) => name);
  for (let index = 0; index < ready.length; index++) {
    for (const name of dependents.get(ready[index])) {
      indegree.set(name, indegree.get(name) - 1);
      if (indegree.get(name) === 0) ready.push(name);
    }
  }
  const blocked = [...indegree].filter(([, count]) => count > 0).map(([name]) => name);
  if (blocked.length) errors.push(`prerequisite cycle, or dependent on one: ${blocked.join(", ")}`);
  return { errors, tutorials: graph.size, edges };
}

if (process.argv[1] && realpathSync(process.argv[1]) === realpathSync(script)) {
  try {
    if (process.argv.length > 3) throw new Error("usage: check-dag [tutorial-directory]");
    const result = lintTutorials(process.argv[2] ? resolve(process.argv[2]) : defaultDirectory);
    if (result.errors.length) {
      console.error(result.errors.join("\n"));
      process.exitCode = 1;
    } else {
      console.log(`Tutorial DAG OK: ${result.tutorials} tutorials, ${result.edges} prerequisites.`);
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
