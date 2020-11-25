//! Component that manages [`TransitableState`].

use std::{cell::RefCell, rc::Rc, time::Duration};

use futures::{future, future::Either, FutureExt, StreamExt};
use medea_reactive::ObservableCell;
use wasm_bindgen_futures::spawn_local;

use crate::{
    peer::media::{
        transitable_state::{
            media_exchange_state, mute_state, InStable, InTransition,
        },
        MediaConnectionsError, Result,
    },
    utils::{resettable_delay_for, ResettableDelayHandle},
};

use super::TransitableState;

/// [`TransitableStateController`] for the [`mute_state`].
pub type MuteStateController =
    TransitableStateController<mute_state::Stable, mute_state::Transition>;
/// [`TransitableStateController`] for the [`media_exchange_state`].
pub type MediaExchangeStateController = TransitableStateController<
    media_exchange_state::Stable,
    media_exchange_state::Transition,
>;

/// Component that manages all kinds of [`TransitableState`].
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
                            let mut states = this.state.subscribe().skip(1);
                            let (timeout, timeout_handle) =
                                resettable_delay_for(Self::TRANSITION_TIMEOUT);
                            this.timeout_handle
                                .borrow_mut()
                                .replace(timeout_handle);
                            match future::select(
                                states.next(),
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

    /// Returns [`Future`] which will be resolved when state of this
    /// [`TransitableStateController`] will be [`TransitableState::Stable`] or
    /// the [`TransitableStateController`] is dropped.
    ///
    /// Succeeds if [`TransitableStateController`]'s state transits into the
    /// `desired_state` or the [`TransitableStateController`] is dropped.
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::MediaStateTransitsIntoOppositeState`]
    /// is returned if [`TransitableStateController`]'s
    /// [`MediaState`] transits into the opposite to the
    /// `desired_state`.
    ///
    /// [`Future`]: futures::future::Future
    /// [`MediaState`]: super::MediaState
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

    /// Updates [`TransitableStateController::state`].
    pub(in super::super) fn update(&self, new_state: S) {
        let current_state = self.state.get();

        let state_update = match current_state {
            TransitableState::Stable(_) => new_state.into(),
            TransitableState::Transition(t) => {
                if t.intended() == new_state {
                    new_state.into()
                } else {
                    t.set_inner(new_state).into()
                }
            }
        };

        self.state.set(state_update);
    }
}

impl MuteStateController {
    /// Checks whether [`TransitableStateController`]'s mute state
    /// is in [`mute_state::Stable::Muted`].
    pub fn muted(&self) -> bool {
        self.state.get() == mute_state::Stable::Muted.into()
    }

    /// Checks whether [`TransitableStateController`]'s mute state
    /// is in [`mute_state::Stable::Unmuted`].
    pub fn unmuted(&self) -> bool {
        self.state.get() == mute_state::Stable::Unmuted.into()
    }
}

impl MediaExchangeStateController {
    /// Checks whether [`TransitableStateController`]'s media exchange state
    /// is in [`media_exchange_state::Stable::Disabled`].
    pub fn disabled(&self) -> bool {
        self.state.get() == media_exchange_state::Stable::Disabled.into()
    }

    /// Checks whether [`TransitableStateController`]'s media exchange state
    /// is in [`media_exchange_state::Stable::Enabled`].
    pub fn enabled(&self) -> bool {
        self.state.get() == media_exchange_state::Stable::Enabled.into()
    }
}
