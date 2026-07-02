import type { SourceType } from "./sources";
export interface FragmentAvailability { index: number; fragmentId: string; byteRangeStart: number; byteRangeEnd: number; sizeBytes: number; sha256: string; originAvailable: boolean; replicaSourceIds: string[]; peerSourceIds: string[]; availableSourceTypes: SourceType[]; }
export interface AvailabilityResponse { bucket: string; key: string; manifestId: string; objectState: string; originAvailable: boolean; replicaSources: number; peerSources: number; fragments: FragmentAvailability[]; }
