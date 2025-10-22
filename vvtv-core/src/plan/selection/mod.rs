use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use chrono::{DateTime, Duration, Utc};
use rand::distributions::Open01;
use rand::Rng;

/// Applies Gumbel-Top-k without replacement using the provided RNG.
///
/// The function is deterministic as long as the RNG is seeded with a
/// reproducible seed.
pub fn gumbel_topk_indices<R>(scores: &[f64], k: usize, rng: &mut R) -> Vec<usize>
where
    R: Rng + ?Sized,
{
    if scores.is_empty() || k == 0 {
        return Vec::new();
    }
    let limit = k.min(scores.len());

    let mut perturbed: Vec<(usize, f64)> = scores
        .iter()
        .enumerate()
        .map(|(idx, score)| {
            // Sample from (0, 1] to avoid log(0).
            let u: f64 = rng.sample(Open01);
            let gumbel = -(-u.ln()).ln();
            (idx, score + gumbel)
        })
        .collect();

    perturbed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    perturbed.truncate(limit);
    perturbed.into_iter().map(|(idx, _)| idx).collect()
}

/// Generates a reproducible seed based on the current slot epoch, the
/// programming window identifier and a global seed from the business card.
pub fn generate_slot_seed_robust(
    now: DateTime<Utc>,
    slot_duration: Duration,
    window_id: u64,
    global_seed: u64,
) -> u64 {
    let slot_seconds = slot_duration.num_seconds().max(60);
    let epoch_index = now.timestamp().div_euclid(slot_seconds);

    let mut hasher = DefaultHasher::new();
    epoch_index.hash(&mut hasher);
    window_id.hash(&mut hasher);
    global_seed.hash(&mut hasher);
    hasher.finish()
}

/// Returns a min-max normalized copy of the provided scores, useful for logging.
pub fn normalize_scores(scores: &[f64]) -> Vec<f64> {
    if scores.is_empty() {
        return Vec::new();
    }
    let min = scores
        .iter()
        .fold(f64::INFINITY, |acc, value| acc.min(*value));
    let max = scores
        .iter()
        .fold(f64::NEG_INFINITY, |acc, value| acc.max(*value));
    if (max - min).abs() < f64::EPSILON {
        return vec![1.0; scores.len()];
    }
    scores
        .iter()
        .map(|value| (value - min) / (max - min))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;

    #[test]
    fn deterministic_gumbel_topk() {
        let mut rng = ChaCha20Rng::seed_from_u64(42);
        let scores = vec![0.9, 0.4, 0.1, 0.7, 0.5];
        let indices = gumbel_topk_indices(&scores, 3, &mut rng);
        assert_eq!(indices, vec![0, 1, 3]);

        let mut rng_again = ChaCha20Rng::seed_from_u64(42);
        let indices_again = gumbel_topk_indices(&scores, 3, &mut rng_again);
        assert_eq!(indices, indices_again);
    }

    #[test]
    fn seed_changes_with_slot() {
        let now = Utc::now();
        let seed_a = generate_slot_seed_robust(now, Duration::minutes(15), 1, 99);
        let seed_b =
            generate_slot_seed_robust(now + Duration::minutes(15), Duration::minutes(15), 1, 99);
        assert_ne!(seed_a, seed_b);
    }

    #[test]
    fn normalize_scores_behaviour() {
        let scores = vec![1.0, 3.0, 5.0];
        let normalized = normalize_scores(&scores);
        assert_eq!(normalized, vec![0.0, 0.5, 1.0]);

        let same_scores = vec![2.0, 2.0];
        let normalized_same = normalize_scores(&same_scores);
        assert_eq!(normalized_same, vec![1.0, 1.0]);
    }
}
