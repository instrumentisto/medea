//! Component that manages [`MediaExchangeState`].

use std::{cell::RefCell, rc::Rc, time::Duration};

use futures::{
    future, future::Either, stream::LocalBoxStream, FutureExt, StreamExt,
};
use medea_reactive::ObservableCell;
use wasm_bindgen_futures::spawn_local;

use crate::{
    peer::{
        media::{
            media_exchange_state::{InStable, InTransition},
            MediaConnectionsError, Result,
        },
        MediaExchangeStateTransition,
    },
    utils::{resettable_delay_for, ResettableDelayHandle},
};

use super::{MediaExchangeState, StableMediaExchangeState};

/// Component that manages [`MediaExchangeState`].
pub struct MediaExchangeStateController<T, S> {
    /// Actual [`MediaExchangeState`].
    state: ObservableCell<MediaExchangeState<T, S>>,

    /// Timeout of the [`MediaExchangeStateController::state`] transition.
    timeout_handle: RefCell<Option<ResettableDelayHandle>>,
}

impl<T, S> MediaExchangeStateController<T, S>
where
    T: InTransition<Stable = S>
        + Clone
        + Copy
        + PartialEq
        + Into<MediaExchangeState<T, S>>
        + 'static,
    S: InStable<Transition = T>
        + Clone
        + Copy
        + PartialEq
        + Into<MediaExchangeState<T, S>>
        + 'static,
{
    #[cfg(not(feature = "mockable"))]
    const TRANSITION_TIMEOUT: Duration = Duration::from_secs(10);
    #[cfg(feature = "mockable")]
    const TRANSITION_TIMEOUT: Duration = Duration::from_millis(500);

    /// Returns new [`MediaExchangeStateController`] with a provided
    /// [`StableMediaExchangeState`].
    pub(in super::super) fn new(state: S) -> Rc<Self> {
        let this = Rc::new(Self {
            state: ObservableCell::new(state.into()),
            timeout_handle: RefCell::new(None),
        });
        this.clone().spawn();
        this
    }

    /// Returns [`Stream`] to which [`StableMediaExchangeState`] update will be
    /// sent on [`MediaExchangeStateController::state`] stabilization.
    pub(in super::super) fn on_stabilize(&self) -> LocalBoxStream<'static, S> {
        self.state
            .subscribe()
            .skip(1)
            .filter_map(|state| async move {
                if let MediaExchangeState::Stable(s) = state {
                    Some(s)
                } else {
                    None
                }
            })
            .boxed_local()
    }

    /// Spawns all needed [`Stream`] listeners for this
    /// [`MediaExchangeStateController`].
    fn spawn(self: Rc<Self>) {
        // we don't care about initial state, cause transceiver is inactive atm
        let mut media_exchange_state_changes = self.state.subscribe().skip(1);
        let weak_this = Rc::downgrade(&self);
        spawn_local(async move {
            while let Some(media_exchange_state) =
                media_exchange_state_changes.next().await
            {
                if let Some(this) = weak_this.upgrade() {
                    if let MediaExchangeState::Transition(_) =
                        media_exchange_state
                    {
                        let weak_this = Rc::downgrade(&this);
                        spawn_local(async move {
                            let mut transitions =
                                this.state.subscribe().skip(1);
                            let (timeout, timeout_handle) =
                                resettable_delay_for(Self::TRANSITION_TIMEOUT);
                            this.timeout_handle
                                .borrow_mut()
                                .replace(timeout_handle);
                            match future::select(
                                transitions.next(),
                                Box::pin(timeout),
                            )
                            .await
                            {
                                Either::Left(_) => (),
                                Either::Right(_) => {
                                    if let Some(this) = weak_this.upgrade() {
                                        let stable = this
                                            .state
                                            .get()
                                            .cancel_transition();
                                        this.state.set(stable);
                                    }
                                }
                            }
                        });
                    }
                } else {
                    break;
                }
            }
        });
    }

    /// Stops disable/enable timeout of this [`MediaExchangeStateController`].
    pub(in super::super) fn stop_transition_timeout(&self) {
        if let Some(timer) = &*self.timeout_handle.borrow() {
            timer.stop();
        }
    }

    /// Resets disable/enable timeout of this [`MediaExchangeStateController`].
    pub(in super::super) fn reset_transition_timeout(&self) {
        if let Some(timer) = &*self.timeout_handle.borrow() {
            timer.reset();
        }
    }

    /// Returns current [`MediaExchangeStateController::state`].
    pub fn media_exchange_state(&self) -> MediaExchangeState<T, S> {
        self.state.get()
    }

    /// Starts transition of the [`MediaExchangeStateController::state`] to
    /// the provided one.
    pub(in super::super) fn transition_to(&self, desired_state: S) {
        let current_media_exchange_state = self.state.get();
        self.state
            .set(current_media_exchange_state.transition_to(desired_state));
    }

    /// Cancels [`MediaExchangeStateController::state`] transition.
    pub(in super::super) fn cancel_transition(&self) {
        let state = self.state.get();
        self.state.set(state.cancel_transition());
    }

    /// Returns [`Future`] which will be resolved when [`MediaExchangeState`] of
    /// this [`MediaExchangeStateController`] will be
    /// [`MediaExchangeState::Stable`] or the
    /// [`MediaExchangeStateController`] is dropped.
    ///
    /// Succeeds if [`MediaExchangeStateController`]'s [`MediaExchangeState`]
    /// transits into the `desired_state` or the
    /// [`MediaExchangeStateController`] is dropped.
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::MediaExchangeStateTransitsIntoOppositeState`]
    /// is returned if [`MediaExchangeStateController`]'s
    /// [`MediaExchangeState`] transits into the opposite to the
    /// `desired_state`.
    pub fn when_media_exchange_state_stable(
        &self,
        desired_state: S,
    ) -> future::LocalBoxFuture<'static, Result<()>> {
        let mut media_exchange_states = self.state.subscribe();
        async move {
            while let Some(state) = media_exchange_states.next().await {
                match state {
                    MediaExchangeState::Transition(_) => continue,
                    MediaExchangeState::Stable(s) => {
                        return if s == desired_state {
                            Ok(())
                        } else {
                            Err(tracerr::new!(
                                MediaConnectionsError::
                                MediaExchangeStateTransitsIntoOppositeState
                            ))
                        }
                    }
                }
            }
            Ok(())
        }
        .boxed_local()
    }
}

impl
    MediaExchangeStateController<
        MediaExchangeStateTransition,
        StableMediaExchangeState,
    >
{
    /// Updates [`MediaExchangeStateController::state`].
    ///
    /// Real disable/enable __wouldn't__ be performed on this update.
    ///
    /// `Room.disable_audio` like `Promise`s will be resolved based on this
    /// update.
    pub(in super::super) fn update(&self, is_disabled: bool) {
        let new_media_exchange_state =
            StableMediaExchangeState::from(is_disabled);
        let current_media_exchange_state = self.state.get();

        let media_exchange_state_update: MediaExchangeState<
            MediaExchangeStateTransition,
            StableMediaExchangeState,
        > = match current_media_exchange_state {
            MediaExchangeState::Stable(_) => new_media_exchange_state.into(),
            MediaExchangeState::Transition(t) => {
                if t.intended() == new_media_exchange_state {
                    new_media_exchange_state.into()
                } else {
                    t.set_inner(new_media_exchange_state).into()
                }
            }
        };

        self.state.set(media_exchange_state_update);
    }

    /// Checks whether [`MediaExchangeStateController`]'s media exchange state
    /// is in [`MediaExchangeState::Disabled`].
    pub fn is_disabled(&self) -> bool {
        self.state.get() == StableMediaExchangeState::Disabled.into()
    }

    /// Checks whether [`MediaExchangeStateController`]'s media exchange state
    /// is in [`MediaExchangeState::Enabled`].
    pub fn is_enabled(&self) -> bool {
        self.state.get() == StableMediaExchangeState::Enabled.into()
    }
}
