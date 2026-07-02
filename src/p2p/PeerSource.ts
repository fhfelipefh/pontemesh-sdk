import type { AuthorizedSource } from "../contracts/sources";
export interface PeerSource extends AuthorizedSource { sourceType: "PEER"; }
