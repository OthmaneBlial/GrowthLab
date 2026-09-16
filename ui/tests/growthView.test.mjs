import assert from "node:assert/strict";
import test from "node:test";
import { variantRun, selectedVariant, runPhase } from "../src/growth/view.ts";

test("a live checkpoint never replaces a sealed variant or becomes a sealed result", () => {
  const sealed = { variantId: "v", status: "failed" };
  const live = { variantId: "v", status: "done" };
  const status = { runs: [{ run: sealed, archiveDigest: "sealed-digest" }], attempts: [{ run: live, checkpointDigest: "live-digest" }] };
  assert.deepEqual(variantRun(status, "v"), { run: sealed, sealed: true, digest: "sealed-digest" });
  assert.deepEqual(variantRun({ ...status, runs: [] }, "v"), { run: live, sealed: false, digest: "live-digest" });
  assert.equal(variantRun(status, "other"), null);
});

test("delivery failures and pending intentions cannot change the last completed selection", () => {
  const selections = [
    { variantId: "a", action: "select", status: "done" },
    { variantId: "b", action: "select", status: "done" },
    { variantId: "a", action: "apply", status: "done" },
    { variantId: "c", action: "select", status: "failed" },
    { variantId: "c", action: "select", status: "pending" },
  ];
  assert.equal(selectedVariant({ selections }), "b");
  assert.equal(selectedVariant({ selections: selections.slice(2) }), null);
});

test("run phases distinguish registered checks from proposal preparation and terminal outcomes", () => {
  assert.equal(runPhase(), "Prepared");
  assert.equal(runPhase({ status: "running", candidateCommit: null }), "Preparing proposal");
  assert.equal(runPhase({ status: "running", candidateCommit: "commit", activeValidation: { commandIndex: 2 } }), "Checking command 3");
  assert.equal(runPhase({ status: "failed", activeValidation: { commandIndex: 2 } }), "Failed");
});
