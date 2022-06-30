
use libafl::{
    monitors::SimpleMonitor,
    monitors::Monitor,
    monitors::ClientStats,
};

use core::time::Duration;
use libafl::bolts::{current_time, format_duration_hms};

use std::{fmt, fmt::Debug};

#[derive(Clone)]
pub struct UEMonitor<F>
    where F : FnMut(String) {
    inner : SimpleMonitor<F>,
    print_fn : F,
}

impl<F> UEMonitor<F>
    where F : FnMut(String) + Clone {

    pub fn new(f : F) -> Self {
        return UEMonitor {
            inner : SimpleMonitor::new(f.clone()),
            print_fn : f,
        }
    }
}

impl<F> Monitor for UEMonitor<F>
    where F : FnMut(String) {

    fn display(&mut self, event_msg: String, sender_id: u32) {
        let per_c = self.inner.client_stats().iter()
            .flat_map(|cs| {
                cs.user_monitor.iter().map(|(k, v)| format!("{}: {}", k, v))
            })
            .collect::<Vec<String>>()
            .join(",");
        let fmt = format!(
            "[{} #{}]: {{ run time: {}, clients: {}, corpus: {}, objectives: {}, executions: {}, exec/sec: {}, \n\tper_c [{}] }}",
            event_msg,
            sender_id,
            format_duration_hms(&(current_time() - self.inner.start_time())),
            self.client_stats().len(),
            self.corpus_size(),
            self.objective_size(),
            self.total_execs(),
            self.execs_per_sec(),
            per_c
        );
        (self.print_fn)(fmt);

        // Only print perf monitor if the feature is enabled
        #[cfg(feature = "introspection")]
        {
            // Print the client performance monitor.
            let fmt = format!(
                "Client {:03}:\n{}",
                sender_id, self.client_stats[sender_id as usize].introspection_monitor
            );
            (self.print_fn)(fmt);

            // Separate the spacing just a bit
            (self.print_fn)("".to_string());
        }
    }

    fn client_stats_mut(&mut self) -> &mut Vec<ClientStats> { self.inner.client_stats_mut() }
    fn client_stats(&self) -> &[ClientStats] { self.inner.client_stats() }
    fn start_time(&mut self) -> Duration { self.inner.start_time() }
    fn corpus_size(&self) -> u64 { self.inner.corpus_size() }
    fn objective_size(&self) -> u64 { self.inner.objective_size() }
    fn total_execs(&mut self) -> u64 { self.inner.total_execs() }
    fn execs_per_sec(&mut self) -> u64 { self.inner.execs_per_sec() }
    fn client_stats_mut_for(&mut self, client_id: u32) -> &mut ClientStats { self.inner.client_stats_mut_for(client_id) }
}

impl<F> Debug for UEMonitor<F>
where
    F: FnMut(String),
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        return self.inner.fmt(f)
    }
}
