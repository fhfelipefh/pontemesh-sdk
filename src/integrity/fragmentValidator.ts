import type { FragmentDescriptor } from "../contracts/manifest"; import { sha256 } from "./sha256";
export async function validateFragment(input:{fragment:FragmentDescriptor; bytes:Uint8Array}): Promise<boolean>{ if(input.fragment.hashAlgorithm.toUpperCase()!=="SHA-256") return false; return (await sha256(input.bytes))===input.fragment.sha256.toLowerCase(); }
