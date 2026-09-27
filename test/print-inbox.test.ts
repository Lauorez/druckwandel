import { describe, expect, it } from "vitest";
import {
  printJobIdFromDeepLink,
  reconcilePrintInbox,
  shouldLoadPrintJob,
  type PrintJob,
} from "../apps/desktop/src/printInbox.js";

function job(path: string, modifiedMs: number): PrintJob {
  return { path, name: `${path}.pdf`, modifiedMs, size: 100 };
}

describe("print inbox reconciliation", () => {
  it("accepts only the versioned print-job deep link", () => {
    const id = "301a9e41-00d3-4208-a970-6b9809bc2eeb";
    expect(printJobIdFromDeepLink(`erechnung-review://print-job/${id}?source=windows`)).toBe(id);
    expect(printJobIdFromDeepLink(`https://print-job/${id}`)).toBeUndefined();
    expect(printJobIdFromDeepLink(`erechnung-review://other/${id}`)).toBeUndefined();
    expect(printJobIdFromDeepLink("file:///C:/Users/x/invoice.pdf")).toBeUndefined();
    expect(printJobIdFromDeepLink("erechnung-review://print-job/C:/Users/x/secret.pdf")).toBeUndefined();
  });

  it("treats existing files as history on startup", () => {
    const update = reconcilePrintInbox(new Set(), [job("old", 1)], true);
    expect(update.unseenJobsNewestFirst).toEqual([]);
    expect([...update.knownPaths]).toEqual(["old"]);
  });

  it("lets an explicit handoff override the startup baseline", () => {
    const knownPaths = new Set(["requested-job"]);
    expect(shouldLoadPrintJob(knownPaths, "requested-job", false)).toBe(false);
    expect(shouldLoadPrintJob(knownPaths, "requested-job", true)).toBe(true);
  });

  it("selects the newest new job and forgets removed paths", () => {
    const update = reconcilePrintInbox(
      new Set(["old", "removed"]),
      [job("newest", 3), job("newer", 2), job("old", 1)],
      false,
    );
    expect(update.unseenJobsNewestFirst.map((entry) => entry.path)).toEqual(["newest", "newer"]);
    expect([...update.knownPaths]).toEqual(["newest", "newer", "old"]);
  });
});
