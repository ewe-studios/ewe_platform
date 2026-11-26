// A mounted view owns its loader and recovery attempts.
export class ViewRecovery {
  constructor(load, install, {maxAttempts = 3, onError = () => {}} = {}) {
    if (!Number.isSafeInteger(maxAttempts) || maxAttempts < 1) throw new RangeError("Invalid recovery attempt limit");
    this.load = load;
    this.install = install;
    this.maxAttempts = maxAttempts;
    this.onError = onError;
    this.attempts = 0;
    this.generation = 0;
    this.active = true;
    this.ready = false;
    this.expiredStreams = new Set();
  }

  open() { return this.run(); }

  expire(stream) {
    if (!this.active) return;
    this.expiredStreams.add(stream);
    if (this.flight) return this.flight;
    if (this.attempts >= this.maxAttempts) {
      this.ready = false;
      this.onError(new Error("View recovery attempt limit reached"));
      return;
    }
    this.attempts++;
    return this.run();
  }

  retry() {
    if (this.flight) return this.flight;
    this.attempts = 0;
    return this.run();
  }

  run() {
    if (!this.active || this.flight) return this.flight;
    const generation = ++this.generation;
    this.ready = false;
    const controller = new AbortController();
    this.abort = controller;
    this.flight = Promise.resolve().then(() => this.load(controller.signal)).then(bytes => {
      if (!this.active || this.generation !== generation) return;
      this.ready = true;
      this.install(bytes, this.expiredStreams);
      this.expiredStreams.clear();
    }).catch(error => {
      if (!this.active || this.generation !== generation) return;
      this.ready = false;
      this.onError(error);
      throw error;
    }).finally(() => { if (this.generation === generation) this.flight = null; });
    return this.flight;
  }

  accepts(generation) { return this.active && this.ready && generation === this.generation; }

  close() {
    this.active = false;
    this.ready = false;
    this.generation++;
    this.abort?.abort();
    this.expiredStreams.clear();
  }
}
