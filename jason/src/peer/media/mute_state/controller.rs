//! Controller of the [`MuteState`] for the all [`Track`]s.

use std::{cell::RefCell, rc::Rc, time::Duration};

use futures::{
    channel::mpsc, future, future::Either, stream::LocalBoxStream, FutureExt,
    StreamExt,
};
use medea_reactive::ObservableCell;
use wasm_bindgen_futures::spawn_local;

use crate::{
    peer::media::{MediaConnectionsError, Result},
    utils::{resettable_delay_for, ResettableDelayHandle},
};

use super::{MuteState, StableMuteState};

/// Controller of the [`MuteState`]s of the [`Track`]s.
pub struct MuteStateController {
    /// General [`StableMuteState`] between `Recv` and `Send` [`Track`]s.
    general_mute_state: ObservableCell<StableMuteState>,

    /// [`MuteState`] of the local [`Track`].
    individual_mute_state: ObservableCell<MuteState>,

    /// Timeout of the [`MuteStateController::individual_mute_state`]
    /// transition.
    mute_timeout_handle: RefCell<Option<ResettableDelayHandle>>,

    /// All subscribers on the [`MuteStateController::general_mute_state`]
    /// changes.
    on_finalized_subs: RefCell<Vec<mpsc::UnboundedSender<StableMuteState>>>,

    on_individual_update: RefCell<Vec<mpsc::UnboundedSender<StableMuteState>>>,
}

impl MuteStateController {
    #[cfg(not(feature = "mockable"))]
    const MUTE_TRANSITION_TIMEOUT: Duration = Duration::from_secs(10);
    #[cfg(feature = "mockable")]
    const MUTE_TRANSITION_TIMEOUT: Duration = Duration::from_millis(500);

    /// Returns new [`MuteStateController`] with a provided [`StableMuteState`].
    pub fn new(mute_state: StableMuteState) -> Rc<Self> {
        let this = Rc::new(Self {
            general_mute_state: ObservableCell::new(mute_state),
            individual_mute_state: ObservableCell::new(mute_state.into()),
            on_finalized_subs: RefCell::default(),
            on_individual_update: RefCell::default(),
            mute_timeout_handle: RefCell::new(None),
        });
        this.clone().spawn();

        this
    }

    /// Returns [`Stream`] to which all
    /// [`MuteStateController::general_mute_state`]s will be sent.
    pub fn on_finalized(&self) -> LocalBoxStream<'static, StableMuteState> {
        let (tx, rx) = mpsc::unbounded();
        self.on_finalized_subs.borrow_mut().push(tx);

        Box::pin(rx)
    }

    /// Sends [`MuteStateController::general_mute_state`] update.
    fn send_finalized_state(&self, state: StableMuteState) {
        let mut on_finalize_subs = self.on_finalized_subs.borrow_mut();
        *on_finalize_subs = on_finalize_subs
            .drain(..)
            .filter(|s| s.unbounded_send(state).is_ok())
            .collect();
    }

    /// Returns [`Stream`] to which all
    /// [`MuteStateController::individual_mute_state`]s will be sent.
    pub fn on_individual_update(
        &self,
    ) -> LocalBoxStream<'static, StableMuteState> {
        let (tx, rx) = mpsc::unbounded();
        self.on_individual_update.borrow_mut().push(tx);

        Box::pin(rx)
    }

    /// Sends [`MuteStateController::individual_mute_state`] update.
    fn send_individual_update(&self, state: StableMuteState) {
        let mut on_individual_update = self.on_individual_update.borrow_mut();
        *on_individual_update = on_individual_update
            .drain(..)
            .filter(|s| s.unbounded_send(state).is_ok())
            .collect();
    }

    /// Spawns all needed [`Stream`] listeners for this [`MuteStateController`].
    fn spawn(self: Rc<Self>) {
        // we don't care about initial state, cause transceiver is inactive atm
        let mut mute_state_changes =
            self.individual_mute_state.subscribe().skip(1);
        let weak_this = Rc::downgrade(&self);
        spawn_local({
            let weak_this = Rc::downgrade(&self);
            let mut general_mute_state_changes =
                self.general_mute_state.subscribe().skip(1);
            async move {
                while let Some(mute_state) =
                    general_mute_state_changes.next().await
                {
                    if let Some(this) = weak_this.upgrade() {
                        this.send_finalized_state(mute_state);
                    }
                }
            }
        });
        spawn_local(async move {
            while let Some(mute_state) = mute_state_changes.next().await {
                if let Some(this) = weak_this.upgrade() {
                    match mute_state {
                        MuteState::Stable(upd) => {
                            this.send_individual_update(upd);
                        }
                        MuteState::Transition(_) => {
                            let weak_this = Rc::downgrade(&this);
                            spawn_local(async move {
                                let mut transitions = this
                                    .individual_mute_state
                                    .subscribe()
                                    .skip(1);
                                let (timeout, timeout_handle) =
                                    resettable_delay_for(
                                        Self::MUTE_TRANSITION_TIMEOUT,
                                    );
                                this.mute_timeout_handle
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
                                        if let Some(this) = weak_this.upgrade()
                                        {
                                            let stable = this
                                                .individual_mute_state
                                                .get()
                                                .cancel_transition();
                                            this.individual_mute_state
                                                .set(stable);
                                        }
                                    }
                                }
                            });
                        }
                    }
                } else {
                    break;
                }
            }
        });
    }

    /// Checks whether [`MuteStateController`] is in [`MuteState::Muted`].
    pub fn is_muted(&self) -> bool {
        self.individual_mute_state.get() == StableMuteState::Muted.into()
    }

    /// Checks whether [`MuteStateController`] is in [`MuteState::NotMuted`].
    pub fn is_not_muted(&self) -> bool {
        self.individual_mute_state.get() == StableMuteState::NotMuted.into()
    }

    /// Stops mute/unmute timeout of this [`MuteStateController`].
    pub fn stop_mute_state_transition_timeout(&self) {
        if let Some(timer) = &*self.mute_timeout_handle.borrow() {
            timer.stop();
        }
    }

    /// Resets mute/unmute timeout of this [`MuteStateController`].
    pub fn reset_mute_state_transition_timeout(&self) {
        if let Some(timer) = &*self.mute_timeout_handle.borrow() {
            timer.reset();
        }
    }

    /// Updates [`MuteStateController::general_mute_state`].
    ///
    /// Real mute/unmute will be performed on this update.
    ///
    /// No `Promise`s will be resolved on this state update.
    pub fn update_general(&self, is_muted: bool) {
        self.general_mute_state.set(is_muted.into());
    }

    /// Updates [`MuteStateController::individual_mute_state`].
    ///
    /// Real mute/unmute __wouldn't__ be performed on this update.
    ///
    /// `Room.mute_audio` like `Promise`s will be resolved based on this update.
    pub fn update_individual(&self, is_muted: bool) {
        let new_mute_state = StableMuteState::from(is_muted);
        let current_mute_state = self.individual_mute_state.get();

        let mute_state_update: MuteState = match current_mute_state {
            MuteState::Stable(_) => new_mute_state.into(),
            MuteState::Transition(t) => {
                if t.intended() == new_mute_state {
                    new_mute_state.into()
                } else {
                    t.set_inner(new_mute_state).into()
                }
            }
        };

        self.individual_mute_state.set(mute_state_update);
    }

    /// Returns current [`MuteStateController::individual_mute_state`].
    pub fn individual_mute_state(&self) -> MuteState {
        self.individual_mute_state.get()
    }

    /// Starts transition of the [`MuteStateController::individual_mute_state`]
    /// to the provided one.
    pub fn transition_to(&self, desired_state: StableMuteState) {
        let current_mute_state = self.individual_mute_state.get();
        self.individual_mute_state
            .set(current_mute_state.transition_to(desired_state));
    }

    /// Cancels [`MuteStateController::individual_mute_state`] transition.
    pub fn cancel_transition(&self) {
        let mute_state = self.individual_mute_state.get();
        self.individual_mute_state
            .set(mute_state.cancel_transition());
    }

    /// Returns [`Future`] which will be resolved when [`MuteState`] of this
    /// [`MuteStateController`] will be [`MuteState::Stable`] or the
    /// [`MuteStateController`] is dropped.
    ///
    /// Succeeds if [`MuteStateController`]'s [`MuteState`] transits into the
    /// `desired_state` or the [`MuteStateController`] is dropped.
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::MuteStateTransitsIntoOppositeState`] is
    /// returned if [`MuteStateController`]'s [`MuteState`] transits into the
    /// opposite to the `desired_state`.
    pub fn when_mute_state_stable(
        &self,
        desired_state: StableMuteState,
    ) -> future::LocalBoxFuture<'static, Result<()>> {
        let mut mute_states = self.individual_mute_state.subscribe();
        async move {
            while let Some(state) = mute_states.next().await {
                match state {
                    MuteState::Transition(_) => continue,
                    MuteState::Stable(s) => {
                        return if s == desired_state {
                            Ok(())
                        } else {
                            Err(tracerr::new!(
                                MediaConnectionsError::
                                MuteStateTransitsIntoOppositeState
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
