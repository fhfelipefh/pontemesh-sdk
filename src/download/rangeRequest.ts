import type { FragmentDescriptor } from "../contracts/manifest";
export function rangeHeader(fragment: FragmentDescriptor): string { return `bytes=${fragment.byteRangeStart}-${fragment.byteRangeEnd}`; }
