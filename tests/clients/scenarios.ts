// Independent scenarios: one failure is recorded and the suite moves on, so a
// missing feature fails only its own scenarios. Tags name the tutorial that
// introduced the behavior, letting reports separate skipped scope from bugs.
type Outcome = { tag: string; label: string; ok: boolean; error?: string };
const outcomes: Outcome[] = [];

export async function scenario(tag: string, label: string, run: () => Promise<void>): Promise<void> {
  try {
    await run();
    outcomes.push({ tag, label, ok: true });
    console.log(`PASS [${tag}] ${label}`);
  } catch (error) {
    const message = error instanceof Error ? (error.stack ?? error.message) : String(error);
    outcomes.push({ tag, label, ok: false, error: message.split("\n").slice(0, 3).join(" | ") });
    console.log(`FAIL [${tag}] ${label}\n  ${message.split("\n").slice(0, 6).join("\n  ")}`);
  }
}

/** Record shared, unscoped setup that ran to completion as a passing scenario. */
export function passed(tag: string, label: string): void {
  outcomes.push({ tag, label, ok: true });
  console.log(`PASS [${tag}] ${label}`);
}

/** Print a machine-readable summary; returns false if any scenario failed. */
export function report(): boolean {
  const failed = outcomes.filter((o) => !o.ok).length;
  console.log(`SCENARIOS ${JSON.stringify({ passed: outcomes.length - failed, failed, outcomes })}`);
  return failed === 0;
}
