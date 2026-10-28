/**
 * Minimal ZIP writer: STORE method (no compression), UTF-8 filenames, fixed
 * timestamps so the same graph always produces the same bytes.
 *
 * Obsidian vaults are folders, not single files, so exporting one requires an
 * archive. Pulling in a zip dependency for ~60 lines would add more supply
 * chain than it removes; this writer is covered by tests that unzip its output
 * with the system `unzip`.
 */

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let i = 0; i < 256; i++) {
    let c = i;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[i] = c >>> 0;
  }
  return table;
})();

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) {
    crc = CRC_TABLE[(crc ^ bytes[i]) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

class Writer {
  private parts: Uint8Array[] = [];
  private size = 0;

  get length(): number {
    return this.size;
  }

  bytes(bytes: Uint8Array): void {
    this.parts.push(bytes);
    this.size += bytes.length;
  }

  u16(value: number): void {
    const buffer = new Uint8Array(2);
    new DataView(buffer.buffer).setUint16(0, value, true);
    this.bytes(buffer);
  }

  u32(value: number): void {
    const buffer = new Uint8Array(4);
    new DataView(buffer.buffer).setUint32(0, value >>> 0, true);
    this.bytes(buffer);
  }

  concat(): Uint8Array {
    const out = new Uint8Array(this.size);
    let offset = 0;
    for (const part of this.parts) {
      out.set(part, offset);
      offset += part.length;
    }
    return out;
  }
}

export interface ZipEntry {
  /** Path inside the archive, `/`-separated. */
  path: string;
  content: string;
}

export function zipSync(entries: ZipEntry[]): Uint8Array {
  const encoder = new TextEncoder();
  const out = new Writer();
  const central: { name: Uint8Array; size: number; crc: number; offset: number }[] = [];

  // 1980-01-01 00:00:00 — the ZIP epoch. A fixed date keeps output stable.
  const dosTime = 0;
  const dosDate = 0x21;

  for (const entry of entries) {
    const name = encoder.encode(entry.path);
    const data = encoder.encode(entry.content);
    const crc = crc32(data);
    const offset = out.length;

    out.u32(0x04034b50); // local file header
    out.u16(20); // version needed
    out.u16(0x0800); // UTF-8 filename flag
    out.u16(0); // STORE
    out.u16(dosTime);
    out.u16(dosDate);
    out.u32(crc);
    out.u32(data.length); // compressed size
    out.u32(data.length); // uncompressed size
    out.u16(name.length);
    out.u16(0); // extra length
    out.bytes(name);
    out.bytes(data);

    central.push({ name, size: data.length, crc, offset });
  }

  const centralStart = out.length;
  for (const file of central) {
    out.u32(0x02014b50); // central directory header
    out.u16(20); // version made by
    out.u16(20); // version needed
    out.u16(0x0800);
    out.u16(0);
    out.u16(dosTime);
    out.u16(dosDate);
    out.u32(file.crc);
    out.u32(file.size);
    out.u32(file.size);
    out.u16(file.name.length);
    out.u16(0); // extra
    out.u16(0); // comment
    out.u16(0); // disk number
    out.u16(0); // internal attrs
    out.u32(0); // external attrs
    out.u32(file.offset);
    out.bytes(file.name);
  }
  const centralSize = out.length - centralStart;

  out.u32(0x06054b50); // end of central directory
  out.u16(0);
  out.u16(0);
  out.u16(central.length);
  out.u16(central.length);
  out.u32(centralSize);
  out.u32(centralStart);
  out.u16(0); // comment length

  return out.concat();
}
