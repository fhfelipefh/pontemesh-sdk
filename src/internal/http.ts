import { PontemeshError } from "../errors/PontemeshError";
export type FetchLike = typeof fetch;
export function joinUrl(base:string,path:string): string { return `${base.replace(/\/+$/,"")}/${path.replace(/^\/+/ ,"")}`; }
export async function readJson<T>(response: Response, code="ORIGIN_REQUEST_FAILED"): Promise<T>{ if(!response.ok) throw new PontemeshError(code as never, `HTTP ${response.status}`); return await response.json() as T; }
