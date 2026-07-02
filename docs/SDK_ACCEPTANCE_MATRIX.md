# Matriz de aceite do SDK

| Bloco | Status nesta TK | Evidência |
|---|---|---|
| Access package | Implementado | `OriginClient.createAccessPackage` e contratos TS |
| Manifesto | Implementado | contrato `Manifest` e parse mínimo |
| Source selection | Implementado | PEER, REPLICA_EDGE e ORIGIN final |
| Origin download | Implementado | source HTTP com Bearer packageToken e Range |
| Replica download | Implementado | mesma camada HTTP para fonte autorizada |
| Peer contract | Implementado | `PeerSourceAdapter` |
| Peer adapter | Parcial | `DisabledPeerAdapter` controlado |
| Range download | Implementado | header `Range: bytes=start-end` |
| Hash validation | Implementado | sha256 por fragmento antes de aceitar |
| Progress map | Implementado | em memória |
| Fallback | Implementado | por fragmento, origem como garantia final quando autorizada |
| Revalidation | Contrato implementado | método `revalidateAccessPackage` |
| Revocation/expiration | Parcial | fontes expiradas são ignoradas; persistência futura pendente |
| Events/metrics | Parcial | eventos internos e best-effort `recordFragmentEvent` |
| Public API | Implementado | `PontemeshClient.downloadObject` |

## Limitações conscientes

- P2P transport real: Parcial
  - A primeira versão do SDK possui contrato, seleção, validação, fallback e adapter plugável para PEER, mas ainda não implementa transporte real WebRTC/libp2p.
- persistência de progresso em disco: Pendente
- CLI: Pendente
- streaming progressivo real: Pendente
- métricas avançadas de fonte: Pendente
- WebRTC/libp2p: Pendente
