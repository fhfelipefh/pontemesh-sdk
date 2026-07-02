#include "../../bindings/cpp/pontemesh_sdk.hpp"

int main() {
    pontemesh::Client client("https://origin.example.com", "application-token");
    client.sync_object(
        "game-assets",
        "maps/desert-v3.pak",
        "./Game/Content/maps/desert-v3.pak"
    );
    return 0;
}

