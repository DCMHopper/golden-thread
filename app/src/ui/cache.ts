export class LruCache<K, V> {
  private readonly max: number;
  private readonly map = new Map<K, V>();

  constructor(max: number) {
    this.max = Math.max(1, max);
  }

  has(key: K): boolean {
    return this.map.has(key);
  }

  get(key: K): V | undefined {
    const value = this.map.get(key);
    if (value === undefined) return undefined;
    this.map.delete(key);
    this.map.set(key, value);
    return value;
  }

  set(key: K, value: V): void {
    if (this.map.has(key)) {
      this.map.delete(key);
    }
    this.map.set(key, value);
    if (this.map.size > this.max) {
      const oldest = this.map.keys().next().value;
      if (oldest !== undefined) {
        this.map.delete(oldest);
      }
    }
  }

  delete(key: K): boolean {
    return this.map.delete(key);
  }

  clear(): void {
    this.map.clear();
  }

  forEach(callback: (value: V, key: K) => void): void {
    this.map.forEach((value, key) => callback(value, key));
  }
}

export class WeightedLruCache<K, V> {
  private readonly maxWeight: number;
  private readonly weightOf: (value: V) => number;
  private readonly map = new Map<K, { value: V; weight: number }>();
  private total = 0;

  constructor(maxWeight: number, weightOf: (value: V) => number) {
    this.maxWeight = Math.max(1, maxWeight);
    this.weightOf = weightOf;
  }

  has(key: K): boolean {
    return this.map.has(key);
  }

  get(key: K): V | undefined {
    const entry = this.map.get(key);
    if (!entry) return undefined;
    this.map.delete(key);
    this.map.set(key, entry);
    return entry.value;
  }

  set(key: K, value: V): void {
    if (this.map.has(key)) {
      this.delete(key);
    }
    const weight = Math.max(1, this.weightOf(value));
    this.map.set(key, { value, weight });
    this.total += weight;
    this.evict();
  }

  delete(key: K): boolean {
    const entry = this.map.get(key);
    if (!entry) return false;
    this.total -= entry.weight;
    this.map.delete(key);
    return true;
  }

  clear(): void {
    this.map.clear();
    this.total = 0;
  }

  forEach(callback: (value: V, key: K) => void): void {
    this.map.forEach((entry, key) => callback(entry.value, key));
  }

  private evict(): void {
    while (this.total > this.maxWeight && this.map.size > 1) {
      const oldest = this.map.keys().next().value;
      if (oldest === undefined) return;
      this.delete(oldest);
    }
  }
}
