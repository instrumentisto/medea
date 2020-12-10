use std::rc::Rc;

use futures::future::LocalBoxFuture;
use medea_client_api_proto::{MediaType, MemberId, TrackId, TrackPatchEvent};
use medea_macro::{watch, watchers};
use medea_reactive::{Guarded, ProgressableCell};
use tracerr::Traced;

use crate::{
    api::RoomCtx,
    media::RecvConstraints,
    peer::{MediaConnectionsError, Receiver},
    utils::Component,
};

pub type ReceiverComponent = Component<ReceiverState, Rc<Receiver>, RoomCtx>;

pub struct ReceiverState {
    id: TrackId,
    mid: Option<String>,
    media_type: MediaType,
    sender: MemberId,
    enabled_individual: ProgressableCell<bool>,
    enabled_general: ProgressableCell<bool>,
    muted: ProgressableCell<bool>,
}

impl ReceiverState {
    pub fn new(
        id: TrackId,
        mid: Option<String>,
        media_type: MediaType,
        sender: MemberId,
        recv_constraints: &RecvConstraints,
    ) -> Self {
        let enabled = match &media_type {
            MediaType::Audio(_) => recv_constraints.is_audio_enabled(),
            MediaType::Video(_) => recv_constraints.is_video_enabled(),
        };
        Self {
            id,
            mid,
            media_type,
            sender,
            enabled_general: ProgressableCell::new(enabled),
            enabled_individual: ProgressableCell::new(enabled),
            muted: ProgressableCell::new(false),
        }
    }

    pub fn id(&self) -> TrackId {
        self.id
    }

    pub fn mid(&self) -> &Option<String> {
        &self.mid
    }

    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    pub fn sender(&self) -> &MemberId {
        &self.sender
    }

    pub fn enabled_individual(&self) -> bool {
        self.enabled_individual.get()
    }

    pub fn enabled_general(&self) -> bool {
        self.enabled_general.get()
    }

    pub fn update(&self, track_patch: TrackPatchEvent) {
        if self.id != track_patch.id {
            return;
        }
        if let Some(enabled_general) = track_patch.enabled_general {
            self.enabled_general.set(enabled_general);
        }
        if let Some(enabled_individual) = track_patch.enabled_individual {
            self.enabled_individual.set(enabled_individual);
        }
        if let Some(muted) = track_patch.muted {
            self.muted.set(muted);
        }
    }

    pub fn when_updated(&self) -> LocalBoxFuture<'static, ()> {
        let fut = futures::future::join_all(vec![
            self.enabled_general.when_all_processed(),
            self.enabled_individual.when_all_processed(),
            self.muted.when_all_processed(),
        ]);
        Box::pin(async move {
            fut.await;
        })
    }
}

#[watchers]
impl ReceiverComponent {
    #[watch(self.state().muted.subscribe())]
    async fn observe_muted(
        ctx: Rc<Receiver>,
        _: Rc<RoomCtx>,
        _: Rc<ReceiverState>,
        muted: Guarded<bool>,
    ) -> Result<(), Traced<MediaConnectionsError>> {
        ctx.set_muted(*muted);

        Ok(())
    }

    #[watch(self.state().enabled_individual.subscribe())]
    async fn observe_enabled_individual(
        ctx: Rc<Receiver>,
        _: Rc<RoomCtx>,
        _: Rc<ReceiverState>,
        enabled_individual: Guarded<bool>,
    ) -> Result<(), Traced<MediaConnectionsError>> {
        ctx.set_enabled_individual_state(*enabled_individual);

        Ok(())
    }

    #[watch(self.state().enabled_general.subscribe())]
    async fn observe_enabled_general(
        ctx: Rc<Receiver>,
        _: Rc<RoomCtx>,
        _: Rc<ReceiverState>,
        enabled_general: Guarded<bool>,
    ) -> Result<(), Traced<MediaConnectionsError>> {
        ctx.set_enabled_general_state(*enabled_general);

        Ok(())
    }
}
