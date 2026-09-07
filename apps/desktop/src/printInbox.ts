export interface PrintJob {
  jobId?: string;
  path: string;
  name: string;
  sourceApplication?: string;
  pages?: number;
  modifiedMs: number;
  size: number;
}

export interface PrintInboxUpdate {
  knownPaths: Set<string>;
  unseenJobsNewestFirst: PrintJob[];
}

const PRINT_JOB_LINK = /^erechnung-review:\/\/print-job\/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})(?:[?#].*)?$/i;

export function printJobIdFromDeepLink(value: string): string | undefined {
  return PRINT_JOB_LINK.exec(value)?.[1]?.toLowerCase();
}

export function shouldLoadPrintJob(
  knownPaths: ReadonlySet<string>,
  path: string,
  explicitlyRequested: boolean,
): boolean {
  return explicitlyRequested || !knownPaths.has(path);
}

/**
 * Reconciles the current inbox snapshot with the previous one. The first
 * snapshot is a baseline: files left from earlier sessions are never treated
 * as a newly printed invoice.
 */
export function reconcilePrintInbox(
  previousPaths: ReadonlySet<string>,
  jobsNewestFirst: readonly PrintJob[],
  establishBaseline: boolean,
): PrintInboxUpdate {
  const knownPaths = new Set(jobsNewestFirst.map((job) => job.path));
  const unseenJobsNewestFirst = establishBaseline
    ? []
    : jobsNewestFirst.filter((job) => !previousPaths.has(job.path));
  return { knownPaths, unseenJobsNewestFirst };
}
