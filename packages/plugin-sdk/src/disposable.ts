export interface Disposable {
  dispose(): void | Promise<void>;
}

export interface SubscriptionStore {
  add<T extends Disposable>(disposable: T): T;
}

export function createSubscriptionStore(): SubscriptionStore {
  const disposables: Disposable[] = [];
  return {
    add<T extends Disposable>(d: T): T {
      disposables.push(d);
      return d;
    },
    // Called by bootstrap on deactivate — LIFO order
    async disposeAll(): Promise<void> {
      for (let i = disposables.length - 1; i >= 0; i--) {
        try {
          await disposables[i].dispose();
        } catch {
          // continue cleanup
        }
      }
      disposables.length = 0;
    },
  };
}
