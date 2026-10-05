use pontemesh_sdk_core::{
    p2p::P2pConfig, PontemeshClient, PontemeshClientConfig, SyncObjectRequest,
};

fn main() -> Result<(), pontemesh_sdk_core::PontemeshError> {
    let client = PontemeshClient::new(PontemeshClientConfig::new(
        "https://origin.example.com",
        "application-token",
    ))?;

    let result = client.sync_object_with_summary(SyncObjectRequest {
        bucket: "game-assets".to_string(),
        key: "maps/desert-v3.pak".to_string(),
        destination: "./Game/Content/maps/desert-v3.pak".into(),
    })?;

    println!(
        "downloaded via peer={}, replica={}, origin={}",
        result.summary.bytes_from_peer,
        result.summary.bytes_from_replica,
        result.summary.bytes_from_origin
    );
    Ok(())
}
