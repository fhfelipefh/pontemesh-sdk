# Referência contratual do servidor

O repositório local `../pontemesh-server` não estava disponível no ambiente desta TK e o clone remoto exigiu autenticação. Portanto, esta referência registra as rotas e contratos exigidos pela tarefa como referência contratual inicial, sem copiar implementação interna Rust.

## Rotas de aplicação/SDK usadas

| Handler esperado | Método | Rota no SDK | Autenticação |
|---|---:|---|---|
| create_access_package | POST | `/api/v1/access-packages` | `Authorization: Bearer <applicationToken>` |
| get_manifest | GET | `/api/v1/objects/{bucket}/{key}/manifest` | `Authorization: Bearer <applicationToken>` |
| get_sources | GET | `/api/v1/objects/{bucket}/{key}/sources` | `Authorization: Bearer <applicationToken>` |
| get_availability | GET | `/api/v1/objects/{bucket}/{key}/availability` | `Authorization: Bearer <applicationToken>` |
| get_object_policy | GET | `/api/v1/objects/{bucket}/{key}/policy` | `Authorization: Bearer <applicationToken>` |
| revalidate_access_package | POST | `/api/v1/access-packages/{packageId}/revalidate` | `Authorization: Bearer <packageToken>` |
| record_sdk_fragment_event | POST | `/api/v1/access-packages/{packageId}/fragment-events` | `Authorization: Bearer <packageToken>` |
| get_object_with_access_package | GET | endpoint em `AuthorizedSource.endpoint` ou `fallback.objectEndpoint` | `Authorization: Bearer <packageToken>` + `Range` |
| announce_peer_availability | POST | pendente para TK de sharing real | package token/application token conforme servidor |

## Contratos modelados

- `AccessPackage`: pacote emitido pelo Origin contendo `packageToken`, manifesto, fontes autorizadas, política de seleção e fallback.
- `Manifest`: descrição imutável do objeto e dos fragmentos.
- `FragmentDescriptor`: índice, faixa de bytes, tamanho, SHA-256 e header de fallback por range.
- `AuthorizedSource`: fonte `ORIGIN`, `REPLICA_EDGE` ou `PEER` autorizada pelo Origin.
- `SourceSelectionContract`: estratégia, prioridade, thresholds e permissões para Replica/Edge e Peer.
- `FallbackContract`: fonte final, endpoint, suporte a Range e preservação de fragmentos validados.
- `AvailabilityResponse`: disponibilidade por fragmento em Origin, Replica/Edge e Peer.
- `RevalidateAccessPackageResponse`: renovação/validação de pacote e fontes autorizadas.

## Endpoints diretos

- Replica direct serving endpoint: o SDK usa o `endpoint` retornado em `AuthorizedSource` para fontes `REPLICA_EDGE`.
- Peer availability endpoint: preparado como contrato futuro via `PeerAvailability` e `announceValidatedFragment`, sem transporte real nesta TK.
- SDK fragment event endpoint: `/api/v1/access-packages/{packageId}/fragment-events`, best-effort.

## Divergências pendentes

Quando o repositório `pontemesh-server` estiver disponível, confirmar esta tabela com `src/http/mod.rs`, `docs/api/origin-api.md` e `docs/api/replica-api.md`.
