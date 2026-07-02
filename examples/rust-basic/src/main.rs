use pontemesh_sdk_core::{PontemeshClient, PontemeshClientConfig, SyncObjectRequest};

fn main() -> Result<(), pontemesh_sdk_core::PontemeshError> {
    let client = PontemeshClient::new(PontemeshClientConfig {
        origin_url: "https://origin.example.com".to_string(),
        application_token: "application-token".to_string(),
    });

    client.sync_object(SyncObjectRequest {
        bucket: "game-assets".to_string(),
        key: "maps/desert-v3.pak".to_string(),
        destination: "./Game/Content/maps/desert-v3.pak".into(),
    })
}

