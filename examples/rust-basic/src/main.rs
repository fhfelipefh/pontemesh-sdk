use pontemesh_sdk_core::{
    p2p::P2pConfig, PontemeshClient, PontemeshClientConfig, SyncObjectRequest,
};

fn main() -> Result<(), pontemesh_sdk_core::PontemeshError> {
    let client = PontemeshClient::new(PontemeshClientConfig {
        origin_url: "https://origin.example.com".to_string(),
        application_token: "application-token".to_string(),
        p2p: P2pConfig::default(),
    })?;

    client.sync_object(SyncObjectRequest {
        bucket: "game-assets".to_string(),
        key: "maps/desert-v3.pak".to_string(),
        destination: "./Game/Content/maps/desert-v3.pak".into(),
    })
}
