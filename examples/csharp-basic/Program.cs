using PonteMesh;

using var client = new PontemeshClient(
    "https://origin.example.com",
    "application-token"
);

client.SyncObject(
    "game-assets",
    "maps/desert-v3.pak",
    "./Game/Content/maps/desert-v3.pak"
);

