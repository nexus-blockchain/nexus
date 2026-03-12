function isPlainObject(value: unknown): value is Record<string, unknown> {
  return value != null && typeof value === 'object' && !Array.isArray(value);
}

export function normalizeIdentifier(value: string): string {
  return value.replace(/[^a-zA-Z0-9]/g, '').toLowerCase();
}

export function codecToJson<T = unknown>(value: any): T {
  if (value && typeof value.toJSON === 'function') {
    return value.toJSON() as T;
  }
  return value as T;
}

export function codecToHuman<T = unknown>(value: any): T {
  if (value && typeof value.toHuman === 'function') {
    return value.toHuman() as T;
  }
  return value as T;
}

export function readObjectField(record: unknown, ...candidates: string[]): unknown {
  if (!isPlainObject(record)) {
    return undefined;
  }

  for (const candidate of candidates) {
    if (candidate in record) {
      return record[candidate];
    }

    const normalized = normalizeIdentifier(candidate);
    const matchedKey = Object.keys(record).find((key) => normalizeIdentifier(key) === normalized);
    if (matchedKey) {
      return record[matchedKey];
    }
  }

  return undefined;
}

export function coerceNumber(value: unknown): number | undefined {
  if (value == null) {
    return undefined;
  }
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === 'bigint') {
    return Number(value);
  }
  if (typeof value === 'string') {
    const cleaned = value.replace(/,/g, '').trim();
    if (!cleaned) {
      return undefined;
    }
    const parsed = Number(cleaned);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}

export function decodeTextValue(value: unknown): string | undefined {
  if (value == null) {
    return undefined;
  }
  if (typeof value === 'string') {
    if (value.startsWith('0x') && value.length % 2 === 0) {
      try {
        return Buffer.from(value.slice(2), 'hex').toString('utf8');
      } catch {
        return value;
      }
    }
    return value;
  }
  if (Array.isArray(value) && value.every((item) => typeof item === 'number')) {
    return new TextDecoder().decode(Uint8Array.from(value));
  }
  return undefined;
}

export function describeValue(value: unknown): string {
  if (typeof value === 'string') {
    return value;
  }
  return JSON.stringify(value);
}
