export type Result<Value> = { ok: true; value: Value } | { ok: false; error: unknown };

export const settle = async <Value>(work: () => Value | PromiseLike<Value>): Promise<Result<Value>> =>
  new Promise<Value>((resolve) => {
    resolve(work());
  }).then(
    (value): Result<Value> => ({ ok: true, value }),
    (error: unknown): Result<Value> => ({ ok: false, error }),
  );
