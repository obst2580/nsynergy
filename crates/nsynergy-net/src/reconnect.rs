use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

/// Configuration for reconnection with exponential backoff.
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Initial delay between reconnection attempts.
    pub initial_delay: Duration,
    /// Maximum delay between reconnection attempts.
    pub max_delay: Duration,
    /// Multiplier applied to the delay after each failed attempt.
    pub backoff_factor: f64,
    /// Maximum number of reconnection attempts (0 = unlimited).
    pub max_attempts: u32,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            backoff_factor: 2.0,
            max_attempts: 0,
        }
    }
}

/// Tracks the state of a reconnection sequence.
#[derive(Debug, Clone)]
pub struct ReconnectState {
    config: ReconnectConfig,
    current_delay: Duration,
    attempt: u32,
}

impl ReconnectState {
    pub fn new(config: ReconnectConfig) -> Self {
        let current_delay = config.initial_delay;
        Self {
            config,
            current_delay,
            attempt: 0,
        }
    }

    /// Returns the current attempt number.
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Returns the current delay.
    pub fn current_delay(&self) -> Duration {
        self.current_delay
    }

    /// Waits for the backoff delay, then increments the attempt counter.
    /// Returns `true` if another attempt should be made, `false` if
    /// the maximum number of attempts has been reached.
    pub async fn wait_and_advance(&mut self) -> bool {
        if self.config.max_attempts > 0 && self.attempt >= self.config.max_attempts {
            warn!(
                attempts = self.attempt,
                max = self.config.max_attempts,
                "max reconnection attempts reached"
            );
            return false;
        }

        self.attempt += 1;
        info!(
            attempt = self.attempt,
            delay_ms = self.current_delay.as_millis(),
            "reconnecting"
        );

        sleep(self.current_delay).await;

        // Advance delay with exponential backoff
        let next = Duration::from_secs_f64(
            self.current_delay.as_secs_f64() * self.config.backoff_factor,
        );
        self.current_delay = next.min(self.config.max_delay);

        true
    }

    /// Resets the reconnection state after a successful connection.
    pub fn reset(&mut self) {
        self.attempt = 0;
        self.current_delay = self.config.initial_delay;
        info!("reconnection state reset");
    }
}

/// Heartbeat configuration.
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// How often to send heartbeat pings.
    pub interval: Duration,
    /// How long to wait for a pong before considering the connection dead.
    pub timeout: Duration,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(5),
            timeout: Duration::from_secs(15),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = ReconnectConfig::default();
        assert_eq!(config.initial_delay, Duration::from_secs(1));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert!((config.backoff_factor - 2.0).abs() < f64::EPSILON);
        assert_eq!(config.max_attempts, 0);
    }

    #[test]
    fn initial_state() {
        let state = ReconnectState::new(ReconnectConfig::default());
        assert_eq!(state.attempt(), 0);
        assert_eq!(state.current_delay(), Duration::from_secs(1));
    }

    #[test]
    fn reset_restores_initial_state() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_factor: 2.0,
            max_attempts: 0,
        };
        let mut state = ReconnectState::new(config);
        state.attempt = 5;
        state.current_delay = Duration::from_secs(5);

        state.reset();
        assert_eq!(state.attempt(), 0);
        assert_eq!(state.current_delay(), Duration::from_millis(100));
    }

    #[tokio::test]
    async fn backoff_increases_delay() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            backoff_factor: 2.0,
            max_attempts: 5,
        };
        let mut state = ReconnectState::new(config);

        assert!(state.wait_and_advance().await);
        assert_eq!(state.attempt(), 1);
        // After first attempt, delay should double: 10ms -> 20ms
        assert_eq!(state.current_delay(), Duration::from_millis(20));

        assert!(state.wait_and_advance().await);
        assert_eq!(state.attempt(), 2);
        // 20ms -> 40ms
        assert_eq!(state.current_delay(), Duration::from_millis(40));
    }

    #[tokio::test]
    async fn backoff_caps_at_max() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_millis(800),
            backoff_factor: 2.0,
            max_attempts: 0,
        };
        let mut state = ReconnectState::new(config);

        state.wait_and_advance().await;
        // 500ms * 2.0 = 1000ms, capped to 800ms
        assert_eq!(state.current_delay(), Duration::from_millis(800));

        state.wait_and_advance().await;
        // 800ms * 2.0 = 1600ms, still capped to 800ms
        assert_eq!(state.current_delay(), Duration::from_millis(800));
    }

    #[tokio::test]
    async fn max_attempts_stops_reconnection() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            backoff_factor: 1.5,
            max_attempts: 3,
        };
        let mut state = ReconnectState::new(config);

        assert!(state.wait_and_advance().await); // attempt 1
        assert!(state.wait_and_advance().await); // attempt 2
        assert!(state.wait_and_advance().await); // attempt 3
        assert!(!state.wait_and_advance().await); // should return false
    }

    #[tokio::test]
    async fn reset_after_reconnect_allows_new_attempts() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            backoff_factor: 2.0,
            max_attempts: 2,
        };
        let mut state = ReconnectState::new(config);

        assert!(state.wait_and_advance().await);
        assert!(state.wait_and_advance().await);
        assert!(!state.wait_and_advance().await); // exhausted

        state.reset();
        assert!(state.wait_and_advance().await); // fresh start
        assert_eq!(state.attempt(), 1);
    }

    #[test]
    fn heartbeat_default_config() {
        let config = HeartbeatConfig::default();
        assert_eq!(config.interval, Duration::from_secs(5));
        assert_eq!(config.timeout, Duration::from_secs(15));
    }
}
