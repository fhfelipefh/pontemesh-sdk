import type { ErrorCode } from "./ErrorCodes";
export class PontemeshError extends Error { constructor(public code: ErrorCode, message: string, public detail?: unknown){ super(message); this.name="PontemeshError"; } }
