export function concatBytes(parts: Uint8Array[]): Uint8Array { const total=parts.reduce((n,p)=>n+p.byteLength,0); const out=new Uint8Array(total); let o=0; for(const p of parts){ out.set(p,o); o+=p.byteLength;} return out; }
export function toUint8Array(buffer: ArrayBuffer): Uint8Array { return new Uint8Array(buffer); }
