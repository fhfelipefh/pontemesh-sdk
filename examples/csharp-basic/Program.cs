using System;
using PonteMesh;

using var client = new PontemeshClient(
    "https://origin.example.com",
    "application-token"
);

var summary = client.SyncObjectWithSummary(
    "game-assets",
    "maps/desert-v3.pak",
    "./Game/Content/maps/desert-v3.pak"
);

Console.WriteLine(
    $"downloaded via peer={summary.BytesFromPeer}, replica={summary.BytesFromReplica}, origin={summary.BytesFromOrigin}"
);
