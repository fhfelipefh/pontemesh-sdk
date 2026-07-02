# Game Engine Integration

Game engines and launchers call the native SDK and receive files on disk.

The game provides:

- Origin URL
- application token
- bucket
- key
- destination path

Example Unity flow:

```csharp
using PonteMesh;

var client = new PontemeshClient(
    "https://origin.example.com",
    "application-token"
);

client.SyncObject(
    "game-assets",
    "maps/desert-v3.pak",
    Application.persistentDataPath + "/maps/desert-v3.pak"
);
```

The game does not need to know about manifests, fragments, peers, Replica/Edge, hashes or access packages.

