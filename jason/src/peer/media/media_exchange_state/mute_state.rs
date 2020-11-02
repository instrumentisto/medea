#[derive(Debug, Clone, Copy)]
pub enum StableMuteState {
    Muted,
    Unmuted,
}

#[derive(Debug, Clone, Copy)]
pub enum TransitionMuteState {
    Muting,
    Unmuting,
}
