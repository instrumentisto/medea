//! Component that manages [`MediaExchangeState`].

use std::{cell::RefCell, rc::Rc, time::Duration};

use futures::{future, future::Either, FutureExt, StreamExt};
use medea_reactive::ObservableCell;
use wasm_bindgen_futures::spawn_local;

use crate::{
    peer::media::{
        transitable_state::{
            media_exchange_state, mute_state, InStable, InTransition,
            MediaExchangeState, MuteState,
        },
        MediaConnectionsError, Result,
    },
    utils::{resettable_delay_for, ResettableDelayHandle},
};

use super::TransitableState;

/// [`TransitableStateController`] for the [`mute_state::Stable`].
pub type MuteStateController =
    TransitableStateController<mute_state::Stable, mute_state::Transition>;
/// [`TransitableStateController`] for the [`media_exchange_state::Stable`].
pub type MediaExchangeStateController = TransitableStateController<
    media_exchange_state::Stable,
    media_exchange_state::Transition,
>;

/// Component that manages all kinds of [`MediaState`].
pub struct TransitableStateController<S, T> {
    /// Actual [`TransitableState`].
    state: ObservableCell<TransitableState<S, T>>,

    /// Timeout of the [`TransitableStateController::state`] transition.
    timeout_handle: RefCell<Option<ResettableDelayHandle>>,
}

impl<S, T> TransitableStateController<S, T>
where
    S: InStable<Transition = T> + Into<TransitableState<S, T>> + 'static,
    T: InTransition<Stable = S> + Into<TransitableState<S, T>> + 'static,
{
    #[cfg(not(feature = "mockable"))]
    const TRANSITION_TIMEOUT: Duration = Duration::from_secs(10);
    #[cfg(feature = "mockable")]
    const TRANSITION_TIMEOUT: Duration = Duration::from_millis(500);

    /// Returns new [`TransitableStateController`] with a provided
    /// [`InStable`] state.
    pub(in super::super) fn new(state: S) -> Rc<Self> {
        let this = Rc::new(Self {
            state: ObservableCell::new(state.into()),
            timeout_handle: RefCell::new(None),
        });
        this.clone().spawn();
        this
    }

    /// Spawns all needed [`Stream`] listeners for this
    /// [`TransitableStateController`].
    fn spawn(self: Rc<Self>) {
        // we don't care about initial state, cause transceiver is inactive atm
        let mut state_changes = self.state.subscribe().skip(1);
        let weak_this = Rc::downgrade(&self);
        spawn_local(async move {
            while let Some(state) = state_changes.next().await {
                if let Some(this) = weak_this.upgrade() {
                    if let TransitableState::Transition(_) = state {
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

    /// Stops disable/enable timeout of this [`TransitableStateController`].
    pub(in super::super) fn stop_transition_timeout(&self) {
        if let Some(timer) = &*self.timeout_handle.borrow() {
            timer.stop();
        }
    }

    /// Resets disable/enable timeout of this [`TransitableStateController`].
    pub(in super::super) fn reset_transition_timeout(&self) {
        if let Some(timer) = &*self.timeout_handle.borrow() {
            timer.reset();
        }
    }

    /// Returns current [`TransitableStateController::state`].
    pub fn state(&self) -> TransitableState<S, T> {
        self.state.get()
    }

    /// Starts transition of the [`TransitableStateController::state`] to
    /// the provided one.
    pub(in super::super) fn transition_to(&self, desired_state: S) {
        let current_state = self.state.get();
        self.state.set(current_state.transition_to(desired_state));
    }

    /// Returns [`Future`] which will be resolved when [`InStable`] state of
    /// this [`TransitableStateController`] will be
    /// [`TransitableState::Stable`] or the
    /// [`TransitableStateController`] is dropped.
    ///
    /// Succeeds if [`TransitableStateController`]'s [`InStable`] state
    /// transits into the `desired_state` or the
    /// [`TransitableStateController`] is dropped.
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::MediaStateTransitsIntoOppositeState`]
    /// is returned if [`TransitableStateController`]'s
    /// [`MediaState`] transits into the opposite to the
    /// `desired_state`.
    pub fn when_media_state_stable(
        &self,
        desired_state: S,
    ) -> future::LocalBoxFuture<'static, Result<()>> {
        let mut states = self.state.subscribe();
        async move {
            while let Some(state) = states.next().await {
                match state {
                    TransitableState::Transition(_) => continue,
                    TransitableState::Stable(s) => {
                        return if s == desired_state {
                            Ok(())
                        } else {
                            Err(tracerr::new!(
                                MediaConnectionsError::
                                MediaStateTransitsIntoOppositeState
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

impl MuteStateController {
    /// Updates [`TransitableStateController::state`].
    ///
    /// `Room.mute_audio` like `Promise`s will be resolved based on this
    /// update.
    pub(in super::super) fn update(&self, is_muted: bool) {
        let new_mute_state = mute_state::Stable::from(is_muted);
        let current_mute_state = self.state.get();

        let mute_state_update: MuteState = match current_mute_state {
            TransitableState::Stable(_) => new_mute_state.into(),
            TransitableState::Transition(t) => {
                if t.intended() == new_mute_state {
                    new_mute_state.into()
                } else {
                    t.set_inner(new_mute_state).into()
                }
            }
        };

        self.state.set(mute_state_update);
    }

    /// Checks whether [`TransitableStateController`]'s mute state
    /// is in [`mute_state::Stable::Muted`].
    pub fn is_muted(&self) -> bool {
        self.state.get() == mute_state::Stable::Muted.into()
    }

    /// Checks whether [`TransitableStateController`]'s mute state
    /// is in [`mute_state::Stable::Unmuted`].
    pub fn is_unmuted(&self) -> bool {
        self.state.get() == mute_state::Stable::Unmuted.into()
    }
}

impl MediaExchangeStateController {
    /// Updates [`TransitableStateController::state`].
    ///
    /// Real disable/enable __wouldn't__ be performed on this update.
    ///
    /// `Room.disable_audio` like `Promise`s will be resolved based on this
    /// update.
    pub(in super::super) fn update(&self, enabled: bool) {
        let new_media_exchange_state =
            media_exchange_state::Stable::from(enabled);
        let current_media_exchange_state = self.state.get();

        let media_exchange_state_update: MediaExchangeState =
            match current_media_exchange_state {
                TransitableState::Stable(_) => new_media_exchange_state.into(),
                TransitableState::Transition(t) => {
                    if t.intended() == new_media_exchange_state {
                        new_media_exchange_state.into()
                    } else {
                        t.set_inner(new_media_exchange_state).into()
                    }
                }
            };

        self.state.set(media_exchange_state_update);
    }

    /// Checks whether [`TransitableStateController`]'s media exchange state
    /// is in [`MediaExchangeState::Disabled`].
    pub fn is_disabled(&self) -> bool {
        self.state.get() == media_exchange_state::Stable::Disabled.into()
    }

    /// Checks whether [`TransitableStateController`]'s media exchange state
    /// is in [`MediaExchangeState::Enabled`].
    pub fn is_enabled(&self) -> bool {
        self.state.get() == media_exchange_state::Stable::Enabled.into()
    }
}
