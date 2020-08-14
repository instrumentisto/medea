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

#[cfg(not(feature = "mockable"))]
const MUTE_TRANSITION_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(feature = "mockable")]
const MUTE_TRANSITION_TIMEOUT: Duration = Duration::from_millis(500);

pub struct MuteStateController {
    mute_state: ObservableCell<MuteState>,
    mute_timeout_handle: RefCell<Option<ResettableDelayHandle>>,
    on_finalized_subs: RefCell<Vec<mpsc::UnboundedSender<StableMuteState>>>,
}

impl MuteStateController {
    pub fn new(mute_state: StableMuteState) -> Rc<Self> {
        let this = Rc::new(Self {
            mute_state: ObservableCell::new(mute_state.into()),
            on_finalized_subs: RefCell::default(),
            mute_timeout_handle: RefCell::new(None),
        });
        this.clone().spawn();

        this
    }

    pub fn on_finalized(&self) -> LocalBoxStream<'static, StableMuteState> {
        let (tx, rx) = mpsc::unbounded();
        self.on_finalized_subs.borrow_mut().push(tx);

        Box::pin(rx)
    }

    fn send_finalized_state(&self, state: StableMuteState) {
        let mut on_finalize_subs = self.on_finalized_subs.borrow_mut();
        *on_finalize_subs = on_finalize_subs
            .drain(..)
            .filter(|s| s.unbounded_send(state).is_ok())
            .collect();
    }

    pub fn spawn(self: Rc<Self>) {
        // we don't care about initial state, cause transceiver is inactive atm
        let mut mute_state_changes = self.mute_state.subscribe().skip(1);
        let weak_this = Rc::downgrade(&self);
        spawn_local(async move {
            while let Some(mute_state) = mute_state_changes.next().await {
                if let Some(this) = weak_this.upgrade() {
                    match mute_state {
                        MuteState::Stable(stable) => {
                            this.send_finalized_state(stable);
                        }
                        MuteState::Transition(_) => {
                            let weak_this = Rc::downgrade(&this);
                            spawn_local(async move {
                                let mut transitions =
                                    this.mute_state.subscribe().skip(1);
                                let (timeout, timeout_handle) =
                                    resettable_delay_for(
                                        MUTE_TRANSITION_TIMEOUT,
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
                                                .mute_state
                                                .get()
                                                .cancel_transition();
                                            this.mute_state.set(stable);
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

    /// Checks whether [`Sender`] is in [`MuteState::Muted`].
    pub fn is_muted(&self) -> bool {
        self.mute_state.get() == StableMuteState::Muted.into()
    }

    /// Checks whether [`Sender`] is in [`MuteState::NotMuted`].
    pub fn is_not_muted(&self) -> bool {
        self.mute_state.get() == StableMuteState::NotMuted.into()
    }

    /// Stops mute/unmute timeout of this [`Sender`].
    pub fn stop_mute_state_transition_timeout(&self) {
        if let Some(timer) = &*self.mute_timeout_handle.borrow() {
            timer.stop();
        }
    }

    /// Resets mute/unmute timeout of this [`Sender`].
    pub fn reset_mute_state_transition_timeout(&self) {
        if let Some(timer) = &*self.mute_timeout_handle.borrow() {
            timer.reset();
        }
    }

    pub fn update(&self, is_muted: bool) {
        let new_mute_state = StableMuteState::from(is_muted);
        let current_mute_state = self.mute_state.get();

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

        self.mute_state.set(mute_state_update);
    }

    pub fn mute_state(&self) -> MuteState {
        self.mute_state.get()
    }

    pub fn transition_to(&self, desired_state: StableMuteState) {
        let current_mute_state = self.mute_state.get();
        self.mute_state
            .set(current_mute_state.transition_to(desired_state));
    }

    pub fn cancel_transition(&self) {
        let mute_state = self.mute_state.get();
        self.mute_state.set(mute_state.cancel_transition());
    }

    /// Returns [`Future`] which will be resolved when [`MuteState`] of this
    /// [`Sender`] will be [`MuteState::Stable`] or the [`Sender`] is dropped.
    ///
    /// Succeeds if [`Sender`]'s [`MuteState`] transits into the `desired_state`
    /// or the [`Sender`] is dropped.
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::MuteStateTransitsIntoOppositeState`] is
    /// returned if [`Sender`]'s [`MuteState`] transits into the opposite to
    /// the `desired_state`.
    pub fn when_mute_state_stable(
        &self,
        desired_state: StableMuteState,
    ) -> future::LocalBoxFuture<'static, Result<()>> {
        let mut mute_states = self.mute_state.subscribe();
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
