use std::collections::VecDeque;

// Tracks which load the web UI is waiting on, so `mpv-event-video-ready` is only
// sent for the current file and only once per load (seeks also fire PlaybackRestart).
#[derive(Debug, Default)]
pub struct VideoReadyState {
    next_load_id: u64,
    current_load_id: Option<u64>,
    pending_load_ids: VecDeque<u64>,
    active_load_id: Option<u64>,
    file_loaded_id: Option<u64>,
    ready_sent_id: Option<u64>,
}

impl VideoReadyState {
    pub fn begin_transition(&mut self, loads_file: bool) -> u64 {
        self.next_load_id += 1;
        self.current_load_id = Some(self.next_load_id);
        self.file_loaded_id = None;
        self.ready_sent_id = None;
        if loads_file {
            self.pending_load_ids.push_back(self.next_load_id);
        }
        self.next_load_id
    }

    pub fn start_file(&mut self) {
        self.active_load_id = self.pending_load_ids.pop_front();
        self.file_loaded_id = None;
    }

    pub fn file_loaded(&mut self) {
        if self.active_load_id == self.current_load_id {
            self.file_loaded_id = self.active_load_id;
        }
    }

    pub fn playback_restarted(&mut self) -> Option<u64> {
        let load_id = self.current_load_id?;
        if self.file_loaded_id != Some(load_id) || self.ready_sent_id == Some(load_id) {
            return None;
        }
        self.ready_sent_id = Some(load_id);
        Some(load_id)
    }
}

#[cfg(test)]
mod tests {
    use super::VideoReadyState;

    #[test]
    fn ready_once_per_load() {
        let mut state = VideoReadyState::default();
        let id = state.begin_transition(true);
        state.start_file();
        assert_eq!(state.playback_restarted(), None);
        state.file_loaded();
        assert_eq!(state.playback_restarted(), Some(id));
        assert_eq!(state.playback_restarted(), None);
    }

    #[test]
    fn stale_load_is_ignored() {
        let mut state = VideoReadyState::default();
        state.begin_transition(true);
        let second = state.begin_transition(true);

        state.start_file();
        state.file_loaded();
        assert_eq!(state.playback_restarted(), None);

        state.start_file();
        state.file_loaded();
        assert_eq!(state.playback_restarted(), Some(second));
    }

    #[test]
    fn stop_invalidates_pending_load() {
        let mut state = VideoReadyState::default();
        state.begin_transition(true);
        state.begin_transition(false);

        state.start_file();
        state.file_loaded();
        assert_eq!(state.playback_restarted(), None);
    }
}
