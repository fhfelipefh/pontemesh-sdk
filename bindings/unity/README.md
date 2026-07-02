# Unity binding

Unity loads the same native C ABI used by C, C++, C#, and future Python/Node wrappers.

Place the compiled library beside `Assets/Plugins/PonteMesh/PontemeshSdk.cs`:

- Windows: `pontemesh_sdk.dll`
- Linux: `libpontemesh_sdk.so`
- macOS: `libpontemesh_sdk.dylib`

Example:

```csharp
using PonteMesh;

var client = new PontemeshClient("https://origin.example.com", "application-token");
client.SyncObject(
    "game-assets",
    "maps/desert-v3.pak",
    Application.persistentDataPath + "/maps/desert-v3.pak"
);
```
