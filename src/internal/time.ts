export function isExpired(iso: string, now = new Date()): boolean { const t=Date.parse(iso); return Number.isFinite(t) && t <= now.getTime(); }
