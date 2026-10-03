use bird_podman::ContainerState;

pub(super) const MAX_STRIKES: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Verdict {
    Healthy,
    Start { strikes: u32 },
    Strike { strikes: u32 },
    Fail,
}

impl Verdict {
    pub(super) fn counts_toward_desired(self) -> bool {
        !matches!(self, Self::Fail)
    }
}

// every unhealthy observation is a strike, so crash loops and hung apps both end in Fail
pub(super) fn judge(state: ContainerState, responded: Option<bool>, strikes: u32) -> Verdict {
    if state == ContainerState::Running && responded == Some(true) {
        return Verdict::Healthy;
    }
    let strikes = strikes.saturating_add(1);
    if strikes >= MAX_STRIKES {
        return Verdict::Fail;
    }
    match state {
        ContainerState::Created | ContainerState::Exited => Verdict::Start { strikes },
        _ => Verdict::Strike { strikes },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_only_when_running_and_answering() {
        assert_eq!(
            judge(ContainerState::Running, Some(true), 2),
            Verdict::Healthy
        );
        assert_eq!(
            judge(ContainerState::Running, Some(false), 0),
            Verdict::Strike { strikes: 1 }
        );
        assert_eq!(
            judge(ContainerState::Running, None, 0),
            Verdict::Strike { strikes: 1 }
        );
    }

    #[test]
    fn restarts_stopped_containers() {
        assert_eq!(
            judge(ContainerState::Exited, None, 0),
            Verdict::Start { strikes: 1 }
        );
        assert_eq!(
            judge(ContainerState::Created, None, 1),
            Verdict::Start { strikes: 2 }
        );
    }

    #[test]
    fn fails_after_max_strikes() {
        assert_eq!(
            judge(ContainerState::Running, Some(false), MAX_STRIKES - 1),
            Verdict::Fail
        );
        assert_eq!(
            judge(ContainerState::Exited, None, MAX_STRIKES - 1),
            Verdict::Fail
        );
        assert!(!Verdict::Fail.counts_toward_desired());
        assert!(Verdict::Strike { strikes: 1 }.counts_toward_desired());
    }
}
