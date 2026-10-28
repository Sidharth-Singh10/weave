import { test } from "node:test";
import assert from "node:assert/strict";

import { zipSync } from "./zip.ts";

const startsWith = (bytes: Uint8Array, signature: number[]) =>
  signature.every((byte, index) => bytes[index] === byte);

test("produces a local file header and an end-of-central-directory record", () => {
  const zip = zipSync([{ path: "a.md", content: "hello" }]);

  assert.ok(startsWith(zip, [0x50, 0x4b, 0x03, 0x04]), "local header missing");
  const eocd = zip.length - 22;
  assert.ok(
    startsWith(zip.subarray(eocd), [0x50, 0x4b, 0x05, 0x06]),
    "end of central directory missing"
  );
});

test("stores UTF-8 paths and content", () => {
  const zip = zipSync([{ path: "Nodes/Pratibha.md", content: "héllo" }]);
  const text = new TextDecoder().decode(zip);

  assert.ok(text.includes("Nodes/Pratibha.md"));
  assert.ok(text.includes("héllo"));
});

test("is deterministic: identical input yields identical bytes", () => {
  const entries = [
    { path: "b.md", content: "two" },
    { path: "a.md", content: "one" },
  ];
  assert.deepEqual(Array.from(zipSync(entries)), Array.from(zipSync(entries)));
});

test("handles an empty archive", () => {
  const zip = zipSync([]);
  // EOCD only: 22 bytes, zero entries.
  assert.equal(zip.length, 22);
  assert.ok(startsWith(zip, [0x50, 0x4b, 0x05, 0x06]));
});
