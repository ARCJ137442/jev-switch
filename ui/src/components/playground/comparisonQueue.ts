export type QueueStatus = 'queued' | 'loading' | 'ok' | 'error' | 'cancelled';

export interface QueueJob<Payload, Result> {
  key: string;
  attemptId: string;
  payload: Payload;
  execute: (signal: AbortSignal) => Promise<Result>;
}

export interface QueueCallbacks<Payload, Result> {
  onStatus: (job: QueueJob<Payload, Result>, status: QueueStatus, value?: Result | Error) => void;
}

interface Entry<Payload, Result> {
  job: QueueJob<Payload, Result>;
  status: QueueStatus;
  controller?: AbortController;
}

/** Bounded scheduler with per-key attempt fencing and immediate queue cancellation. */
export class ComparisonQueue<Payload, Result> {
  private readonly waiting: Entry<Payload, Result>[] = [];
  private readonly active = new Map<string, Entry<Payload, Result>>();
  private readonly latestAttempt = new Map<string, string>();
  private disposed = false;

  constructor(private readonly limit: number, private readonly callbacks: QueueCallbacks<Payload, Result>) {}

  get activeCount(): number { return this.active.size; }
  get pendingCount(): number { return this.waiting.length; }

  enqueue(job: QueueJob<Payload, Result>): void {
    if (this.disposed) return;
    this.latestAttempt.set(job.key, job.attemptId);
    const entry: Entry<Payload, Result> = { job, status: 'queued' };
    this.waiting.push(entry);
    this.callbacks.onStatus(job, 'queued');
    this.pump();
  }

  cancel(key: string, attemptId: string): void {
    const pendingIndex = this.waiting.findIndex((entry) => entry.job.key === key && entry.job.attemptId === attemptId);
    if (pendingIndex >= 0) {
      const [entry] = this.waiting.splice(pendingIndex, 1);
      entry.status = 'cancelled';
      this.callbacks.onStatus(entry.job, 'cancelled');
    }
    const running = this.active.get(attemptId);
    if (running && running.job.key === key && running.status === 'loading') {
      running.status = 'cancelled';
      running.controller?.abort();
      this.callbacks.onStatus(running.job, 'cancelled');
    }
  }

  cancelAll(): void {
    for (const entry of this.waiting.splice(0)) {
      entry.status = 'cancelled';
      this.callbacks.onStatus(entry.job, 'cancelled');
    }
    for (const entry of this.active.values()) {
      if (entry.status !== 'loading') continue;
      entry.status = 'cancelled';
      entry.controller?.abort();
      this.callbacks.onStatus(entry.job, 'cancelled');
    }
  }

  dispose(): void {
    this.disposed = true;
    this.cancelAll();
  }

  private pump(): void {
    while (!this.disposed && this.active.size < this.limit && this.waiting.length > 0) {
      const entry = this.waiting.shift()!;
      if (!this.isCurrent(entry) || entry.status !== 'queued') continue;
      entry.status = 'loading';
      entry.controller = new AbortController();
      this.active.set(entry.job.attemptId, entry);
      this.callbacks.onStatus(entry.job, 'loading');
      void entry.job.execute(entry.controller.signal).then((result) => {
        if (this.isCurrent(entry) && entry.status === 'loading') {
          entry.status = 'ok';
          this.callbacks.onStatus(entry.job, 'ok', result);
        }
      }).catch((error: unknown) => {
        if (this.isCurrent(entry) && entry.status === 'loading') {
          entry.status = 'error';
          this.callbacks.onStatus(entry.job, 'error', error instanceof Error ? error : new Error(String(error)));
        }
      }).finally(() => {
        this.active.delete(entry.job.attemptId);
        this.pump();
      });
    }
  }

  private isCurrent(entry: Entry<Payload, Result>): boolean {
    return this.latestAttempt.get(entry.job.key) === entry.job.attemptId;
  }
}
