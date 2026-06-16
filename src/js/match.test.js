// Unit tests for the pure matching core. Run: `node --test src/js/match.test.js`
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  fuzzySubstring,
  threshold,
  convert,
  candidates,
  matches,
} from "./match.js";

test("fuzzySubstring: prefix costs nothing", () => {
  assert.equal(fuzzySubstring("grow", "growpay"), 0);
  assert.equal(fuzzySubstring("pay", "growpay"), 0); // interior substring
});

test("fuzzySubstring: single substitution costs one", () => {
  assert.equal(fuzzySubstring("grokpay", "growpay"), 1);
  assert.equal(fuzzySubstring("drowpay", "growpay"), 1);
});

test("fuzzySubstring: empty query is free, empty text is full cost", () => {
  assert.equal(fuzzySubstring("", "growpay"), 0);
  assert.equal(fuzzySubstring("abc", ""), 3);
});

test("threshold scales with length", () => {
  assert.equal(threshold(3), 0);
  assert.equal(threshold(4), 1);
  assert.equal(threshold(7), 1);
  assert.equal(threshold(8), 2);
});

test("convert remaps known chars, passes others through", () => {
  const map = { п: "g", к: "r", щ: "o", ц: "w", з: "p", ф: "a", н: "y" };
  assert.equal(convert("пкщцзфн", map), "growpay");
  assert.equal(convert("пк-x", map), "gr-x");
});

test("candidates includes raw, both directions, deduped", () => {
  const map = { а: "f", ф: "a" };
  const c = candidates("ф", [{ id: "ru", map }]);
  assert.ok(c.includes("ф")); // raw
  assert.ok(c.includes("a")); // script -> latin
  // inverse maps latin 'a' -> 'ф', so converting 'ф' through the inverse is a no-op here;
  // the key assertion is no duplicates and raw is preserved.
  assert.equal(new Set(c).size, c.length);
});

test("matches: exact substring", () => {
  assert.ok(matches("pay", "growpay", []));
  assert.ok(matches("GROW", "growpay", [])); // case-insensitive
  assert.ok(!matches("zzz", "growpay", []));
});

test("matches: typo tolerance via fuzzy", () => {
  assert.ok(matches("grokpay", "growpay", []));
  assert.ok(matches("drowpay", "growpay", []));
});

test("matches: short queries stay exact (no fuzzy noise)", () => {
  assert.ok(!matches("xy", "growpay", [])); // 2 chars, no exact substring -> no match
  assert.ok(matches("gr", "growpay", [])); // exact substring still matches
});

test("matches: empty query matches everything", () => {
  assert.ok(matches("", "anything", []));
});

test("matches: layout conversion finds a Latin name typed on a non-Latin layout", () => {
  // ЙЦУКЕН: physical g r o w p a y -> п к щ ц з ф н
  const jcuken = {
    map: { п: "g", к: "r", щ: "o", ц: "w", з: "p", ф: "a", н: "y" },
  };
  assert.ok(matches("пкщцзфн", "growpay", [jcuken]));
});

test("matches: Latin query finds a non-Latin project name (inverse direction)", () => {
  const jcuken = { map: { п: "g", к: "r", щ: "о", о: "j" } };
  // project literally named "пк"; user types latin keys "gr" on a latin layout
  assert.ok(matches("gr", "пк", [jcuken]));
});

test("matches: empty maps degrade to raw + fuzzy", () => {
  assert.ok(matches("grokpay", "growpay", []));
  assert.ok(matches("grokpay", "growpay", undefined));
});
