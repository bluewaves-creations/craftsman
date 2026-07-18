// The mutate-gate e2e target (cli/tests/mutate.rs): boundary mutants
// (< -> <=, > -> >=) survive the deliberately weak test in calc.test.ts.
export function clamp(n: number, lo: number, hi: number): number {
  if (n < lo) {
    return lo;
  }
  if (n > hi) {
    return hi;
  }
  return n;
}
