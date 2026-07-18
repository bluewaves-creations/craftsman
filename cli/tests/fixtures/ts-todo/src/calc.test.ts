// Deliberately WEAK test (mutate e2e): only the pass-through path.
import { expect, test } from "bun:test";
import { clamp } from "./calc";

test("clamp passes a mid value through", () => {
  expect(clamp(5, 0, 10)).toBe(5);
});
