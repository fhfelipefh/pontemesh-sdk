import type { FragmentAvailability } from "./availability";
export type SourceType = "ORIGIN" | "REPLICA_EDGE" | "PEER";
export interface AuthorizedSource { id: string; sourceType: SourceType; endpoint: string; priority: number; expiresAt: string; availableFragments: number[]; }
export interface SourceSelectionContract { strategy: string; fragmentPriority: string; failureThreshold: number; allowPeerSharing: boolean; allowReplicaEdge: boolean; }
export interface SourcesResponse { bucket: string; key: string; manifestId: string; authorizedSources: AuthorizedSource[]; sourceSelection: SourceSelectionContract; fallback: import("./fallback").FallbackContract; }
export function sourceHasFragment(source: AuthorizedSource, index: number): boolean { return source.sourceType === "ORIGIN" || source.availableFragments.includes(index); }
export type { FragmentAvailability };
