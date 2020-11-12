//! Component that manages [`media_exchange_state::State`].

use std::{cell::RefCell, rc::Rc, time::Duration};

use futures::{future, future::Either, FutureExt, StreamExt};
use medea_reactive::ObservableCell;
use wasm_bindgen_futures::spawn_local;

use crate::{
    peer::media::{media_exchange_state, MediaConnectionsError, Result},
    utils::{resettable_delay_for, ResettableDelayHandle},
};

/// Component that manages [`media_exchange_state::State`].
pub struct Controller {
    /// Actual [`media_exchange_state::State`].
    state: ObservableCell<media_exchange_state::State>,

    /// Timeout of the [`Controller::state`] transition.
    timeout_handle: RefCell<Option<ResettableDelayHandle>>,
}

impl Controller {
    #[cfg(not(feature = "mockable"))]
    const TRANSITION_TIMEOUT: Duration = Duration::from_secs(10);
    #[cfg(feature = "mockable")]
    const TRANSITION_TIMEOUT: Duration = Duration::from_millis(500);

    /// Returns new [`Controller`] with a provided
    /// [`media_exchange_state::Stable`].
    pub(in super::super) fn new(
        state: media_exchange_state::Stable,
    ) -> Rc<Self> {
        let this = Rc::new(Self {
            state: ObservableCell::new(state.into()),
            timeout_handle: RefCell::new(None),
        });
        this.clone().spawn();
        this
    }

    /// Spawns all needed [`Stream`] listeners for this [`Controller`].
    fn spawn(self: Rc<Self>) {
        // we don't care about initial state, cause transceiver is inactive atm
        let mut media_exchange_state_changes = self.state.subscribe().skip(1);
        let weak_this = Rc::downgrade(&self);
        spawn_local(async move {
            while let Some(media_exchange_state) =
                media_exchange_state_changes.next().await
            {
                if let Some(this) = weak_this.upgrade() {
                    if let media_exchange_state::State::Transition(_) =
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

    /// Checks whether [`Controller`]'s media exchange state is in
    /// [`media_exchange_state::Stable::Disabled`].
    pub fn disabled(&self) -> bool {
        self.state.get() == media_exchange_state::Stable::Disabled.into()
    }

    /// Checks whether [`Controller`]'s media exchange state is in
    /// [`media_exchange_state::Stable::Enabled`].
    pub fn enabled(&self) -> bool {
        self.state.get() == media_exchange_state::Stable::Enabled.into()
    }

    /// Stops disable/enable timeout of this [`Controller`].
    pub(in super::super) fn stop_transition_timeout(&self) {
        if let Some(timer) = &*self.timeout_handle.borrow() {
            timer.stop();
        }
    }

    /// Resets disable/enable timeout of this [`Controller`].
    pub(in super::super) fn reset_transition_timeout(&self) {
        if let Some(timer) = &*self.timeout_handle.borrow() {
            timer.reset();
        }
    }

    /// Updates [`Controller::state`].
    ///
    /// Real disable/enable __wouldn't__ be performed on this update.
    ///
    /// `Room.disable_audio` like `Promise`s will be resolved based on this
    /// update.
    pub(in super::super) fn update(&self, enabled: bool) {
        use media_exchange_state::State;

        let new_state = media_exchange_state::Stable::from(enabled);
        let current_media_exchange_state = self.state.get();

        let media_exchange_state_update: State =
            match current_media_exchange_state {
                State::Stable(_) => new_state.into(),
                State::Transition(t) => {
                    if t.intended() == new_state {
                        new_state.into()
                    } else {
                        t.set_inner(new_state).into()
                    }
                }
            };

        self.state.set(media_exchange_state_update);
    }

    /// Returns current [`Controller::state`].
    pub fn media_exchange_state(&self) -> media_exchange_state::State {
        self.state.get()
    }

    /// Starts transition of the [`Controller::state`] to the provided one.
    pub(in super::super) fn transition_to(
        &self,
        desired_state: media_exchange_state::Stable,
    ) {
        let current_media_exchange_state = self.state.get();
        self.state
            .set(current_media_exchange_state.transition_to(desired_state));
    }

    /// Cancels [`Controller::state`] transition.
    pub(in super::super) fn cancel_transition(&self) {
        let state = self.state.get();
        self.state.set(state.cancel_transition());
    }

    /// Returns [`Future`] which will be resolved when
    /// [`media_exchange_state::State`] of this [`Controller`] will be
    /// [`media_exchange_state::State::Stable`] or the [`Controller`] is
    /// dropped.
    ///
    /// Succeeds if [`Controller`]'s [`media_exchange_state::State`] transits
    /// into the `desired_state` or the [`Controller`] is dropped.
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::MediaExchangeStateTransitsIntoOppositeState`]
    /// is returned if [`Controller`]'s [`media_exchange_state::State`] transits
    /// into the opposite to the `desired_state`.
    pub fn when_media_exchange_state_stable(
        &self,
        desired_state: media_exchange_state::Stable,
    ) -> future::LocalBoxFuture<'static, Result<()>> {
        let mut media_exchange_states = self.state.subscribe();
        async move {
            while let Some(state) = media_exchange_states.next().await {
                match state {
                    media_exchange_state::State::Transition(_) => continue,
                    media_exchange_state::State::Stable(s) => {
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
