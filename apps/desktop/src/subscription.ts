/** Dispose even when an asynchronous registration finishes after unmount. */
export function subscription(
  registration: Promise<() => void>,
  onError: (reason: unknown) => void = console.error,
): () => void {
  let disposed = false;
  let stop: (() => void) | undefined;
  void registration.then((unlisten) => {
    if (disposed) unlisten();
    else stop = unlisten;
  }).catch((reason) => { if (!disposed) onError(reason); });
  return () => { disposed = true; stop?.(); };
}
