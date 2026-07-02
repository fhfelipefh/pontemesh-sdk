# Unity basic example

Use `bindings/unity/Assets/Plugins/PonteMesh/PontemeshSdk.cs` in a Unity project.

```csharp
using PonteMesh;

var client = new PontemeshClient("https://origin.example.com", "application-token");
client.SyncObject("game-assets", "maps/desert-v3.pak", Application.persistentDataPath + "/maps/desert-v3.pak");
```

