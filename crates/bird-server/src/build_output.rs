use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use bird_core::ImageRef;

// a build is usually deployed right after, so only the last few are worth holding on to
const KEPT_BUILDS: usize = 8;
const MAX_LINES: usize = 3000;

/// The output of recent builds, until the deploy of their image takes it into its log
#[derive(Debug, Default)]
pub(crate) struct BuildOutputs(Mutex<VecDeque<(ImageRef, Vec<String>)>>);

impl BuildOutputs {
    pub(crate) fn record(&self, image: ImageRef, mut lines: Vec<String>) {
        if lines.len() > MAX_LINES {
            lines.drain(..lines.len() - MAX_LINES);
        }
        let mut builds = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if builds.len() >= KEPT_BUILDS {
            builds.pop_front();
        }
        builds.push_back((image, lines));
    }

    pub(crate) fn take(&self, image: &ImageRef) -> Vec<String> {
        let mut builds = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        builds
            .iter()
            .position(|(built, _)| built == image)
            .and_then(|index| builds.remove(index))
            .map(|(_, lines)| lines)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(tag: u32) -> ImageRef {
        format!("localhost/bird/web:{tag}").parse().unwrap()
    }

    #[test]
    fn hands_a_builds_output_to_its_deploy_once() {
        let outputs = BuildOutputs::default();
        outputs.record(image(1), vec!["STEP 1/2".to_owned()]);
        assert_eq!(outputs.take(&image(1)), ["STEP 1/2"]);
        assert_eq!(outputs.take(&image(1)), Vec::<String>::new());
        for tag in 0..=u32::try_from(KEPT_BUILDS).unwrap() {
            outputs.record(image(tag), vec![tag.to_string()]);
        }
        assert_eq!(outputs.take(&image(0)), Vec::<String>::new());
        assert_eq!(outputs.take(&image(2)), ["2"]);
    }
}
