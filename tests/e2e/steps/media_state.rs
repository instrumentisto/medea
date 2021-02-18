use cucumber_rust::when;

use crate::{object::MediaKind, parse_media_kind, world::World};

#[when(regex = "^Member (\\S*) (disables|mutes) (audio|video|all)$")]
async fn when_disables_mutes(
    world: &mut World,
    id: String,
    disable_or_mutes: String,
    audio_or_video: String,
) {
    let member = world.get_member(&id).unwrap();
    if disable_or_mutes == "disables" {
        if let Some(kind) = parse_media_kind(&audio_or_video) {
            member.disable_media_send(kind, None).await.unwrap();
        } else {
            member
                .disable_media_send(MediaKind::Audio, None)
                .await
                .unwrap();
            member
                .disable_media_send(MediaKind::Video, None)
                .await
                .unwrap();
        }
    } else {
        todo!("Muting is unimplemented atm.")
    }
}
