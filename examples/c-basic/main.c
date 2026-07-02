#include "pontemesh_sdk.h"

int main(void) {
    PontemeshClient* client = 0;
    PontemeshStatus status = pontemesh_client_create(
        "https://origin.example.com",
        "application-token",
        &client
    );

    if (status == PONTEMESH_OK) {
        status = pontemesh_client_sync_object(
            client,
            "game-assets",
            "maps/desert-v3.pak",
            "./Game/Content/maps/desert-v3.pak"
        );
    }

    pontemesh_client_free(client);
    return status == PONTEMESH_OK ? 0 : 1;
}

