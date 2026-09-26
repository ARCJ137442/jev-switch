/** Whole-document history preserves every route attribute, including future fields. */
export class DocumentHistory<T> {
  private past: T[] = [];
  private future: T[] = [];
  current: T;
  constructor(initial: T, private readonly capacity = 100) { this.current = structuredClone(initial); }
  get canUndo() { return this.past.length > 0; }
  get canRedo() { return this.future.length > 0; }
  reset(value: T): T { this.past = []; this.future = []; return this.current = structuredClone(value); }
  push(value: T): T {
    if (JSON.stringify(value) === JSON.stringify(this.current)) return this.current;
    this.past.push(structuredClone(this.current));
    if (this.past.length > this.capacity) this.past.shift();
    this.future = [];
    return this.current = structuredClone(value);
  }
  undo(): T {
    const previous = this.past.pop();
    if (previous !== undefined) { this.future.push(structuredClone(this.current)); this.current = previous; }
    return this.current;
  }
  redo(): T {
    const next = this.future.pop();
    if (next !== undefined) { this.past.push(structuredClone(this.current)); this.current = next; }
    return this.current;
  }
}
