# Server SDK Protocol

The native SDK consumes Ponte Mesh protocol endpoints only.

Allowed endpoints:

- `POST /pontemesh/access-packages`
- `GET /pontemesh/objects/{bucket}/manifest/{key}`
- `GET /pontemesh/objects/{bucket}/sources/{key}`
- `GET /pontemesh/objects/{bucket}/availability/{key}`
- `GET /pontemesh/objects/{bucket}/policies/{key}`
- `GET /pontemesh/access-packages/{packageId}/objects/{bucket}/{key}`
- `POST /pontemesh/access-packages/{packageId}/revalidate/{bucket}/{key}`
- `POST /pontemesh/access-packages/{packageId}/events/{bucket}/{key}`
- `POST /pontemesh/access-packages/{packageId}/peers/{bucket}/{key}`
- `GET /pontemesh/replica/access-packages/{packageId}/objects/{bucket}/{key}`

Forbidden in the SDK:

- S3
- SigV4
- S3 access key
- S3 secret
- `ListBuckets`
- `PutObject`
- `DeleteObject`
- admin API
- MCP
- database access
- migrations

